use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationOffer, PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub const B_PLUS_TARGET: &str = "conduitos/armv6/raspberry-pi-model-b-plus-v1.2";
pub const ZERO_TARGET: &str = "conduitos/armv6/raspberry-pi-zero-v1";

pub struct RaspberryPiFabricationPackage;

fn target(label: &str, machine: &str) -> TargetDescriptor {
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

impl HostFabricationPackage for RaspberryPiFabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "conduit-host-raspberry-pi@1".into(),
            package_revision: 1,
            catalog: Default::default(),
            targets: vec![
                target(
                    "Raspberry Pi Model B+ v1.2",
                    "raspberry-pi-model-b-plus-v1.2",
                ),
                target("Raspberry Pi Zero v1", "raspberry-pi-zero-v1"),
            ],
            offers: vec![ImplementationOffer {
                base_kind: "serial/text".into(),
                implementation_id: "raspberry-pi/pl011@1".into(),
                implementation_revision: 1,
                target_patterns: vec![B_PLUS_TARGET.into(), ZERO_TARGET.into()],
                prerequisites: Vec::new(),
                build_feature: Some("base-pl011".into()),
            }],
        })
    }
}
