use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationMetadata, ImplementationOffer, PackageCatalogContribution, PostBuildAction,
    PrerequisiteNode, PresenterMetadata, SporeOutputKind, TargetDescriptor, TargetPresenter,
};

use std::collections::BTreeMap;

pub const PACKAGE_ID: &str = "conduit-host-orange-pi@1";
pub const ORANGE_PI_5_TARGET: &str = "conduitos/aarch64/orange-pi-5-rk3588s";
pub const ORANGE_PI_5_UART: &str = "orange-pi/dw-apb-uart2@1";

pub struct OrangePiFabricationPackage;

fn orange_pi_5_target() -> TargetDescriptor {
    TargetDescriptor {
        label: "Orange Pi 5 · RK3588S · bare-metal ConduitOS".into(),
        family: "conduitos".into(),
        architecture: "aarch64".into(),
        machine: "orange-pi-5-rk3588s".into(),
        board: Some("orange-pi-5".into()),
        os: None,
        host_core: "host-core/conduitos@1".into(),
        presenter: Some(TargetPresenter {
            id: "presenter/main".into(),
            implementation_id: "presenter/linear-serial@1".into(),
            interactive: false,
        }),
        host_operations: vec!["conduit.host/present@1".into()],
        toolchain_identity: "rustc:stable+aarch64-unknown-none+llvm-tools+u-boot-v2026.04-rk3588s"
            .into(),
        builder_adapter: "conduit-host-orange-pi/build-conduitos-sd-image@1".into(),
        deployment_adapter: Some("conduit-host-orange-pi/flash-removable-media@1".into()),
        outputs: vec![SporeOutputKind::SdImage],
        default_output: SporeOutputKind::SdImage,
        post_build_actions: vec![PostBuildAction::Flash, PostBuildAction::Boot],
        fabrication_descriptors: Vec::new(),
        maxima: HostBounds {
            static_memory_bytes: 64 * 1024 * 1024,
            heap_arena_bytes: 64 * 1024 * 1024,
            queue_items: 4096,
            buffered_bytes: 16 * 1024 * 1024,
            active_instances: 4096,
            operation_slots: 4096,
            timer_slots: 4096,
            line_sessions: 1024,
            evidence_items: 65_536,
        },
    }
}

fn catalog() -> PackageCatalogContribution {
    PackageCatalogContribution {
        implementations: BTreeMap::from([(
            "presenter/linear-serial@1".into(),
            ImplementationMetadata {
                kind: "presentation/linear".into(),
                contract_revision: "conduit.presentation/linear@1".into(),
                targets: vec![ORANGE_PI_5_TARGET.into()],
                prerequisites: vec![
                    PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                    PrerequisiteNode::Base("serial/text".into()),
                ],
            },
        )]),
        presenters: BTreeMap::from([(
            "presenter/linear-serial@1".into(),
            PresenterMetadata {
                targets: vec![ORANGE_PI_5_TARGET.into()],
                prerequisites: vec![
                    PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                    PrerequisiteNode::Base("serial/text".into()),
                ],
            },
        )]),
        dependencies: BTreeMap::from([(
            PrerequisiteNode::Base("serial/text".into()),
            vec![PrerequisiteNode::Driver(ORANGE_PI_5_UART.into())],
        )]),
        facilities: Vec::new(),
        profile_fragments: Vec::new(),
        mutually_exclusive_mechanisms: Vec::new(),
    }
}

impl HostFabricationPackage for OrangePiFabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: PACKAGE_ID.into(),
            package_revision: 1,
            catalog: catalog(),
            targets: vec![orange_pi_5_target()],
            offers: vec![ImplementationOffer {
                base_kind: "serial/text".into(),
                implementation_id: ORANGE_PI_5_UART.into(),
                implementation_revision: 1,
                target_patterns: vec![ORANGE_PI_5_TARGET.into()],
                prerequisites: Vec::new(),
                build_feature: Some("aarch64-orange-pi-5".into()),
            }],
        })
    }
}
