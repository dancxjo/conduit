//! Target-owned browser mechanisms exposed by the bare browser Host proof page.

const RP2040_INDEX: &[u8] = include_bytes!("../../../../rp2040/deployment/browser/index.mjs");
const RP2040_DEPLOYMENT: &[u8] =
    include_bytes!("../../../../rp2040/deployment/browser/deployment.mjs");
const RP2040_PICOBOOT: &[u8] = include_bytes!("../../../../rp2040/deployment/browser/picoboot.mjs");
const RP2040_UF2: &[u8] = include_bytes!("../../../../rp2040/deployment/browser/uf2.mjs");
const RP2040_BOOTSEL: &[u8] = include_bytes!("../../../../rp2040/deployment/browser/bootsel.mjs");
const RP2040_SPAWN: &[u8] = include_bytes!("../../../../rp2040/deployment/browser/spawn.mjs");
const RP2040_FABRICATION: &[u8] =
    include_bytes!("../../../../rp2040/deployment/browser/fabrication.mjs");
const RP2040_PICO_LOCAL_MANIFEST: &[u8] =
    include_bytes!("../../assets/artifacts/pico-w-signal-pico-local.json");
const RP2040_PICO_LOCAL_UF2: &[u8] =
    include_bytes!("../../assets/artifacts/pico-w-signal-pico-local.uf2");
const ESP32_INDEX: &[u8] = include_bytes!("../../../../esp32/deployment/browser/index.mjs");
const ESP32_DEPLOYMENT: &[u8] =
    include_bytes!("../../../../esp32/deployment/browser/deployment.mjs");
const ESP32_IMAGE: &[u8] = include_bytes!("../../../../esp32/deployment/browser/image.mjs");
const ESP32_MD5: &[u8] = include_bytes!("../../../../esp32/deployment/browser/md5.mjs");
const ESP32_RESET: &[u8] = include_bytes!("../../../../esp32/deployment/browser/reset.mjs");
const ESP32_ROM_LOADER: &[u8] =
    include_bytes!("../../../../esp32/deployment/browser/rom-loader.mjs");
const ESP32_SLIP: &[u8] = include_bytes!("../../../../esp32/deployment/browser/slip.mjs");

pub(super) fn response(request: Option<&str>) -> Option<(&'static str, &'static [u8])> {
    let javascript = "text/javascript; charset=utf-8";
    let body: &[u8] = match request? {
        "GET /targets/rp2040/browser-deployment/index.mjs HTTP/1.1" => RP2040_INDEX,
        "GET /targets/rp2040/browser-deployment/deployment.mjs HTTP/1.1" => RP2040_DEPLOYMENT,
        "GET /targets/rp2040/browser-deployment/picoboot.mjs HTTP/1.1" => RP2040_PICOBOOT,
        "GET /targets/rp2040/browser-deployment/uf2.mjs HTTP/1.1" => RP2040_UF2,
        "GET /targets/rp2040/browser-deployment/bootsel.mjs HTTP/1.1" => RP2040_BOOTSEL,
        "GET /targets/rp2040/browser-deployment/spawn.mjs HTTP/1.1" => RP2040_SPAWN,
        "GET /targets/rp2040/browser-deployment/fabrication.mjs HTTP/1.1" => RP2040_FABRICATION,
        "GET /creche/artifacts/pico-w-signal-pico-local.json HTTP/1.1" => {
            return Some((
                "application/json; charset=utf-8",
                RP2040_PICO_LOCAL_MANIFEST,
            ));
        }
        "GET /creche/artifacts/pico-w-signal-pico-local.uf2 HTTP/1.1" => {
            return Some(("application/octet-stream", RP2040_PICO_LOCAL_UF2));
        }
        "GET /targets/esp32/browser-deployment/index.mjs HTTP/1.1" => ESP32_INDEX,
        "GET /targets/esp32/browser-deployment/deployment.mjs HTTP/1.1" => ESP32_DEPLOYMENT,
        "GET /targets/esp32/browser-deployment/image.mjs HTTP/1.1" => ESP32_IMAGE,
        "GET /targets/esp32/browser-deployment/md5.mjs HTTP/1.1" => ESP32_MD5,
        "GET /targets/esp32/browser-deployment/reset.mjs HTTP/1.1" => ESP32_RESET,
        "GET /targets/esp32/browser-deployment/rom-loader.mjs HTTP/1.1" => ESP32_ROM_LOADER,
        "GET /targets/esp32/browser-deployment/slip.mjs HTTP/1.1" => ESP32_SLIP,
        _ => return None,
    };
    Some((javascript, body))
}
