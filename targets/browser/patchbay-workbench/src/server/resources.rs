use super::{PatchbayHtmlServer, ServerError};
use conduit_browser_host::application_package;

pub(super) const INDEX: &[u8] = include_bytes!("../../assets/index.html");
const APPLICATION_TEMPLATE: &[u8] =
    include_bytes!("../../assets/patchbay.application.template.json");
pub(super) const APPLICATION_LOADER: &[u8] =
    include_bytes!("../../../../../targets/browser/host/assets/browser-application-loader.mjs");
pub(super) const APPLICATION_STORAGE: &[u8] =
    include_bytes!("../../../../../targets/browser/host/assets/browser-application-storage.mjs");
// Matches the runtime resource bound in patchbay.application.template.json.
pub(super) const MAX_BROWSER_WASM_BYTES: usize = 20 * 1024 * 1024;
pub(super) const EMPTY_BROWSER_WASM: &[u8] = b"\0asm\x01\0\0\0";

impl PatchbayHtmlServer {
    pub(super) fn application_resource(&self, path: &str) -> Option<&[u8]> {
        crate::application_resources::resource(
            path,
            self.browser_wasm.as_deref().unwrap_or(EMPTY_BROWSER_WASM),
            &self.theme_css,
        )
        .map(|(_, bytes)| bytes)
    }

    pub(super) fn application_manifest(&self) -> Result<Vec<u8>, ServerError> {
        application_package::build_manifest(APPLICATION_TEMPLATE, |path| {
            self.application_resource(path)
        })
        .map_err(ServerError::ApplicationPackage)
    }
}
