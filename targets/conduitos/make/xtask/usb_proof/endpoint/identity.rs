//! Proof-specific Boot disposition: no product input offer is initialized.
use super::{ConduitosError, GuestBootSign, refusal};

pub(super) fn validate_boot_mode(
    sign: &GuestBootSign,
    mode: super::ProofMode,
) -> Result<(), ConduitosError> {
    let profile = mode.qemu_profile();
    let arena = mode.arena_bytes();
    let exact_id =
        |value: &str| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if sign.schema != "conduit.conduitos.boot-sign/v1"
        || sign.status != "accepted"
        || sign.arch != "x86_64"
        || sign.firmware != "x86-bios"
        || sign.profile_id.is_empty()
        || sign.build_id.is_empty()
        || sign.image_binding.is_empty()
        || sign.offer_generation != 0
        || sign.limine != "12.5.2"
        || sign.qemu_profile != profile
        || !exact_id(&sign.host_id)
        || !exact_id(&sign.boot_id)
        || sign.memory_regions == 0
        || sign.runtime_arena_bytes != arena
    {
        return Err(refusal("endpoint-proof-boot", format!("{sign:?}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specimen() -> GuestBootSign {
        GuestBootSign {
            schema: "conduit.conduitos.boot-sign/v1".into(),
            status: "accepted".into(),
            arch: "x86_64".into(),
            firmware: "x86-bios".into(),
            profile_id: "fixture/profile".into(),
            build_id: "fixture/build".into(),
            image_binding: "fixture/image".into(),
            offer_generation: 0,
            limine: "12.5.2".into(),
            qemu_profile: conduitos::make::USB_ENDPOINT_QEMU_PROFILE.into(),
            host_id: "a".repeat(64),
            boot_id: "b".repeat(64),
            memory_regions: 19,
            artifacts: 1,
            framebuffers: 1,
            command_line_bytes: 38,
            runtime_arena_bytes: 16 * 1024 * 1024,
        }
    }

    #[test]
    fn hid_boot_requires_its_exact_profile_and_preparation_budget() {
        let mut sign = specimen();
        assert!(validate_boot_mode(&sign, super::super::ProofMode::Keyboard).is_err());
        sign.qemu_profile = conduitos::make::USB_HID_ENDPOINT_QEMU_PROFILE.into();
        assert!(validate_boot_mode(&sign, super::super::ProofMode::Keyboard).is_err());
        sign.runtime_arena_bytes = conduitos::make::USB_HID_ENDPOINT_ARENA_BYTES;
        validate_boot_mode(&sign, super::super::ProofMode::Keyboard).unwrap();
        assert!(validate_boot_mode(&sign, super::super::ProofMode::Raw).is_err());
        sign.runtime_arena_bytes -= 1;
        assert!(validate_boot_mode(&sign, super::super::ProofMode::Keyboard).is_err());
        sign.qemu_profile = conduitos::make::USB_HID_MOUSE_QEMU_PROFILE.into();
        sign.runtime_arena_bytes = conduitos::make::USB_HID_MOUSE_ARENA_BYTES;
        validate_boot_mode(&sign, super::super::ProofMode::Mouse).unwrap();
        assert!(validate_boot_mode(&sign, super::super::ProofMode::Keyboard).is_err());
        sign.runtime_arena_bytes = conduitos::make::USB_HID_ENDPOINT_ARENA_BYTES;
        assert!(validate_boot_mode(&sign, super::super::ProofMode::Mouse).is_err());
    }

    #[test]
    fn mouse_boot_refuses_substituted_keyboard_profile() {
        use super::super::ProofMode;
        let mut sign = specimen();
        sign.runtime_arena_bytes = ProofMode::Mouse.arena_bytes();
        sign.qemu_profile = ProofMode::Keyboard.qemu_profile().into();
        assert!(validate_boot_mode(&sign, ProofMode::Mouse).is_err());
        sign.qemu_profile = ProofMode::Mouse.qemu_profile().into();
        validate_boot_mode(&sign, ProofMode::Mouse).unwrap();
        assert!(validate_boot_mode(&sign, ProofMode::Keyboard).is_err());
    }

    #[test]
    fn endpoint_boot_refuses_wrong_profile_budget_and_invented_product_offer() {
        validate_boot_mode(&specimen(), super::super::ProofMode::Raw).unwrap();
        for (field, value) in [
            ("schema", serde_json::json!("wrong")),
            ("status", serde_json::json!("refused")),
            ("arch", serde_json::json!("aarch64")),
            ("firmware", serde_json::json!("uefi")),
            ("profile_id", serde_json::json!("")),
            ("build_id", serde_json::json!("")),
            ("image_binding", serde_json::json!("")),
            ("offer_generation", serde_json::json!(1)),
            ("limine", serde_json::json!("wrong")),
            ("qemu_profile", serde_json::json!("ordinary-product")),
            ("host_id", serde_json::json!("z".repeat(64))),
            ("boot_id", serde_json::json!("b".repeat(63))),
            ("memory_regions", serde_json::json!(0)),
            (
                "runtime_arena_bytes",
                serde_json::json!(16 * 1024 * 1024 - 1),
            ),
        ] {
            let mut candidate = serde_json::to_value(specimen()).unwrap();
            candidate[field] = value;
            let candidate = serde_json::from_value(candidate).unwrap();
            assert!(
                validate_boot_mode(&candidate, super::super::ProofMode::Raw).is_err(),
                "accepted {field}"
            );
        }
    }
}
