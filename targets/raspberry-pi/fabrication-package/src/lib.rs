use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationOffer, PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub const B_PLUS_TARGET: &str = "conduitos/armv6/raspberry-pi-model-b-plus-v1.2";
pub const ZERO_TARGET: &str = "conduitos/armv6/raspberry-pi-zero-v1";
pub const ZERO_W_TARGET: &str = "conduitos/armv6/raspberry-pi-zero-w-v1.1";
pub const ZERO_WH_TARGET: &str = "conduitos/armv6/raspberry-pi-zero-wh-v1.1";
pub const ZERO_2_W_TARGET: &str = "std/aarch64/raspberry-pi-zero-2-w-rev-1.0";
pub const ZERO_2_WH_TARGET: &str = "std/aarch64/raspberry-pi-zero-2-wh-rev-1.0";
pub const RASPBERRY_PI_OS_TARGET: &str = "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb";

pub struct RaspberryPiFabricationPackage;

fn bare_metal_target(label: &str, machine: &str) -> TargetDescriptor {
    TargetDescriptor {
        label: label.into(),
        family: "conduitos".into(),
        architecture: "armv6".into(),
        machine: machine.into(),
        board: Some(machine.into()),
        os: None,
        host_core: "host-core/conduitos@1".into(),
        presenter: None,
        host_operations: Vec::new(),
        toolchain_identity: "rustc:stable+armv6-none-eabi+rust-lld".into(),
        builder_adapter: "conduit-host-raspberry-pi/build-sd-image@1".into(),
        deployment_adapter: Some("conduit-host-raspberry-pi/flash-removable-media@1".into()),
        outputs: vec![SporeOutputKind::SdImage],
        default_output: SporeOutputKind::SdImage,
        post_build_actions: vec![PostBuildAction::Flash, PostBuildAction::Boot],
        fabrication_descriptors: Vec::new(),
        maxima: HostBounds {
            static_memory_bytes: 512 * 1024 * 1024,
            heap_arena_bytes: 512 * 1024 * 1024,
            queue_items: 4096,
            buffered_bytes: 64 * 1024 * 1024,
            active_instances: 4096,
            operation_slots: 4096,
            timer_slots: 4096,
            line_sessions: 1024,
            evidence_items: 65_536,
        },
    }
}

fn raspberry_pi_os_target(
    label: &str,
    machine: &str,
    maximum_memory_bytes: u64,
) -> TargetDescriptor {
    TargetDescriptor {
        label: label.into(),
        family: "std".into(),
        architecture: "aarch64".into(),
        machine: machine.into(),
        board: Some(machine.into()),
        os: Some("raspberry-pi-os-bookworm-64".into()),
        host_core: "host-core/std@1".into(),
        presenter: None,
        host_operations: Vec::new(),
        toolchain_identity:
            "rustc:stable+aarch64-unknown-linux-gnu+gcc-aarch64-linux-gnu+libc6-dev-arm64-cross"
                .into(),
        builder_adapter: "conduit-host-raspberry-pi/build-raspios-native@1".into(),
        deployment_adapter: Some("conduit-host-raspberry-pi/install-raspios-package@1".into()),
        outputs: vec![SporeOutputKind::NativeBundle],
        default_output: SporeOutputKind::NativeBundle,
        post_build_actions: vec![PostBuildAction::Launch],
        fabrication_descriptors: Vec::new(),
        maxima: HostBounds {
            static_memory_bytes: maximum_memory_bytes,
            heap_arena_bytes: maximum_memory_bytes,
            queue_items: 262_144,
            buffered_bytes: 512 * 1024 * 1024,
            active_instances: 262_144,
            operation_slots: 262_144,
            timer_slots: 262_144,
            line_sessions: 65_536,
            evidence_items: 262_144,
        },
    }
}

impl HostFabricationPackage for RaspberryPiFabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "conduit-host-raspberry-pi@1".into(),
            package_revision: 2,
            catalog: Default::default(),
            targets: vec![
                raspberry_pi_os_target(
                    "Raspberry Pi 4 Model B rev 1.5 (4 GB) · Raspberry Pi OS Bookworm 64-bit",
                    "raspberry-pi-4-model-b-rev-1.5-4gb",
                    2 * 1024 * 1024 * 1024,
                ),
                raspberry_pi_os_target(
                    "Raspberry Pi Zero 2 W rev 1.0 · Raspberry Pi OS Bookworm 64-bit",
                    "raspberry-pi-zero-2-w-rev-1.0",
                    512 * 1024 * 1024,
                ),
                raspberry_pi_os_target(
                    "Raspberry Pi Zero 2 WH rev 1.0 · Raspberry Pi OS Bookworm 64-bit",
                    "raspberry-pi-zero-2-wh-rev-1.0",
                    512 * 1024 * 1024,
                ),
                bare_metal_target(
                    "Raspberry Pi Model B+ v1.2",
                    "raspberry-pi-model-b-plus-v1.2",
                ),
                bare_metal_target("Raspberry Pi Zero v1", "raspberry-pi-zero-v1"),
                bare_metal_target("Raspberry Pi Zero W v1.1", "raspberry-pi-zero-w-v1.1"),
                bare_metal_target("Raspberry Pi Zero WH v1.1", "raspberry-pi-zero-wh-v1.1"),
            ],
            offers: vec![
                ImplementationOffer {
                    base_kind: "serial/text".into(),
                    implementation_id: "raspberry-pi/pl011@1".into(),
                    implementation_revision: 1,
                    target_patterns: vec![
                        B_PLUS_TARGET.into(),
                        ZERO_TARGET.into(),
                        ZERO_W_TARGET.into(),
                        ZERO_WH_TARGET.into(),
                    ],
                    prerequisites: Vec::new(),
                    build_feature: Some("base-pl011".into()),
                },
                ImplementationOffer {
                    base_kind: "serial/text".into(),
                    implementation_id: "raspberry-pi-os/serial@1".into(),
                    implementation_revision: 1,
                    target_patterns: vec![
                        RASPBERRY_PI_OS_TARGET.into(),
                        ZERO_2_W_TARGET.into(),
                        ZERO_2_WH_TARGET.into(),
                    ],
                    prerequisites: Vec::new(),
                    build_feature: Some("base-serial".into()),
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_family_targets_preserve_exact_board_and_architecture_truth() {
        let FabricationContribution::Anchor(anchor) = RaspberryPiFabricationPackage.contribution()
        else {
            panic!("Raspberry Pi fabrication package must be an anchor");
        };

        for target in [ZERO_TARGET, ZERO_W_TARGET, ZERO_WH_TARGET] {
            let descriptor = anchor
                .targets
                .iter()
                .find(|descriptor| descriptor.key() == target)
                .expect("each BCM2835 Zero board must have an exact descriptor");
            assert_eq!(descriptor.family, "conduitos");
            assert_eq!(descriptor.architecture, "armv6");
            assert_eq!(descriptor.default_output, SporeOutputKind::SdImage);
        }

        for target in [ZERO_2_W_TARGET, ZERO_2_WH_TARGET] {
            let descriptor = anchor
                .targets
                .iter()
                .find(|descriptor| descriptor.key() == target)
                .expect("each RP3A0 Zero 2 board must have an exact descriptor");
            assert_eq!(descriptor.family, "std");
            assert_eq!(descriptor.architecture, "aarch64");
            assert_eq!(
                descriptor.os.as_deref(),
                Some("raspberry-pi-os-bookworm-64")
            );
            assert_eq!(descriptor.default_output, SporeOutputKind::NativeBundle);
        }
    }

    #[test]
    fn zero_family_offers_do_not_cross_the_bcm2835_and_rp3a_zero_boundary() {
        let FabricationContribution::Anchor(anchor) = RaspberryPiFabricationPackage.contribution()
        else {
            panic!("Raspberry Pi fabrication package must be an anchor");
        };
        let bare_metal = anchor
            .offers
            .iter()
            .find(|offer| offer.implementation_id == "raspberry-pi/pl011@1")
            .unwrap();
        let os = anchor
            .offers
            .iter()
            .find(|offer| offer.implementation_id == "raspberry-pi-os/serial@1")
            .unwrap();

        assert!(bare_metal.target_patterns.contains(&ZERO_W_TARGET.into()));
        assert!(!bare_metal.target_patterns.contains(&ZERO_2_W_TARGET.into()));
        assert!(os.target_patterns.contains(&ZERO_2_W_TARGET.into()));
        assert!(!os.target_patterns.contains(&ZERO_W_TARGET.into()));
    }
}
