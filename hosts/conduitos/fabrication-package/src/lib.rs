use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationMetadata, ImplementationOffer, PackageCatalogContribution, PostBuildAction,
    PrerequisiteNode, PresenterMetadata, SporeOutputKind, TargetDescriptor, TargetPresenter,
};
use std::collections::BTreeMap;

pub struct ConduitOsFabricationPackage;

fn package_catalog() -> PackageCatalogContribution {
    PackageCatalogContribution {
        implementations: BTreeMap::from([(
            "conduitos/kernel-http-client-http1-literal@1".into(),
            ImplementationMetadata {
                kind: "http/client".into(),
                contract_revision: "conduit.http/client@1".into(),
                targets: vec!["conduitos/x86_64/pc".into()],
                prerequisites: vec![
                    PrerequisiteNode::HostOperation("conduit.host/http-client-exchange@1".into()),
                    PrerequisiteNode::Resource("conduit.resource/network/http-client@1".into()),
                    PrerequisiteNode::Facility("network/http1-literal-client@1".into()),
                ],
            },
        )]),
        presenters: BTreeMap::from([(
            "presenter/linear-serial@1".into(),
            PresenterMetadata {
                targets: vec!["conduitos/aarch64/virt".into()],
                prerequisites: vec![
                    PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                    PrerequisiteNode::Base("serial/text".into()),
                ],
            },
        )]),
        dependencies: BTreeMap::from([
            (
                PrerequisiteNode::Base("serial/text".into()),
                vec![PrerequisiteNode::Driver("conduitos/pl011@1".into())],
            ),
            (
                PrerequisiteNode::Facility("network/http1-literal-client@1".into()),
                vec![
                    PrerequisiteNode::Resource("network/packet-buffer@1".into()),
                    PrerequisiteNode::Resource("network/tcp-socket@1".into()),
                    PrerequisiteNode::Resource("network/timer@1".into()),
                    PrerequisiteNode::Base("network/ipv4-tcp".into()),
                ],
            ),
            (
                PrerequisiteNode::Base("network/ipv4-tcp".into()),
                vec![PrerequisiteNode::Driver(
                    "conduitos/deterministic-ipv4-tcp@1".into(),
                )],
            ),
        ]),
        facilities: vec!["network/http1-literal-client@1".into()],
        profile_fragments: vec![
            "profile-fragment/conduitos-scripted-keyboard-proof@1".into(),
            "profile-fragment/conduitos-hotplug-proof@1".into(),
        ],
        mutually_exclusive_mechanisms: Vec::new(),
    }
}

fn target(label: &str, architecture: &str, machine: &str) -> TargetDescriptor {
    let rust_target = match architecture {
        "ia32" => "i686-unknown-none",
        "riscv64" => "riscv64gc-unknown-none-elf",
        "loongarch64" => "loongarch64-unknown-none",
        "aarch64" => "aarch64-unknown-none",
        "x86_64" => "x86_64-unknown-none",
        _ => unreachable!("package declares a finite architecture set"),
    };
    TargetDescriptor {
        label: label.into(),
        family: "conduitos".into(),
        architecture: architecture.into(),
        machine: machine.into(),
        board: None,
        os: None,
        host_core: "host-core/conduitos@1".into(),
        presenter: (architecture == "aarch64").then(|| TargetPresenter {
            id: "presenter/main".into(),
            implementation_id: "presenter/linear-serial@1".into(),
            interactive: false,
        }),
        host_operations: (architecture == "aarch64")
            .then(|| "conduit.host/present@1".into())
            .into_iter()
            .collect(),
        toolchain_identity: format!("rustc:stable+{rust_target}+llvm-tools"),
        builder_adapter: format!("conduit-host-conduitos/build-{architecture}@1"),
        deployment_adapter: Some(format!("conduit-host-conduitos/boot-{architecture}@1")),
        outputs: vec![SporeOutputKind::DiskImage, SporeOutputKind::EfiArtifact],
        default_output: SporeOutputKind::DiskImage,
        post_build_actions: vec![PostBuildAction::Boot],
        fabrication_descriptors: Vec::new(),
        maxima: HostBounds {
            static_memory_bytes: 512 * 1024 * 1024,
            heap_arena_bytes: 512 * 1024 * 1024,
            queue_items: 65_536,
            buffered_bytes: 512 * 1024 * 1024,
            active_instances: 4096,
            operation_slots: 4096,
            timer_slots: 4096,
            line_sessions: 1024,
            evidence_items: 65_536,
        },
    }
}

impl HostFabricationPackage for ConduitOsFabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "conduitos-image@1".into(),
            package_revision: 1,
            catalog: package_catalog(),
            targets: vec![
                target("ConduitOS x86_64 PC", "x86_64", "pc"),
                target("ConduitOS IA-32 PC", "ia32", "pc"),
                target("ConduitOS AArch64 virt", "aarch64", "virt"),
                target("ConduitOS RISC-V64 virt", "riscv64", "virt"),
                target("ConduitOS LoongArch64 virt", "loongarch64", "virt"),
            ],
            offers: vec![
                ImplementationOffer {
                    base_kind: "serial/text".into(),
                    implementation_id: "conduitos/pl011@1".into(),
                    implementation_revision: 1,
                    target_patterns: vec!["conduitos/aarch64/virt".into()],
                    prerequisites: Vec::new(),
                    build_feature: Some("base-pl011".into()),
                },
                ImplementationOffer {
                    base_kind: "network/ipv4-tcp".into(),
                    implementation_id: "conduitos/deterministic-ipv4-tcp@1".into(),
                    implementation_revision: 1,
                    target_patterns: vec!["conduitos/x86_64/pc".into()],
                    prerequisites: Vec::new(),
                    build_feature: Some("base-ipv4-tcp".into()),
                },
            ],
        })
    }
}
