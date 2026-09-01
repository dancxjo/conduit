//! Production Patchbay presentation assets reused by the executable Book.

const REACT: &[u8] = include_bytes!("../../../../../apps/patchbay/html/assets/react.min.js");
const REACT_DOM: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/react-dom.min.js");
const REACT_FLOW: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/react-flow.min.js");
const REACT_FLOW_STYLE: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/react-flow.css");
const FLOW: &[u8] = include_bytes!("../../../../../apps/patchbay/html/assets/flow.js");
const FLOW_SCENE: &[u8] = include_bytes!("../../../../../apps/patchbay/html/assets/flow-scene.js");
const FLOW_LAYOUT: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/flow-layout.js");
const FLOW_FACEPLATE: &[u8] =
    include_bytes!("../../../../../apps/patchbay/html/assets/flow-faceplate.js");
const PORTABLE_NAVIGATION: &[u8] =
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
        _ => None,
    }
}
