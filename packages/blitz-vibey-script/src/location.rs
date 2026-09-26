//! Location delegates navigation to the embedder. A requested URL does not
//! become this document's URL until the embedder commits a new document.

use blitz_traits::navigation::NavigationOptions;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsNativeError, JsResult, JsValue};
use url::{Position, Url};

use crate::dom::{define_accessor, define_method, dom_ctx, js_str, to_rust_string};

pub(crate) fn register(context: &mut Context) {
    let location = JsObject::with_object_proto(context.intrinsics());
    define_accessor(&location, "href", Some(href), Some(assign), context);
    define_accessor(&location, "origin", Some(origin), None, context);
    define_accessor(
        &location,
        "protocol",
        Some(protocol),
        Some(set_protocol),
        context,
    );
    define_accessor(&location, "host", Some(host), Some(set_host), context);
    define_accessor(
        &location,
        "hostname",
        Some(hostname),
        Some(set_hostname),
        context,
    );
    define_accessor(&location, "port", Some(port), Some(set_port), context);
    define_accessor(
        &location,
        "pathname",
        Some(pathname),
        Some(set_pathname),
        context,
    );
    define_accessor(&location, "search", Some(search), Some(set_search), context);
    define_accessor(&location, "hash", Some(hash), Some(set_hash), context);
    define_method(&location, "assign", 1, assign, context);
    define_method(&location, "replace", 1, replace, context);
    define_method(&location, "reload", 0, reload, context);
    define_method(&location, "toString", 0, href, context);
    dom_ctx(context)
        .expect("DOM context")
        .state
        .borrow_mut()
        .location_object = Some(location);
    define_accessor(
        &context.global_object(),
        "location",
        Some(get),
        Some(assign),
        context,
    );
}

pub(crate) fn get(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    Ok(dom_ctx(context)?
        .state
        .borrow()
        .location_object
        .clone()
        .unwrap()
        .into())
}

fn current_url(context: &mut Context) -> JsResult<Url> {
    Ok(dom_ctx(context)?
        .state
        .borrow()
        .location_url
        .clone()
        .unwrap_or_else(|| Url::parse("about:blank").unwrap()))
}

fn resolve(args: &[JsValue], context: &mut Context) -> JsResult<Url> {
    let value = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
    let ctx = dom_ctx(context)?;
    let base = ctx.doc.borrow().base_url().clone();
    base.join(&value).map_err(|_| {
        JsNativeError::syntax()
            .with_message("Invalid navigation URL")
            .into()
    })
}

fn navigate(url: Url, replace: bool, context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let doc = ctx.doc.borrow();
    let mut options = NavigationOptions::new(url, None, doc.id());
    options.replace = replace;
    doc.navigation_provider.navigate_to(options);
    Ok(JsValue::undefined())
}

pub(crate) fn assign(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let url = resolve(args, context)?;
    navigate(url, false, context)
}

fn replace(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let url = resolve(args, context)?;
    navigate(url, true, context)
}

fn reload(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let url = current_url(context)?;
    navigate(url, true, context)
}

/// History changes the URL of the existing document without loading a resource.
pub(crate) fn history_url(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let url = resolve(args, context)?;
    if url.origin() != current_url(context)?.origin() {
        return Err(JsNativeError::typ()
            .with_message("History URL must have the document's origin")
            .into());
    }
    let ctx = dom_ctx(context)?;
    ctx.doc.borrow_mut().set_base_url(url.as_str());
    let mut state = ctx.state.borrow_mut();
    state.base_url = Some(url.clone());
    state.location_url = Some(url);
    Ok(JsValue::undefined())
}

macro_rules! url_getter {
    ($name:ident, $read:expr) => {
        pub(crate) fn $name(
            _: &JsValue,
            _: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let url = current_url(context)?;
            Ok(js_str(&$read(&url)))
        }
    };
}

url_getter!(href, |u: &Url| u.to_string());
url_getter!(origin, |u: &Url| u.origin().ascii_serialization());
url_getter!(protocol, |u: &Url| format!("{}:", u.scheme()));
url_getter!(host, |u: &Url| u[Position::BeforeHost..Position::AfterPort]
    .to_owned());
url_getter!(hostname, |u: &Url| u
    [Position::BeforeHost..Position::AfterHost]
    .to_owned());
url_getter!(port, |u: &Url| u
    .port()
    .map(|p| p.to_string())
    .unwrap_or_default());
url_getter!(pathname, |u: &Url| u.path().to_owned());
url_getter!(search, |u: &Url| u
    .query()
    .filter(|s| !s.is_empty())
    .map(|s| format!("?{s}"))
    .unwrap_or_default());
url_getter!(hash, |u: &Url| u
    .fragment()
    .filter(|s| !s.is_empty())
    .map(|s| format!("#{s}"))
    .unwrap_or_default());

macro_rules! url_setter {
    ($name:ident, $write:expr) => {
        fn $name(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
            let value = to_rust_string(args.first().unwrap_or(&JsValue::undefined()), context)?;
            let mut url = current_url(context)?;
            if $write(&mut url, &value) {
                navigate(url, false, context)
            } else {
                Ok(JsValue::undefined())
            }
        }
    };
}

url_setter!(set_protocol, |u: &mut Url, s: &str| u
    .set_scheme(s.trim_end_matches(':'))
    .is_ok());
url_setter!(set_hostname, |u: &mut Url, s: &str| u
    .set_host(Some(s))
    .is_ok());
url_setter!(set_host, |u: &mut Url, s: &str| {
    let Ok(parsed) = Url::parse(&format!("{}://{s}/", u.scheme())) else {
        return false;
    };
    u.set_host(parsed.host_str()).is_ok() && u.set_port(parsed.port()).is_ok()
});
url_setter!(set_port, |u: &mut Url, s: &str| {
    let port = if s.is_empty() {
        None
    } else {
        let Ok(port) = s.parse() else {
            return false;
        };
        Some(port)
    };
    u.set_port(port).is_ok()
});
url_setter!(set_pathname, |u: &mut Url, s: &str| {
    u.set_path(s);
    true
});
url_setter!(set_search, |u: &mut Url, s: &str| {
    u.set_query((!s.is_empty()).then(|| s.strip_prefix('?').unwrap_or(s)));
    true
});
url_setter!(set_hash, |u: &mut Url, s: &str| {
    u.set_fragment((!s.is_empty()).then(|| s.strip_prefix('#').unwrap_or(s)));
    true
});
