//! Bounded release and native-package assets for existing-computer Crèche targets.

const SPORE_DOWNLOAD: &[u8] = include_bytes!("../../assets/creche-spore-bundle.mjs");
const NATIVE_ZIP: &[u8] = include_bytes!("../../assets/creche-native-zip.mjs");
const RELEASE_BUNDLE: &[u8] = include_bytes!("../../assets/creche-release-bundle.mjs");
const EXISTING_COMPUTER: &[u8] = include_bytes!("../../assets/creche-existing-computer.mjs");
const STD_ADAPTER: &[u8] = include_bytes!("../../../../std/browser-deployment/creche-adapter.mjs");
const BROWSER_ADAPTER: &[u8] = include_bytes!("../../browser-deployment/creche-adapter.mjs");

pub(super) fn response(request: Option<&str>) -> Option<(&'static str, &'static [u8])> {
    let javascript = "text/javascript; charset=utf-8";
    match request? {
        "GET /creche/creche-spore-bundle.mjs HTTP/1.1" => Some((javascript, SPORE_DOWNLOAD)),
        "GET /creche/creche-native-zip.mjs HTTP/1.1" => Some((javascript, NATIVE_ZIP)),
        "GET /creche/creche-release-bundle.mjs HTTP/1.1" => Some((javascript, RELEASE_BUNDLE)),
        "GET /creche/creche-existing-computer.mjs HTTP/1.1" => {
            Some((javascript, EXISTING_COMPUTER))
        }
        "GET /creche/targets/std/browser-deployment/creche-adapter.mjs HTTP/1.1" => {
            Some((javascript, STD_ADAPTER))
        }
        "GET /creche/targets/browser/browser-deployment/creche-adapter.mjs HTTP/1.1" => {
            Some((javascript, BROWSER_ADAPTER))
        }
        _ => None,
    }
}
