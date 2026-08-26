use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationOffer, PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub struct HostedFabricationPackage;

fn maxima() -> HostBounds {
    HostBounds {
        static_memory_bytes: 2 * 1024 * 1024 * 1024,
        heap_arena_bytes: 2 * 1024 * 1024 * 1024,
        queue_items: 1_048_576,
        buffered_bytes: 2 * 1024 * 1024 * 1024,
        active_instances: 1_048_576,
        operation_slots: 1_048_576,
        timer_slots: 1_048_576,
        line_sessions: 1_048_576,
        evidence_items: 1_048_576,
    }
}

fn target(label: &str, machine: &str) -> TargetDescriptor {
    TargetDescriptor {
        label: label.into(),
        family: "std".into(),
        architecture: "x86_64".into(),
        machine: machine.into(),
        board: None,
        os: Some("linux".into()),
        host_core: "host-core/std@1".into(),
        presenter: None,
        host_operations: Vec::new(),
        toolchain_identity: "rustc:stable".into(),
        builder_adapter: "conduit-host-hosted/build-native@1".into(),
        deployment_adapter: Some("conduit-host-hosted/launch@1".into()),
        outputs: vec![SporeOutputKind::NativeBundle],
        default_output: SporeOutputKind::NativeBundle,
        post_build_actions: vec![PostBuildAction::Launch],
        maxima: maxima(),
    }
}

fn offer(kind: &str, implementation: &str, feature: &str) -> ImplementationOffer {
    ImplementationOffer {
        base_kind: kind.into(),
        implementation_id: implementation.into(),
        implementation_revision: 1,
        target_patterns: vec!["std/*/*".into()],
        prerequisites: Vec::new(),
        build_feature: Some(feature.into()),
    }
}

impl HostFabricationPackage for HostedFabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "hosted-native@1".into(),
            package_revision: 1,
            targets: vec![
                target("Hosted Linux workstation", "workstation"),
                target("Hosted Linux server", "server"),
            ],
            offers: vec![
                offer("clock/monotonic", "hosted/monotonic-clock@1", "base-clock"),
                offer("serial/text", "hosted/serial@1", "base-serial"),
                offer(
                    "storage/protected-file",
                    "hosted/protected-file@1",
                    "base-protected-file",
                ),
                offer("timer/monotonic", "hosted/monotonic-clock@1", "base-timer"),
            ],
        })
    }
}
