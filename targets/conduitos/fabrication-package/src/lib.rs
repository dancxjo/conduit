use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationMetadata, ImplementationOffer, PackageCatalogContribution, PostBuildAction,
    PrerequisiteNode, PresenterMetadata, SporeOutputKind, TargetDescriptor, TargetPresenter,
};
use std::collections::BTreeMap;

pub struct ConduitOsFabricationPackage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConduitOsProductArtifact {
    pub target: &'static str,
    pub binary: &'static str,
    pub rust_target: &'static str,
}

impl ConduitOsProductArtifact {
    pub fn for_target(target: &str) -> Option<Self> {
        match target {
            "conduitos/x86_64/pc" => Some(Self {
                target: "conduitos/x86_64/pc",
                binary: "conduitos",
                rust_target: "x86_64-unknown-none",
            }),
            "conduitos/aarch64/virt" => Some(Self {
                target: "conduitos/aarch64/virt",
                binary: "conduitos-aarch64-product",
                rust_target: "aarch64-unknown-none",
            }),
            "conduitos/ia32/pc" => Some(Self {
                target: "conduitos/ia32/pc",
                binary: "conduitos-ia32-product",
                rust_target: "i686-unknown-linux-gnu",
            }),
            "conduitos/riscv64/virt" => Some(Self {
                target: "conduitos/riscv64/virt",
                binary: "conduitos-riscv64-product",
                rust_target: "riscv64gc-unknown-none-elf",
            }),
            "conduitos/loongarch64/virt" => Some(Self {
                target: "conduitos/loongarch64/virt",
                binary: "conduitos-loongarch64-product",
                rust_target: "loongarch64-unknown-none",
            }),
            _ => None,
        }
    }
}

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
        presenters: BTreeMap::from([
            (
                "presenter/linear-serial@1".into(),
                PresenterMetadata {
                    targets: vec!["conduitos/aarch64/virt".into()],
                    prerequisites: vec![
                        PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                        PrerequisiteNode::Base("serial/text".into()),
                    ],
                },
            ),
            (
                "presenter/ia32-linear-debugcon@1".into(),
                PresenterMetadata {
                    targets: vec!["conduitos/ia32/pc".into()],
                    prerequisites: vec![
                        PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                        PrerequisiteNode::Base("conduitos/ia32-debugcon-text".into()),
                    ],
                },
            ),
            (
                "presenter/riscv64-linear-sbi-console@1".into(),
                PresenterMetadata {
                    targets: vec!["conduitos/riscv64/virt".into()],
                    prerequisites: vec![
                        PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                        PrerequisiteNode::Base("conduitos/riscv64-sbi-console-text".into()),
                    ],
                },
            ),
            (
                "presenter/loongarch64-linear-uart@1".into(),
                PresenterMetadata {
                    targets: vec!["conduitos/loongarch64/virt".into()],
                    prerequisites: vec![
                        PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                        PrerequisiteNode::Base("conduitos/loongarch64-uart-text".into()),
                    ],
                },
            ),
        ]),
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
        "ia32" => "i686-unknown-linux-gnu-object+rust-lld-elf_i386",
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
        presenter: matches!(architecture, "ia32" | "aarch64" | "riscv64" | "loongarch64").then(
            || TargetPresenter {
                id: "presenter/main".into(),
                implementation_id: match architecture {
                    "ia32" => "presenter/ia32-linear-debugcon@1".into(),
                    "riscv64" => "presenter/riscv64-linear-sbi-console@1".into(),
                    "loongarch64" => "presenter/loongarch64-linear-uart@1".into(),
                    _ => "presenter/linear-serial@1".into(),
                },
                interactive: false,
            },
        ),
        host_operations: matches!(architecture, "ia32" | "aarch64" | "riscv64" | "loongarch64")
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
                    base_kind: "conduitos/ia32-debugcon-text".into(),
                    implementation_id: "conduitos/ia32-debugcon@1".into(),
                    implementation_revision: 1,
                    target_patterns: vec!["conduitos/ia32/pc".into()],
                    prerequisites: Vec::new(),
                    build_feature: Some("base-ia32-debugcon".into()),
                },
                ImplementationOffer {
                    base_kind: "conduitos/riscv64-sbi-console-text".into(),
                    implementation_id: "conduitos/riscv64-sbi-console@1".into(),
                    implementation_revision: 1,
                    target_patterns: vec!["conduitos/riscv64/virt".into()],
                    prerequisites: Vec::new(),
                    build_feature: None,
                },
                ImplementationOffer {
                    base_kind: "conduitos/loongarch64-uart-text".into(),
                    implementation_id: "conduitos/loongarch64-uart@1".into(),
                    implementation_revision: 1,
                    target_patterns: vec!["conduitos/loongarch64/virt".into()],
                    prerequisites: Vec::new(),
                    build_feature: None,
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

#[cfg(test)]
mod tests {
    use super::ConduitOsProductArtifact;

    #[test]
    fn product_registry_has_no_architecture_proof_aliases() {
        let product = ConduitOsProductArtifact::for_target("conduitos/aarch64/virt").unwrap();
        assert_eq!(product.binary, "conduitos-aarch64-product");
        let ia32 = ConduitOsProductArtifact::for_target("conduitos/ia32/pc").unwrap();
        assert_eq!(ia32.binary, "conduitos-ia32-product");
        assert_eq!(ia32.rust_target, "i686-unknown-linux-gnu");
        let riscv64 = ConduitOsProductArtifact::for_target("conduitos/riscv64/virt").unwrap();
        assert_eq!(riscv64.binary, "conduitos-riscv64-product");
        assert_eq!(riscv64.rust_target, "riscv64gc-unknown-none-elf");
        let loongarch64 =
            ConduitOsProductArtifact::for_target("conduitos/loongarch64/virt").unwrap();
        assert_eq!(loongarch64.binary, "conduitos-loongarch64-product");
        assert_eq!(loongarch64.rust_target, "loongarch64-unknown-none");
        for proof_identity in [
            "conduitos/aarch64/a0-proof",
            "conduitos/aarch64/a3-proof",
            "conduitos-aarch64-a3",
            "conduitos-riscv64-a4",
            "conduitos-loongarch64-a4",
        ] {
            assert!(ConduitOsProductArtifact::for_target(proof_identity).is_none());
        }
    }
}
