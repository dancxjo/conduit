//! Proof-specific Boot disposition: no product input offer is initialized.
use super::{refusal, ConduitosError, GuestBootSign};

pub(super) fn validate_boot(sign: &GuestBootSign) -> Result<(), ConduitosError> {
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
        || sign.qemu_profile != conduitos::make::USB_ENDPOINT_QEMU_PROFILE
        || !exact_id(&sign.host_id)
        || !exact_id(&sign.boot_id)
        || sign.memory_regions == 0
        || sign.runtime_arena_bytes != 16 * 1024 * 1024
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
    fn endpoint_boot_refuses_wrong_profile_budget_and_invented_product_offer() {
        validate_boot(&specimen()).unwrap();
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
            assert!(validate_boot(&candidate).is_err(), "accepted {field}");
        }
    }
}
