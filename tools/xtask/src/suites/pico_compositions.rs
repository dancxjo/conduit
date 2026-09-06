use crate::{process::Step, proof::ProofClass};

pub const PICO_COMPOSITION_STEPS: &[Step] = &[
    Step::typed(
        "check.thumb.firmware-usb-midi-fixture",
        "Pico W bounded breadboard USB-MIDI fixture Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--bin",
            "conduit-pico-w-midi-fixture",
            "--no-default-features",
            "--features",
            "usb-midi-fixture",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware",
        "Pico W default local composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware-minimal",
        "Pico W minimal local composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--no-default-features",
            "--features",
            "pico-local-minimal",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware-usb-remote",
        "Pico W USB remote sink composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--no-default-features",
            "--features",
            "usb-remote",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware-triple-remote",
        "Pico W triple remote sink composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--no-default-features",
            "--features",
            "triple-remote",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware-wifi-bootstrap",
        "Pico W USB-authorized Wi-Fi bootstrap composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--no-default-features",
            "--features",
            "wifi-bootstrap",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware-r1-control",
        "Pico W R1 three-peer control composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--no-default-features",
            "--features",
            "r1-control",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware-appliance-hello",
        "Pico W finite AP/DHCP/DNS/HTTP Hello composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--no-default-features",
            "--features",
            "appliance-hello",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware-appliance-hil-client",
        "Pico W fixture-only AP/DHCP/DNS/HTTP client probe Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--no-default-features",
            "--features",
            "appliance-hil-client",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
    Step::typed(
        "check.thumb.firmware-bluetooth-line",
        "Pico W finite BLE GATT Line composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
            "--no-default-features",
            "--features",
            "bluetooth-line",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
];

#[cfg(test)]
mod tests {
    use super::PICO_COMPOSITION_STEPS;

    #[test]
    fn session_control_is_optional_and_every_composition_is_checked() {
        let manifest = include_str!("../../../../targets/rp2040/firmware/pico-w-signal/Cargo.toml");
        let firmware =
            include_str!("../../../../targets/rp2040/firmware/pico-w-signal/src/main.rs");
        assert!(manifest.contains("pico-local = [\"session-control\"]"));
        assert!(manifest.contains("pico-local-minimal = []"));
        assert!(manifest.contains("usb-remote = [\"session-control\"]"));
        assert!(manifest.contains("triple-remote = [\"session-control\"]"));
        assert!(manifest.contains("light-switch = [\"session-control\"]"));
        assert!(manifest.contains(
            "wifi-bootstrap = [\"session-control\", \"dep:conduit-net\", \"dep:conduit-rp2040-network-realization\", \"dep:conduit-r1-network-conformance\", \"dep:embassy-net\"]"
        ));
        assert!(manifest.contains("r1-control = [\"wifi-bootstrap\"]"));
        assert!(manifest.contains(
            "appliance-hello = [\"session-control\", \"dep:conduit-net\", \"dep:conduit-rp2040-network-realization\", \"dep:embassy-net\"]"
        ));
        assert!(manifest.contains(
            "appliance-hil-client = [\"session-control\", \"dep:conduit-net\", \"dep:conduit-rp2040-network-realization\", \"dep:embassy-net\"]"
        ));
        let bluetooth_feature = manifest
            .lines()
            .find(|line| line.starts_with("bluetooth-line ="))
            .expect("Bluetooth Line composition feature");
        assert!(bluetooth_feature.contains("dep:conduit-bluetooth"));
        assert!(bluetooth_feature.contains("session-control"));
        assert!(!bluetooth_feature.contains("dep:conduit-wire"));
        assert!(manifest.contains(
            "conduit-wire = { path = \"../../../../architecture/wire\", default-features = false, optional = true }"
        ));
        assert!(firmware.contains(
            "#[cfg(all(feature = \"session-control\", not(feature = \"light-switch\")))]\nmod usb_link;"
        ));
        assert!(firmware.contains("#[cfg(feature = \"pico-local-minimal\")]"));
        assert!(manifest.contains("usb-midi-fixture = [\"dep:embassy-futures\"]"));
        assert_eq!(PICO_COMPOSITION_STEPS.len(), 10);
    }

    #[test]
    fn appliance_access_point_is_explicitly_open_and_has_no_credential_path() {
        let appliance =
            include_str!("../../../../targets/rp2040/firmware/pico-w-signal/src/appliance.rs");
        assert!(appliance.contains("start_ap_open"));
        assert!(!appliance.contains("start_ap_wpa2"));
        assert!(!appliance.contains("password"));
        assert!(!appliance.contains("credential"));
    }

    #[test]
    fn appliance_hil_client_usb_serial_fits_the_control_buffer() {
        let usb = include_str!("../../../../targets/rp2040/firmware/pico-w-signal/src/usb.rs");
        let serial = "conduit-pico-hil-client";
        assert!(usb.contains(serial));
        assert!(serial.encode_utf16().count() * 2 + 2 <= 64);
    }
}
