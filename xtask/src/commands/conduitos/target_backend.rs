use conduit_host_fabrication::{BuildManifest, HostBounds};

use super::{target_lowering, ConduitosArch, ConduitosError};

pub(super) type LowerTarget =
    fn(&BuildManifest) -> Result<target_lowering::TargetBuildInputs, ConduitosError>;

#[derive(Debug, Clone)]
pub(crate) struct TargetBackend {
    pub target: &'static str,
    pub arch: ConduitosArch,
    pub build_maxima: HostBounds,
    pub kernel_file: &'static str,
    pub kernel_role: &'static str,
    pub image_file: &'static str,
    pub image_role: &'static str,
    pub architecture: &'static str,
    pub machine: &'static str,
    pub firmware: &'static str,
    pub boot_entry: &'static str,
    pub machine_boot_proof: bool,
    lower: LowerTarget,
}

const HOSTED_MAXIMA: HostBounds = HostBounds {
    static_memory_bytes: 512 * 1024 * 1024,
    heap_arena_bytes: 512 * 1024 * 1024,
    queue_items: 1_048_576,
    buffered_bytes: 512 * 1024 * 1024,
    active_instances: 65_536,
    operation_slots: 65_536,
    timer_slots: 65_536,
    line_sessions: 65_536,
    evidence_items: 1_048_576,
};

const BACKENDS: &[TargetBackend] = &[
    TargetBackend {
        target: "conduitos/x86_64/pc",
        arch: ConduitosArch::X86_64,
        build_maxima: HOSTED_MAXIMA,
        kernel_file: "conduitos-kernel.elf",
        kernel_role: "freestanding-kernel",
        image_file: "conduitos-x86_64.iso",
        image_role: "final-bootable-image",
        architecture: "x86_64",
        machine: "q35",
        firmware: "OVMF_CODE.fd",
        boot_entry: "BOOTX64.EFI",
        machine_boot_proof: true,
        lower: target_lowering::lower_x86_64_pc,
    },
    TargetBackend {
        target: "conduitos/aarch64/virt",
        arch: ConduitosArch::Aarch64,
        build_maxima: HOSTED_MAXIMA,
        kernel_file: "conduitos-kernel.elf",
        kernel_role: "freestanding-kernel",
        image_file: "conduitos-aarch64.iso",
        image_role: "final-bootable-image",
        architecture: "aarch64",
        machine: "virt",
        firmware: "QEMU_EFI.fd",
        boot_entry: "BOOTAA64.EFI",
        machine_boot_proof: true,
        lower: target_lowering::lower_aarch64_virt,
    },
];

impl TargetBackend {
    pub(super) fn lower(
        &self,
        manifest: &BuildManifest,
    ) -> Result<target_lowering::TargetBuildInputs, ConduitosError> {
        if manifest.target != self.target {
            return Err(ConduitosError::refusal(
                "target-backend-manifest-mismatch",
                format!(
                    "backend {} cannot lower manifest target {}",
                    self.target, manifest.target
                ),
            ));
        }
        (self.lower)(manifest)
    }

    pub(crate) fn require_machine_boot(&self) -> Result<(), ConduitosError> {
        if self.machine_boot_proof {
            Ok(())
        } else {
            Err(ConduitosError::refusal(
                "unsupported-profile-boot",
                self.target.to_owned(),
            ))
        }
    }
}

pub(crate) fn find(target: &str) -> Option<&'static TargetBackend> {
    find_in(BACKENDS, target).ok()
}

pub(crate) fn select(target: &str) -> Result<&'static TargetBackend, ConduitosError> {
    find(target)
        .ok_or_else(|| ConduitosError::refusal("unsupported-profile-target", target.to_owned()))
}

fn find_in<'a>(
    backends: &'a [TargetBackend],
    target: &str,
) -> Result<&'a TargetBackend, &'static str> {
    let mut found = None;
    for backend in backends {
        if backend.target == target {
            if found.is_some() {
                return Err("duplicate-target-backend");
            }
            found = Some(backend);
        }
    }
    found.ok_or("unknown-target-backend")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_fabrication::{
        build_host_image, BuildInputs, FabricationCatalog, HostProfile,
    };

    fn native_manifest() -> BuildManifest {
        let profile: HostProfile = serde_json::from_str(include_str!(
            "../../../../profiles/hosts/conduitos-native.profile.json"
        ))
        .unwrap();
        build_host_image(
            profile,
            &FabricationCatalog::canonical(),
            &BuildInputs {
                source_identity: "test-source".into(),
                toolchain_identity: "test-toolchain".into(),
                toolchain_available: true,
                maxima: HOSTED_MAXIMA.clone(),
            },
        )
        .unwrap()
        .0
        .manifest
    }

    fn fake_lower(_: &BuildManifest) -> Result<target_lowering::TargetBuildInputs, ConduitosError> {
        unreachable!("selection does not invoke lowering")
    }

    fn fake(target: &'static str) -> TargetBackend {
        TargetBackend {
            target,
            arch: ConduitosArch::Armv6,
            build_maxima: HOSTED_MAXIMA,
            kernel_file: "fake-kernel",
            kernel_role: "fake-kernel-role",
            image_file: "fake-image",
            image_role: "fake-image-role",
            architecture: "fake",
            machine: "fake",
            firmware: "fake",
            boot_entry: "fake",
            machine_boot_proof: false,
            lower: fake_lower,
        }
    }

    #[test]
    fn a_test_only_backend_plugs_into_the_registry_algorithm() {
        let backends = [fake("test/fake")];
        let backend = find_in(&backends, "test/fake").unwrap();
        assert_eq!(backend.image_file, "fake-image");
        assert!(backend
            .require_machine_boot()
            .unwrap_err()
            .to_string()
            .contains("unsupported-profile-boot"));
    }

    #[test]
    fn duplicate_and_unknown_backend_keys_refuse_distinctly() {
        let backends = [fake("test/duplicate"), fake("test/duplicate")];
        assert_eq!(
            find_in(&backends, "test/duplicate").unwrap_err(),
            "duplicate-target-backend"
        );
        assert_eq!(
            find_in(&backends, "test/absent").unwrap_err(),
            "unknown-target-backend"
        );
    }

    #[test]
    fn canonical_backends_pin_existing_artifact_and_boot_truth() {
        let x86 = select("conduitos/x86_64/pc").unwrap();
        assert_eq!(
            (
                x86.kernel_file,
                x86.image_file,
                x86.architecture,
                x86.machine,
                x86.firmware,
                x86.boot_entry,
            ),
            (
                "conduitos-kernel.elf",
                "conduitos-x86_64.iso",
                "x86_64",
                "q35",
                "OVMF_CODE.fd",
                "BOOTX64.EFI",
            )
        );
        let aarch64 = select("conduitos/aarch64/virt").unwrap();
        assert_eq!(
            (
                aarch64.image_file,
                aarch64.architecture,
                aarch64.machine,
                aarch64.firmware,
                aarch64.boot_entry,
            ),
            (
                "conduitos-aarch64.iso",
                "aarch64",
                "virt",
                "QEMU_EFI.fd",
                "BOOTAA64.EFI",
            )
        );
    }

    #[test]
    fn unknown_target_and_mismatched_backend_manifest_refuse_exactly() {
        assert!(select("conduitos/unknown")
            .unwrap_err()
            .to_string()
            .contains("unsupported-profile-target"));
        assert!(select("conduitos/aarch64/virt")
            .unwrap()
            .lower(&native_manifest())
            .unwrap_err()
            .to_string()
            .contains("target-backend-manifest-mismatch"));
    }
}
