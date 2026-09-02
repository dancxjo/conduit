//! Production Patchbay presentation assets reused by the executable Book.

use conduit_browser_host::application_package;

pub(super) const DOCUMENT: &[u8] = include_bytes!("../../assets/book.html");
pub(super) const SCRIPT: &[u8] = include_bytes!("../../assets/book.mjs");
pub(super) const STATE: &[u8] = include_bytes!("../../assets/book-state.mjs");
pub(super) const NAVIGATION: &[u8] = include_bytes!("../../assets/book-navigation.mjs");
pub(super) const RUNNER_PRESENTATION: &[u8] =
    include_bytes!("../../assets/book-runner-presentation.mjs");
pub(super) const SYNTAX_EDITOR: &[u8] = include_bytes!("../../assets/book-syntax-editor.mjs");
pub(super) const STYLE: &[u8] = include_bytes!("../../assets/book.css");
const APPLICATION_TEMPLATE: &[u8] = include_bytes!("../../assets/book.application.template.json");
pub(super) const CHAPTERS: [&[u8]; 7] = [
    include_bytes!("../../../../../tour/book/chapter-1.md"),
    include_bytes!("../../../../../tour/book/chapter-2.md"),
    include_bytes!("../../../../../tour/book/chapter-3.md"),
    include_bytes!("../../../../../tour/book/chapter-4.md"),
    include_bytes!("../../../../../tour/book/chapter-5.md"),
    include_bytes!("../../../../../tour/book/chapter-6.md"),
    include_bytes!("../../../../../tour/book/chapter-8.md"),
];

pub(super) const REACT: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/react.min.js");
pub(super) const REACT_DOM: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/react-dom.min.js");
pub(super) const REACT_FLOW: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/react-flow.min.js");
pub(super) const REACT_FLOW_STYLE: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/react-flow.css");
pub(super) const FLOW_STYLE: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/flow.css");
pub(super) const FLOW: &[u8] = include_bytes!("../../../../../apps/patchbay/html/assets/flow.js");
pub(super) const FLOW_SCENE: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/flow-scene.js");
pub(super) const FLOW_LAYOUT: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/flow-layout.js");
pub(super) const FLOW_FACEPLATE: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/flow-faceplate.js");
pub(super) const PORTABLE_NAVIGATION: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/portable-navigation.js");

pub(super) fn build_manifest(runtime: &[u8]) -> Result<Vec<u8>, String> {
    application_package::build_manifest(APPLICATION_TEMPLATE, |path| match path {
        "book.mjs" => Some(SCRIPT),
        "browser-host-membership.mjs" => Some(super::HOST_MEMBERSHIP),
        "book-state.mjs" => Some(STATE),
        "book-navigation.mjs" => Some(NAVIGATION),
        "book-runner-presentation.mjs" => Some(RUNNER_PRESENTATION),
        "book-syntax-editor.mjs" => Some(SYNTAX_EDITOR),
        "assets/flow.js" => Some(FLOW),
        "assets/flow-scene.js" => Some(FLOW_SCENE),
        "assets/flow-layout.js" => Some(FLOW_LAYOUT),
        "assets/flow-faceplate.js" => Some(FLOW_FACEPLATE),
        "assets/portable-navigation.js" => Some(PORTABLE_NAVIGATION),
        "book.css" => Some(STYLE),
        "assets/react-flow.css" => Some(REACT_FLOW_STYLE),
        "assets/flow.css" => Some(FLOW_STYLE),
        "assets/react.min.js" => Some(REACT),
        "assets/react-dom.min.js" => Some(REACT_DOM),
        "assets/react-flow.min.js" => Some(REACT_FLOW),
        "chapter-1.md" => Some(CHAPTERS[0]),
        "chapter-2.md" => Some(CHAPTERS[1]),
        "chapter-3.md" => Some(CHAPTERS[2]),
        "chapter-4.md" => Some(CHAPTERS[3]),
        "chapter-5.md" => Some(CHAPTERS[4]),
        "chapter-6.md" => Some(CHAPTERS[5]),
        "chapter-8.md" => Some(CHAPTERS[6]),
        "runtime.wasm" => Some(runtime),
        _ => None,
    })
}

pub(super) fn response(request: Option<&str>) -> Option<(&'static str, &'static [u8])> {
    let javascript = "text/javascript; charset=utf-8";
    match request? {
        "GET /book/assets/react.min.js HTTP/1.1" => Some((javascript, REACT)),
        "GET /book/assets/react-dom.min.js HTTP/1.1" => Some((javascript, REACT_DOM)),
        "GET /book/assets/react-flow.min.js HTTP/1.1" => Some((javascript, REACT_FLOW)),
        "GET /book/assets/flow.js HTTP/1.1" => Some((javascript, FLOW)),
        "GET /book/assets/flow-scene.js HTTP/1.1" => Some((javascript, FLOW_SCENE)),
        "GET /book/assets/flow-layout.js HTTP/1.1" => Some((javascript, FLOW_LAYOUT)),
        "GET /book/assets/flow-faceplate.js HTTP/1.1" => Some((javascript, FLOW_FACEPLATE)),
        "GET /book/assets/portable-navigation.js HTTP/1.1" => {
            Some((javascript, PORTABLE_NAVIGATION))
        }
        "GET /book/assets/react-flow.css HTTP/1.1" => {
            Some(("text/css; charset=utf-8", REACT_FLOW_STYLE))
        }
        "GET /book/assets/flow.css HTTP/1.1" => Some(("text/css; charset=utf-8", FLOW_STYLE)),
        _ => None,
    }
}
