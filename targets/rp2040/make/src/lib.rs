use conduit_host_make::{
    HostBounds, HostMakePackage, ImplementationOffer, MakeAnchor, MakeContribution,
    PostBuildAction, SporeOutputKind, TargetDescriptor,
};

pub const PACKAGE_ID: &str = "conduit-host-rp2040@1";
pub const TARGET_ID: &str = "conduitos/thumbv6m/pico-w";
pub const BUILDER_ADAPTER: &str = "conduit-host-rp2040/build-uf2@1";

pub struct Rp2040MakePackage;

impl HostMakePackage for Rp2040MakePackage {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Anchor(MakeAnchor {
            package_id: PACKAGE_ID.into(),
            package_revision: 1,
            catalog: Default::default(),
            targets: vec![TargetDescriptor {
                label: "Pico W".into(),
                family: "conduitos".into(),
                architecture: "thumbv6m".into(),
                machine: "pico-w".into(),
                board: Some("pico-w".into()),
                os: None,
                host_core: "host-core/conduitos@1".into(),
                presenter: None,
                host_calls: Vec::new(),
                toolchain_identity: "rustc:1.98.1+thumbv6m-none-eabi".into(),
                builder_adapter: BUILDER_ADAPTER.into(),
                strategy: conduit_host_make::MakeStrategy::DeterministicSpecializedBuild,
                deployment_adapter: Some("conduit-host-rp2040/flash-uf2@1".into()),
                outputs: vec![SporeOutputKind::Uf2],
                default_output: SporeOutputKind::Uf2,
                post_build_actions: vec![PostBuildAction::Flash, PostBuildAction::Boot],
                make_descriptors: Vec::new(),
                maxima: HostBounds {
                    static_memory_bytes: 256 * 1024,
                    heap_arena_bytes: 256 * 1024,
                    queue_items: 4096,
                    buffered_bytes: 4 * 1024 * 1024,
                    active_instances: 4096,
                    operation_slots: 4096,
                    timer_slots: 4096,
                    line_sessions: 4096,
                    evidence_items: 4096,
                },
            }],
            offers: vec![ImplementationOffer {
                base_kind: "serial/text".into(),
                implementation_id: "pico/usb-cdc@1".into(),
                implementation_revision: 1,
                target_patterns: vec![TARGET_ID.into()],
                prerequisites: Vec::new(),
                build_feature: Some("line-usb-cdc".into()),
            }],
        })
    }
}
