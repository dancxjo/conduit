//! Exact firmware composition identity selected by Cargo features.

pub(super) fn firmware_mode() -> &'static str {
    if std::env::var_os("CARGO_FEATURE_R1_CONTROL").is_some() {
        "r1-control"
    } else if std::env::var_os("CARGO_FEATURE_WIFI_BOOTSTRAP").is_some() {
        "wifi-bootstrap"
    } else if std::env::var_os("CARGO_FEATURE_TRIPLE_REMOTE").is_some() {
        "triple-remote"
    } else if std::env::var_os("CARGO_FEATURE_USB_REMOTE").is_some() {
        "usb-remote"
    } else if std::env::var_os("CARGO_FEATURE_PICO_LOCAL_MINIMAL").is_some() {
        "pico-local-minimal"
    } else {
        "pico-local"
    }
}
