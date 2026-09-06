//! Exact firmware composition identity selected by Cargo features.

pub(super) fn firmware_mode() -> &'static str {
    if std::env::var_os("CARGO_FEATURE_INDICATOR_RESOURCE").is_some() {
        "indicator-resource"
    } else if std::env::var_os("CARGO_FEATURE_USB_MIDI_FIXTURE").is_some() {
        "usb-midi-fixture"
    } else if std::env::var_os("CARGO_FEATURE_PETE_CAPSTONE").is_some() {
        "pete-capstone"
    } else if std::env::var_os("CARGO_FEATURE_APPLIANCE_HELLO").is_some() {
        "appliance-hello"
    } else if std::env::var_os("CARGO_FEATURE_BLUETOOTH_LINE").is_some() {
        "bluetooth-line"
    } else if std::env::var_os("CARGO_FEATURE_DISTRIBUTED_LENIA").is_some() {
        "distributed-lenia"
    } else if std::env::var_os("CARGO_FEATURE_APPLIANCE_HIL_CLIENT").is_some() {
        "appliance-hil-client"
    } else if std::env::var_os("CARGO_FEATURE_R1_CONTROL").is_some() {
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
