use std::sync::{Arc, Mutex};

use blitz_dom::DocumentConfig;
use blitz_traits::navigation::{NavigationOptions, NavigationProvider};
use blitz_traits::net::{NetHandler, NetProvider, Request, Url};
use blitz_vibey_script::ScriptDocument;

#[derive(Default)]
struct BrowserServices {
    navigations: Mutex<Vec<NavigationOptions>>,
    cookies: Mutex<Vec<(Url, String)>>,
}

impl NavigationProvider for BrowserServices {
    fn navigate_to(&self, options: NavigationOptions) {
        self.navigations.lock().unwrap().push(options);
    }
}

impl NetProvider for BrowserServices {
    fn fetch(&self, _: usize, _: Request, _: Box<dyn NetHandler>) {}
    fn is_noop(&self) -> bool {
        true
    }
    fn document_cookies(&self, _: &Url) -> String {
        "visible=value".into()
    }
    fn set_document_cookie(&self, url: &Url, cookie: &str) {
        self.cookies
            .lock()
            .unwrap()
            .push((url.clone(), cookie.into()));
    }
}

#[test]
fn location_navigates_through_the_embedder_and_history_stays_in_document() {
    let services = Arc::new(BrowserServices::default());
    let mut doc = ScriptDocument::from_html(
        "<p>current</p>",
        DocumentConfig {
            base_url: Some("https://example.test:8443/dir/start?q=1#old".into()),
            navigation_provider: Some(services.clone()),
            net_provider: Some(services.clone()),
            ..Default::default()
        },
    )
    .without_timer_thread();
    doc.eval(r#"
        __blitz_send_message(String(location === document.location && location === window.location));
        __blitz_send_message([location.host, location.hostname, location.port, location.search, location.hash].join('|'));
        location.href = '../next';
        window.location = '/window';
        document.location = '/document';
        location.assign('/assign');
        location.replace('/replace');
        location.reload();
        __blitz_send_message(String(location));
        history.pushState({ok: true}, '', '/spa?x=2#new');
        __blitz_send_message([document.URL, location.href, history.state.ok].join('|'));
        document.cookie = 'script=ok; Path=/';
        __blitz_send_message(document.cookie);
        try { history.replaceState(null, '', 'https://other.test/'); }
        catch (e) { __blitz_send_message('cross-origin rejected'); }
        __blitz_send_message(location.href);
    "#);
    assert!(doc.take_js_errors().is_empty());
    assert_eq!(
        doc.take_messages(),
        [
            "true",
            "example.test:8443|example.test|8443|?q=1|#old",
            "https://example.test:8443/dir/start?q=1#old",
            "https://example.test:8443/spa?x=2#new|https://example.test:8443/spa?x=2#new|true",
            "visible=value",
            "cross-origin rejected",
            "https://example.test:8443/spa?x=2#new",
        ]
    );
    let navigations = services.navigations.lock().unwrap();
    assert_eq!(navigations.len(), 6);
    assert_eq!(
        navigations.iter().map(|n| n.url.path()).collect::<Vec<_>>(),
        [
            "/next",
            "/window",
            "/document",
            "/assign",
            "/replace",
            "/dir/start"
        ]
    );
    assert_eq!(
        navigations.iter().map(|n| n.replace).collect::<Vec<_>>(),
        [false, false, false, false, true, true]
    );
    let cookies = services.cookies.lock().unwrap();
    assert_eq!(
        cookies[0].0.as_str(),
        "https://example.test:8443/spa?x=2#new"
    );
    assert_eq!(cookies[0].1, "script=ok; Path=/");
}
