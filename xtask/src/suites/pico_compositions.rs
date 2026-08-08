use crate::{process::Step, proof::ProofClass};

pub const PICO_COMPOSITION_STEPS: &[Step] = &[
    Step::typed(
        "check.thumb.firmware",
        "Pico W default local composition Thumb check",
        "cargo",
        &[
            "check",
            "--manifest-path",
            "firmware/conduit-pico-w-signal/Cargo.toml",
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
            "firmware/conduit-pico-w-signal/Cargo.toml",
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
            "firmware/conduit-pico-w-signal/Cargo.toml",
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
            "firmware/conduit-pico-w-signal/Cargo.toml",
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
];

#[cfg(test)]
mod tests {
    use super::PICO_COMPOSITION_STEPS;

    #[test]
    fn session_control_is_optional_and_every_composition_is_checked() {
        let manifest = include_str!("../../../firmware/conduit-pico-w-signal/Cargo.toml");
        let firmware = include_str!("../../../firmware/conduit-pico-w-signal/src/main.rs");
        assert!(manifest.contains("pico-local = [\"session-control\"]"));
        assert!(manifest.contains("pico-local-minimal = []"));
        assert!(manifest.contains("usb-remote = [\"session-control\"]"));
        assert!(manifest.contains("triple-remote = [\"session-control\"]"));
        assert!(manifest.contains(
            "conduit-wire = { path = \"../../crates/conduit-wire\", default-features = false, optional = true }"
        ));
        assert!(firmware.contains("#[cfg(feature = \"session-control\")]\nmod usb_link;"));
        assert!(firmware.contains("#[cfg(feature = \"pico-local-minimal\")]"));
        assert_eq!(PICO_COMPOSITION_STEPS.len(), 4);
    }
}
