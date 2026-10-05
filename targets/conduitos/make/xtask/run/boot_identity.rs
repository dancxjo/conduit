//! Exact proof-profile boot identity and admitted arena validation.
use super::*;

pub(super) fn validate_boot(
    sign: &GuestBootSign,
    qemu_profile: &str,
    arena_bytes: u64,
) -> Result<(), ConduitosError> {
    if sign.schema != "conduit.conduitos.boot-sign/v1"
        || sign.status != "accepted"
        || sign.arch != "x86_64"
        || sign.profile_id.is_empty()
        || sign.build_id.is_empty()
        || sign.image_binding.is_empty()
        || sign.offer_generation == 0
        || sign.limine != LIMINE_VERSION
        || sign.qemu_profile != qemu_profile
        || sign.host_id.len() != 64
        || sign.boot_id.len() != 64
        || sign.memory_regions == 0
        || sign.runtime_arena_bytes != arena_bytes
    {
        return Err(ConduitosError::refusal(
            "invalid-boot-sign",
            format!("boot Sign failed exact validation: {sign:?}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn specimen(profile: &str, arena_bytes: u64) -> GuestBootSign {
        GuestBootSign {
            schema: "conduit.conduitos.boot-sign/v1".into(),
            status: "accepted".into(),
            arch: "x86_64".into(),
            firmware: "x86-bios".into(),
            profile_id: "fixture/profile".into(),
            build_id: "fixture/build".into(),
            image_binding: "fixture/image".into(),
            offer_generation: 1,
            limine: LIMINE_VERSION.into(),
            qemu_profile: profile.into(),
            host_id: "a".repeat(64),
            boot_id: "b".repeat(64),
            memory_regions: 1,
            artifacts: 1,
            framebuffers: 1,
            command_line_bytes: 0,
            runtime_arena_bytes: arena_bytes,
        }
    }
    #[test]
    fn boot_profiles_require_their_exact_emulator_and_arena_bounds() {
        let ordinary_arena = 16 * 1024 * 1024;
        let configuration_arena = conduitos::make::USB_CONFIGURATION_ARENA_BYTES;
        let configuration_profile = conduitos::make::USB_CONFIGURATION_QEMU_PROFILE;
        for (profile, arena) in [
            (QEMU_PROFILE, ordinary_arena),
            (configuration_profile, configuration_arena),
        ] {
            let valid = specimen(profile, arena);
            validate_boot(&valid, profile, arena).unwrap();
            for wrong in [0, arena - 1, arena + 1] {
                assert!(validate_boot(&specimen(profile, wrong), profile, arena).is_err());
            }
        }
        assert!(validate_boot(
            &specimen(QEMU_PROFILE, ordinary_arena),
            configuration_profile,
            configuration_arena
        )
        .is_err());
        assert!(validate_boot(
            &specimen(configuration_profile, configuration_arena),
            QEMU_PROFILE,
            ordinary_arena
        )
        .is_err());
    }
}
