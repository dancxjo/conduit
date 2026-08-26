use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationOffer, PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub struct BrowserFabricationPackage;

impl HostFabricationPackage for BrowserFabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "browser-wasm@1".into(),
            package_revision: 1,
            targets: vec![TargetDescriptor {
                label: "Browser page".into(),
                family: "browser".into(),
                architecture: "wasm32".into(),
                machine: "page".into(),
                board: None,
                os: None,
                host_core: "host-core/std@1".into(),
                presenter: None,
                host_operations: Vec::new(),
                toolchain_identity: "rustc:stable+wasm32-unknown-unknown".into(),
                builder_adapter: "conduit-host-browser/build-wasm@1".into(),
                deployment_adapter: Some("conduit-host-browser/load@1".into()),
                outputs: vec![SporeOutputKind::BrowserBundle],
                default_output: SporeOutputKind::BrowserBundle,
                post_build_actions: vec![PostBuildAction::Load, PostBuildAction::Launch],
                maxima: HostBounds {
                    static_memory_bytes: 64 * 1024 * 1024,
                    heap_arena_bytes: 64 * 1024 * 1024,
                    queue_items: 65_536,
                    buffered_bytes: 64 * 1024 * 1024,
                    active_instances: 4096,
                    operation_slots: 4096,
                    timer_slots: 4096,
                    line_sessions: 1024,
                    evidence_items: 65_536,
                },
            }],
            offers: vec![ImplementationOffer {
                base_kind: "browser/dom".into(),
                implementation_id: "browser/dom@1".into(),
                implementation_revision: 1,
                target_patterns: vec!["browser/wasm32/page".into()],
                prerequisites: Vec::new(),
                build_feature: Some("base-browser-dom".into()),
            }],
        })
    }
}
