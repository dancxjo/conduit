//! Exact Crèche application-package and compatibility asset inventory.

use conduit_browser_host::application_package;

pub(super) const DOCUMENT: &[u8] = include_bytes!("../../assets/creche.html");
pub(super) const SCRIPT: &[u8] = include_bytes!("../../assets/creche.mjs");
pub(super) const LIFECYCLE: &[u8] = include_bytes!("../../assets/creche-lifecycle.mjs");
pub(super) const PHYSICAL: &[u8] = include_bytes!("../../assets/creche-physical.mjs");
pub(super) const TARGET_CATALOG: &[u8] = include_bytes!("../../assets/creche-target-catalog.mjs");
pub(super) const GRADUATION: &[u8] = include_bytes!("../../assets/creche-graduation.mjs");
pub(super) const STYLE: &[u8] = include_bytes!("../../assets/creche.css");
pub(super) const PICO_ARTIFACT: &[u8] =
    include_bytes!("../../assets/artifacts/pico-w-signal-pico-local.uf2");
pub(super) const PICO_ARTIFACT_MANIFEST: &[u8] =
    include_bytes!("../../assets/artifacts/pico-w-signal-pico-local.json");
pub(super) const AVR_ADAPTER: &[u8] =
    include_bytes!("../../../../avr/browser-deployment/creche-adapter.mjs");
pub(super) const AVR_IMAGE: &[u8] = include_bytes!("../../../../avr/browser-deployment/image.mjs");
pub(super) const RP2040_DEPLOYMENT: &[u8] =
    include_bytes!("../../../../rp2040/browser-deployment/index.mjs");
pub(super) const RP2040_DEPLOYMENT_ORCHESTRATOR: &[u8] =
    include_bytes!("../../../../rp2040/browser-deployment/deployment.mjs");
pub(super) const RP2040_PICOBOOT: &[u8] =
    include_bytes!("../../../../rp2040/browser-deployment/picoboot.mjs");
pub(super) const RP2040_UF2: &[u8] =
    include_bytes!("../../../../rp2040/browser-deployment/uf2.mjs");
pub(super) const RP2040_BOOTSEL: &[u8] =
    include_bytes!("../../../../rp2040/browser-deployment/bootsel.mjs");
pub(super) const RP2040_SPAWN: &[u8] =
    include_bytes!("../../../../rp2040/browser-deployment/spawn.mjs");
pub(super) const RP2040_FABRICATION: &[u8] =
    include_bytes!("../../../../rp2040/browser-deployment/fabrication.mjs");
pub(super) const RP2040_ADAPTER: &[u8] =
    include_bytes!("../../../../rp2040/browser-deployment/creche-adapter.mjs");
pub(super) const ESP32_DEPLOYMENT: &[u8] =
    include_bytes!("../../../../esp32/browser-deployment/index.mjs");
pub(super) const ESP32_DEPLOYMENT_ORCHESTRATOR: &[u8] =
    include_bytes!("../../../../esp32/browser-deployment/deployment.mjs");
pub(super) const ESP32_IMAGE: &[u8] =
    include_bytes!("../../../../esp32/browser-deployment/image.mjs");
pub(super) const ESP32_MD5: &[u8] = include_bytes!("../../../../esp32/browser-deployment/md5.mjs");
pub(super) const ESP32_RESET: &[u8] =
    include_bytes!("../../../../esp32/browser-deployment/reset.mjs");
pub(super) const ESP32_ROM_LOADER: &[u8] =
    include_bytes!("../../../../esp32/browser-deployment/rom-loader.mjs");
pub(super) const ESP32_SLIP: &[u8] =
    include_bytes!("../../../../esp32/browser-deployment/slip.mjs");
pub(super) const ESP32_ADAPTER: &[u8] =
    include_bytes!("../../../../esp32/browser-deployment/creche-adapter.mjs");
pub(super) const RASPBERRY_PI_ADAPTER: &[u8] =
    include_bytes!("../../../../raspberry-pi/browser-deployment/creche-adapter.mjs");
pub(super) const RASPBERRY_PI_IMAGE: &[u8] =
    include_bytes!("../../../../raspberry-pi/browser-deployment/image.mjs");
pub(super) const CONDUITOS_ADAPTER: &[u8] =
    include_bytes!("../../../../conduitos/browser-deployment/creche-adapter.mjs");
pub(super) const CONDUITOS_IMAGE: &[u8] =
    include_bytes!("../../../../conduitos/browser-deployment/image.mjs");

const TEMPLATE: &[u8] = include_bytes!("../../assets/creche.application.template.json");

pub(super) fn build_manifest(runtime: &[u8]) -> Result<Vec<u8>, String> {
    application_package::build_manifest(TEMPLATE, |path| resource(path, runtime))
}

pub(super) fn resource<'a>(path: &str, runtime: &'a [u8]) -> Option<&'a [u8]> {
    if let Some(bytes) = super::existing_computer_assets::resource(path) {
        return Some(bytes);
    }
    match path {
        "creche.mjs" => Some(SCRIPT),
        "application-presentation.mjs" => Some(super::APPLICATION_PRESENTATION),
        "browser-host-bootstrap.mjs" => Some(super::HOST_BOOTSTRAP),
        "browser-host-membership.mjs" => Some(super::HOST_MEMBERSHIP),
        "creche-lifecycle.mjs" => Some(LIFECYCLE),
        "creche-physical.mjs" => Some(PHYSICAL),
        "creche-target-catalog.mjs" => Some(TARGET_CATALOG),
        "creche-graduation.mjs" => Some(GRADUATION),
        "device-base.mjs" => Some(super::DEVICE_BASE),
        "usb-device-base.mjs" => Some(super::USB_DEVICE_BASE),
        "targets/avr/browser-deployment/creche-adapter.mjs" => Some(AVR_ADAPTER),
        "targets/avr/browser-deployment/image.mjs" => Some(AVR_IMAGE),
        "targets/conduitos/browser-deployment/creche-adapter.mjs" => Some(CONDUITOS_ADAPTER),
        "targets/conduitos/browser-deployment/image.mjs" => Some(CONDUITOS_IMAGE),
        "targets/esp32/browser-deployment/creche-adapter.mjs" => Some(ESP32_ADAPTER),
        "targets/esp32/browser-deployment/deployment.mjs" => Some(ESP32_DEPLOYMENT_ORCHESTRATOR),
        "targets/esp32/browser-deployment/image.mjs" => Some(ESP32_IMAGE),
        "targets/esp32/browser-deployment/md5.mjs" => Some(ESP32_MD5),
        "targets/esp32/browser-deployment/reset.mjs" => Some(ESP32_RESET),
        "targets/esp32/browser-deployment/rom-loader.mjs" => Some(ESP32_ROM_LOADER),
        "targets/esp32/browser-deployment/slip.mjs" => Some(ESP32_SLIP),
        "targets/raspberry-pi/browser-deployment/creche-adapter.mjs" => Some(RASPBERRY_PI_ADAPTER),
        "targets/raspberry-pi/browser-deployment/image.mjs" => Some(RASPBERRY_PI_IMAGE),
        "targets/rp2040/browser-deployment/creche-adapter.mjs" => Some(RP2040_ADAPTER),
        "targets/rp2040/browser-deployment/deployment.mjs" => Some(RP2040_DEPLOYMENT_ORCHESTRATOR),
        "targets/rp2040/browser-deployment/fabrication.mjs" => Some(RP2040_FABRICATION),
        "targets/rp2040/browser-deployment/picoboot.mjs" => Some(RP2040_PICOBOOT),
        "targets/rp2040/browser-deployment/spawn.mjs" => Some(RP2040_SPAWN),
        "targets/rp2040/browser-deployment/uf2.mjs" => Some(RP2040_UF2),
        "creche.css" => Some(STYLE),
        "runtime.wasm" => Some(runtime),
        _ => None,
    }
}
