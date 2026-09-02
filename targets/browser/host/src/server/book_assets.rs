//! Production Patchbay presentation assets reused by the executable Book.

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
