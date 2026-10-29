use conduit_host_make::{
    HostBounds, HostMakePackage, ImplementationOffer, MakeAnchor, MakeContribution,
    PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub const B_PLUS_TARGET: &str = "conduitos/armv6/raspberry-pi-model-b-plus-v1.2";
pub const B_PLUS_LINUX_TARGET: &str = "std/armv6/raspberry-pi-model-b-plus-v1.2";
pub const PI3_PLUS_LINUX_TARGET: &str = "std/armv6/raspberry-pi-3-model-b-plus-rev-1.3";
pub const ZERO_TARGET: &str = "conduitos/armv6/raspberry-pi-zero-v1";
pub const ZERO_W_TARGET: &str = "conduitos/armv6/raspberry-pi-zero-w-v1.1";
pub const ZERO_WH_TARGET: &str = "conduitos/armv6/raspberry-pi-zero-wh-v1.1";
pub const ZERO_2_W_TARGET: &str = "std/aarch64/raspberry-pi-zero-2-w-rev-1.0";
pub const ZERO_2_WH_TARGET: &str = "std/aarch64/raspberry-pi-zero-2-wh-rev-1.0";
pub const RASPBERRY_PI_OS_TARGET: &str = "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb";

pub struct RaspberryPiMakePackage;

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
        host_calls: Vec::new(),
        toolchain_identity: "rustc:stable+armv6-none-eabi+rust-lld".into(),
        builder_adapter: "conduit-host-raspberry-pi/build-sd-image@1".into(),
        strategy: conduit_host_make::MakeStrategy::DeterministicSpecializedBuild,
        deployment_adapter: Some("conduit-host-raspberry-pi/flash-removable-media@1".into()),
        outputs: vec![SporeOutputKind::SdImage],
        default_output: SporeOutputKind::SdImage,
        post_build_actions: vec![PostBuildAction::Flash, PostBuildAction::Boot],
        make_descriptors: Vec::new(),
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
        host_calls: Vec::new(),
        toolchain_identity:
            "rustc:1.98.1+aarch64-unknown-linux-gnu+gcc-aarch64-linux-gnu+g++-aarch64-linux-gnu+libc6-dev-arm64-cross"
                .into(),
        builder_adapter: "conduit-host-raspberry-pi/build-raspios-native@1".into(),
        strategy: conduit_host_make::MakeStrategy::DeterministicSpecializedBuild,
        deployment_adapter: Some("conduit-host-raspberry-pi/install-raspios-package@1".into()),
        outputs: vec![SporeOutputKind::NativeBundle],
        default_output: SporeOutputKind::NativeBundle,
        post_build_actions: vec![PostBuildAction::Launch],
        make_descriptors: Vec::new(),
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

fn b_plus_linux_target() -> TargetDescriptor {
    let mut target = raspberry_pi_os_target(
        "Raspberry Pi Model B+ v1.2 · Raspberry Pi OS 32-bit",
        "raspberry-pi-model-b-plus-v1.2",
        256 * 1024 * 1024,
    );
    target.architecture = "armv6".into();
    target.os = Some("raspberry-pi-os-32".into());
    target.toolchain_identity = "rustc:1.98.1+cross:0.2.5+arm-unknown-linux-gnueabihf+arm1176jzf-s+sysroot:sha256:0151546031f7255e58249d57d3307f5f08150373c5597951e1fc61dd753f2ac8".into();
    target.maxima = HostBounds {
        static_memory_bytes: 16 * 1024 * 1024,
        heap_arena_bytes: 64 * 1024 * 1024,
        queue_items: 4096,
        buffered_bytes: 16 * 1024 * 1024,
        active_instances: 512,
        operation_slots: 256,
        timer_slots: 128,
        line_sessions: 64,
        evidence_items: 4096,
    };
    target
}

fn pi3_plus_linux_target() -> TargetDescriptor {
    // The ARM1176-compatible executable runs on this ARMv8 board in 32-bit OS mode.
    // Its exact board identity stays distinct from the original B+.
    let mut target = b_plus_linux_target();
    target.label = "Raspberry Pi 3 Model B+ rev 1.3 · Raspberry Pi OS 32-bit".into();
    target.machine = "raspberry-pi-3-model-b-plus-rev-1.3".into();
    target.board = Some(target.machine.clone());
    target
}

impl HostMakePackage for RaspberryPiMakePackage {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Anchor(MakeAnchor {
            package_id: "conduit-host-raspberry-pi@1".into(),
            package_revision: 4,
            catalog: Default::default(),
            targets: vec![
                b_plus_linux_target(),
                pi3_plus_linux_target(),
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
                        B_PLUS_LINUX_TARGET.into(),
                        PI3_PLUS_LINUX_TARGET.into(),
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
    fn pi3_plus_linux_keeps_exact_board_and_compatible_32bit_abi() {
        let target = pi3_plus_linux_target();
        assert_eq!(target.key(), PI3_PLUS_LINUX_TARGET);
        let profile = conduit_host_make::parse_host_configuration_conduit(include_str!(
            "../../../std/profiles/raspberry-pi-3-model-b-plus-rev-1.3-linux.host.conduit"
        ))
        .unwrap();
        assert_eq!(profile.target.machine, target.machine);
        assert_eq!(profile.target.architecture, target.architecture);
        assert_eq!(profile.target.board, target.board);
        assert_eq!(profile.target.os, target.os);
        assert_eq!(profile.limits, target.maxima);
    }

    #[test]
    fn b_plus_linux_is_a_distinct_exact_host_and_preserves_native_target() {
        let MakeContribution::Anchor(anchor) = RaspberryPiMakePackage.contribution() else {
            panic!("anchor")
        };
        let linux = anchor
            .targets
            .iter()
            .find(|target| target.key() == B_PLUS_LINUX_TARGET)
            .unwrap();
        assert_eq!(linux.family, "std");
        assert_eq!(linux.architecture, "armv6");
        assert_eq!(linux.os.as_deref(), Some("raspberry-pi-os-32"));
        assert_eq!(linux.default_output, SporeOutputKind::NativeBundle);
        assert_eq!(linux.host_core, "host-core/std@1");
        assert_eq!(linux.maxima.heap_arena_bytes, 64 * 1024 * 1024);
        assert_eq!(linux.maxima.buffered_bytes, 16 * 1024 * 1024);
        let profile = conduit_host_make::parse_host_configuration_conduit(include_str!(
            "../../../std/profiles/raspberry-pi-model-b-plus-v1.2-linux.host.conduit"
        ))
        .unwrap();
        assert_eq!(profile.target.architecture, linux.architecture);
        assert_eq!(profile.target.machine, linux.machine);
        assert_eq!(profile.target.board, linux.board);
        assert_eq!(profile.target.os, linux.os);
        assert_eq!(profile.limits, linux.maxima);
        let native = anchor
            .targets
            .iter()
            .find(|target| target.key() == B_PLUS_TARGET)
            .unwrap();
        assert_eq!(native.family, "conduitos");
        assert_eq!(native.os, None);
        assert_eq!(native.default_output, SporeOutputKind::SdImage);
        assert!(anchor.offers.iter().any(|offer| offer.implementation_id
            == "raspberry-pi-os/serial@1"
            && offer.target_patterns.contains(&B_PLUS_LINUX_TARGET.into())));
    }

    #[test]
    fn zero_family_targets_preserve_exact_board_and_architecture_truth() {
        let MakeContribution::Anchor(anchor) = RaspberryPiMakePackage.contribution() else {
            panic!("Raspberry Pi make package must be an anchor");
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
        let MakeContribution::Anchor(anchor) = RaspberryPiMakePackage.contribution() else {
            panic!("Raspberry Pi make package must be an anchor");
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
