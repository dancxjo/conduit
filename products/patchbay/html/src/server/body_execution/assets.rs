//! The product consumer and existing browser Host modules in the admitted package.
pub(in crate::server) fn resource(path: &str) -> Option<&'static [u8]> {
    Some(match path {
        "assets/body-execution.mjs" => include_bytes!("../../../assets/body-execution.mjs"),
        "assets/browser-body-host.mjs" => {
            include_bytes!("../../../../../../targets/browser/host/assets/browser-body-host.mjs")
        }
        "assets/browser-human-input.mjs" => {
            include_bytes!("../../../../../../targets/browser/host/assets/browser-human-input.mjs")
        }
        "assets/browser-form-effects.mjs" => {
            include_bytes!("../../../../../../targets/browser/host/assets/browser-form-effects.mjs")
        }
        _ => return None,
    })
}
