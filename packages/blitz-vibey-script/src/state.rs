//! Shared state accessible from both the Rust side (`ScriptDocument`) and the
//! JavaScript side (native functions registered with the Boa `Context`).

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use blitz_dom::{BaseDocument, NodeId};
use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsFunction;
use boa_engine::{Finalize, JsData, Trace};
use url::Url;

use crate::clock::ScriptClock;
use crate::timers::TimerQueue;

/// Prototype objects for the DOM wrapper classes
pub(crate) struct DomProtos {
    pub node: JsObject,
    pub element: JsObject,
    pub character_data: JsObject,
    pub document: JsObject,
    pub event: JsObject,
    pub style: JsObject,
    pub computed_style: JsObject,
}

/// The document's `readyState`
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ReadyState {
    #[default]
    Loading,
    Interactive,
    Complete,
}

impl ReadyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Interactive => "interactive",
            Self::Complete => "complete",
        }
    }
}

/// An event listener registered via `addEventListener`
#[derive(Clone)]
pub(crate) struct Listener {
    pub callback: JsObject,
    pub capture: bool,
    pub once: bool,
}

pub(crate) type ListenerMap = HashMap<String, Vec<Listener>>;

/// A `fetch()` promise awaiting its network response.
pub(crate) struct PendingFetch {
    pub resolve: JsFunction,
    pub reject: JsFunction,
}

/// A completed `fetch()` request, delivered by the network thread.
pub(crate) struct FetchCompletion {
    pub id: usize,
    pub status: u16,
    pub final_url: String,
    pub body: Vec<u8>,
}

/// State owned by the script runtime but shared (via `Rc`) with the native
/// functions exposed to JavaScript.
///
/// Note: this struct stores Boa GC handles (`JsObject`s) in ordinary Rust
/// collections. That is sound because Boa GC handles held outside of the GC
/// heap act as roots (they keep their referents alive).
#[derive(Default)]
pub(crate) struct RuntimeState {
    /// Prototypes for DOM wrapper objects. Set once during runtime initialisation.
    pub protos: Option<DomProtos>,
    /// Cache of JS wrapper objects, keyed by node id.
    ///
    /// DOM wrappers must be cached so that a given DOM node is always represented
    /// by the *same* JS object: scripts rely on object identity (`===`) and on
    /// expando properties persisting across accesses.
    pub node_wrappers: HashMap<NodeId, JsObject>,
    /// Pending `fetch()` requests, keyed by request id.
    pub fetch_pending: HashMap<usize, PendingFetch>,
    /// Monotonic id source for `fetch()` requests.
    pub next_fetch_id: usize,
    /// Completions delivered by the network thread, drained on the UI thread
    /// by [`ScriptRuntime::drain_fetch_queue`](crate::runtime::ScriptRuntime::drain_fetch_queue).
    pub fetch_queue: Arc<Mutex<VecDeque<FetchCompletion>>>,
    /// Event listeners registered on nodes, keyed by node id then event type.
    pub node_listeners: HashMap<NodeId, ListenerMap>,
    /// Event listeners registered on `window`.
    pub window_listeners: ListenerMap,
    /// Pending timers (`setTimeout`/`setInterval`/`requestAnimationFrame`)
    pub timers: TimerQueue,
    /// The clock driving timer deadlines and `Date`
    pub clock: ScriptClock,
    /// Messages sent from JavaScript to the embedder via the
    /// `__blitz_send_message` native function. Drained with
    /// [`ScriptDocument::take_messages`](crate::ScriptDocument::take_messages).
    pub outbound_messages: Vec<String>,
    /// The value exposed as `document.readyState`
    pub ready_state: ReadyState,
    /// Uncaught JavaScript errors (from script loading/evaluation, event
    /// listeners, timer callbacks and promise jobs). Drained with
    /// [`ScriptDocument::take_js_errors`](crate::ScriptDocument::take_js_errors).
    pub uncaught_errors: Vec<String>,
    /// The document's base URL, used to resolve relative `fetch()` URLs.
    pub base_url: Option<Url>,
}

/// Maximum number of errors stored in [`RuntimeState::uncaught_errors`] between
/// drains, so that memory use is bounded for embedders which never drain them
const MAX_STORED_ERRORS: usize = 256;

impl RuntimeState {
    /// Record an error for the embedder to collect via
    /// [`ScriptDocument::take_js_errors`](crate::ScriptDocument::take_js_errors).
    /// Errors beyond [`MAX_STORED_ERRORS`] are dropped (with a marker) until the
    /// stored errors are drained.
    pub fn record_error(&mut self, message: String) {
        use std::cmp::Ordering;
        match self.uncaught_errors.len().cmp(&MAX_STORED_ERRORS) {
            Ordering::Less => self.uncaught_errors.push(message),
            Ordering::Equal => self
                .uncaught_errors
                .push("(further errors suppressed)".to_string()),
            Ordering::Greater => {}
        }
    }

    pub fn protos(&self) -> &DomProtos {
        self.protos
            .as_ref()
            .expect("DOM prototypes not initialised")
    }
}

/// Cloneable handle to the document and the runtime state. This is stored as
/// host-defined data on the Boa [`Context`](boa_engine::Context) so that native
/// functions can access the DOM.
#[derive(Clone, Trace, Finalize, JsData)]
pub(crate) struct DomCtx {
    #[unsafe_ignore_trace]
    pub doc: Rc<RefCell<BaseDocument>>,
    #[unsafe_ignore_trace]
    pub state: Rc<RefCell<RuntimeState>>,
}

impl DomCtx {
    pub fn new(doc: Rc<RefCell<BaseDocument>>) -> Self {
        Self {
            doc,
            state: Rc::new(RefCell::new(RuntimeState::default())),
        }
    }
}
