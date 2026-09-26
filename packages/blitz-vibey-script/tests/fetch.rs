//! Tests for the Fetch API exposed by blitz-vibey-script

use std::sync::Arc;
use std::time::{Duration, Instant};

use blitz_dom::{Document, DocumentConfig};
use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use blitz_vibey_script::ScriptDocument;

/// A NetProvider which answers every request on a background thread after a
/// short delay, reporting the configured status and body.
#[derive(Clone)]
struct StubNetProvider {
    status: u16,
    body: &'static str,
}

impl NetProvider for StubNetProvider {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let status = self.status;
        let body = Bytes::from(self.body.as_bytes().to_vec());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            handler.bytes_with_status(status, request.url.to_string(), body);
        });
    }
}

/// A provider which reports itself as a no-op, so fetch promises reject.
struct NoopNetProvider;

impl NetProvider for NoopNetProvider {
    fn fetch(&self, _doc_id: usize, _request: Request, _handler: Box<dyn NetHandler>) {}

    fn is_noop(&self) -> bool {
        true
    }
}

fn text_of_selector(doc: &ScriptDocument, selector: &str) -> String {
    let inner = doc.inner();
    let node_id = inner
        .query_selector(selector)
        .unwrap()
        .unwrap_or_else(|| panic!("no node matching {selector}"));
    inner.get_node(node_id).unwrap().text_content()
}

fn wait_for_text(doc: &mut ScriptDocument, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        doc.poll(None);
        if text_of_selector(doc, "#root") == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "fetch promise never produced {expected:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn fetch_resolves_json_response() {
    let mut doc = ScriptDocument::from_html(
        r#"
        <html><body>
            <div id="root"></div>
            <script>
                fetch("/api/track").then((response) => response.json()).then((data) => {
                    document.getElementById("root").textContent = data.title;
                });
            </script>
        </body></html>
        "#,
        DocumentConfig {
            net_provider: Some(Arc::new(StubNetProvider {
                status: 200,
                body: r#"{"title":"resolved"}"#,
            })),
            base_url: Some("https://example.com/page".to_string()),
            ..Default::default()
        },
    );
    doc.execute_scripts();
    wait_for_text(&mut doc, "resolved");
}

#[test]
fn fetch_http_error_status_resolves_with_ok_false() {
    let mut doc = ScriptDocument::from_html(
        r#"
        <html><body>
            <div id="root"></div>
            <script>
                fetch("https://example.com/missing").then((response) => {
                    document.getElementById("root").textContent =
                        response.status + ":" + response.ok;
                });
            </script>
        </body></html>
        "#,
        DocumentConfig {
            net_provider: Some(Arc::new(StubNetProvider {
                status: 404,
                body: "",
            })),
            ..Default::default()
        },
    );
    doc.execute_scripts();
    wait_for_text(&mut doc, "404:false");
}

#[test]
fn fetch_rejects_when_no_net_provider_is_available() {
    let mut doc = ScriptDocument::from_html(
        r#"
        <html><body>
            <div id="root"></div>
            <script>
                fetch("https://example.com/unavailable").catch(() => {
                    document.getElementById("root").textContent = "rejected";
                });
            </script>
        </body></html>
        "#,
        DocumentConfig {
            net_provider: Some(Arc::new(NoopNetProvider)),
            ..Default::default()
        },
    );
    doc.execute_scripts();
    wait_for_text(&mut doc, "rejected");
}
