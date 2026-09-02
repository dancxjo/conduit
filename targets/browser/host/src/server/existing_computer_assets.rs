//! Bounded release and native-package assets shared by Crèche targets.

pub(super) const SPORE_DOWNLOAD: &[u8] = include_bytes!("../../assets/creche-spore-bundle.mjs");
const NATIVE_ZIP: &[u8] = include_bytes!("../../assets/creche-native-zip.mjs");
const NATIVE_DISK: &[u8] = include_bytes!("../../assets/creche-native-disk.mjs");
const RELEASE_BUNDLE: &[u8] = include_bytes!("../../assets/creche-release-bundle.mjs");
const EXISTING_COMPUTER: &[u8] = include_bytes!("../../assets/creche-existing-computer.mjs");
const STD_ADAPTER: &[u8] = include_bytes!("../../../../std/browser-deployment/creche-adapter.mjs");
const BROWSER_ADAPTER: &[u8] = include_bytes!("../../browser-deployment/creche-adapter.mjs");
const ORANGE_PI_ADAPTER: &[u8] =
    include_bytes!("../../../../orange-pi/browser-deployment/creche-adapter.mjs");
const ORANGE_PI_IMAGE: &[u8] = include_bytes!("../../../../orange-pi/browser-deployment/image.mjs");

pub(super) fn resource(path: &str) -> Option<&'static [u8]> {
    match path {
        "creche-spore-bundle.mjs" => Some(SPORE_DOWNLOAD),
        "creche-native-zip.mjs" => Some(NATIVE_ZIP),
        "creche-native-disk.mjs" => Some(NATIVE_DISK),
        "creche-release-bundle.mjs" => Some(RELEASE_BUNDLE),
        "creche-existing-computer.mjs" => Some(EXISTING_COMPUTER),
        "targets/std/browser-deployment/creche-adapter.mjs" => Some(STD_ADAPTER),
        "targets/browser/browser-deployment/creche-adapter.mjs" => Some(BROWSER_ADAPTER),
        "targets/orange-pi/browser-deployment/creche-adapter.mjs" => Some(ORANGE_PI_ADAPTER),
        "targets/orange-pi/browser-deployment/image.mjs" => Some(ORANGE_PI_IMAGE),
        _ => None,
    }
}

pub(super) fn response(request: Option<&str>) -> Option<(&'static str, &'static [u8])> {
    let javascript = "text/javascript; charset=utf-8";
    match request? {
        "GET /creche/creche-spore-bundle.mjs HTTP/1.1" => Some((javascript, SPORE_DOWNLOAD)),
        "GET /creche/creche-native-zip.mjs HTTP/1.1" => Some((javascript, NATIVE_ZIP)),
        "GET /creche/creche-native-disk.mjs HTTP/1.1" => Some((javascript, NATIVE_DISK)),
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
        "GET /creche/targets/orange-pi/browser-deployment/creche-adapter.mjs HTTP/1.1" => {
            Some((javascript, ORANGE_PI_ADAPTER))
        }
        "GET /creche/targets/orange-pi/browser-deployment/image.mjs HTTP/1.1" => {
            Some((javascript, ORANGE_PI_IMAGE))
        }
        _ => None,
    }
}
