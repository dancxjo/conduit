use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationMetadata, ImplementationOffer, PackageCatalogContribution, PostBuildAction,
    PrerequisiteNode, SporeOutputKind, TargetDescriptor,
};
use std::collections::BTreeMap;

pub struct HostedFabricationPackage;

pub const HOSTED_TARGET_ID: &str = "std/x86_64/computer";
pub const HOSTED_WINDOWS_X86_64_TARGET_ID: &str = "std/x86_64/windows-computer";
pub const HOSTED_MACOS_AARCH64_TARGET_ID: &str = "std/aarch64/macos-computer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedPlatformSupport {
    Supported,
    Planned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostedPlatformVariant {
    pub os: &'static str,
    pub architecture: &'static str,
    pub support: HostedPlatformSupport,
    pub reason: &'static str,
}

pub const HOSTED_PLATFORM_VARIANTS: [HostedPlatformVariant; 4] = [
    HostedPlatformVariant {
        os: "linux",
        architecture: "x86_64",
        support: HostedPlatformSupport::Supported,
        reason: "reviewed native build and launch adapters are available",
    },
    HostedPlatformVariant {
        os: "linux",
        architecture: "aarch64",
        support: HostedPlatformSupport::Planned,
        reason:
            "cross-built artifacts exist, but the hosted package has no reviewed launch adapter",
    },
    HostedPlatformVariant {
        os: "windows",
        architecture: "x86_64",
        support: HostedPlatformSupport::Supported,
        reason: "reviewed native Windows build and launch adapters are available",
    },
    HostedPlatformVariant {
        os: "macos",
        architecture: "aarch64",
        support: HostedPlatformSupport::Supported,
        reason: "reviewed native macOS build and launch adapters are available",
    },
];

fn package_catalog() -> PackageCatalogContribution {
    let implementations = conduit_std_offers::supported_nucleus_offers()
        .into_iter()
        .map(|offer| {
            let implementation = offer.implementation.implementation_id.as_str().to_owned();
            let mut prerequisites = offer
                .host_operations
                .iter()
                .map(|requirement| {
                    PrerequisiteNode::HostOperation(requirement.contract_id.as_str().to_owned())
                })
                .chain(offer.resource_requirements.iter().map(|requirement| {
                    PrerequisiteNode::Resource(requirement.class_id.as_str().to_owned())
                }))
                .collect::<Vec<_>>();
            prerequisites.sort();
            prerequisites.dedup();
            (
                implementation,
                ImplementationMetadata {
                    kind: offer.kind_id.as_str().to_owned(),
                    contract_revision: offer.kind_contract_revision.as_str().to_owned(),
                    targets: vec!["std/*/*".into()],
                    prerequisites,
                },
            )
        })
        .collect();
    PackageCatalogContribution {
        implementations,
        dependencies: BTreeMap::from([
            (
                PrerequisiteNode::Resource("conduit.resource/timer-slot@1".into()),
                vec![PrerequisiteNode::Base("timer/monotonic".into())],
            ),
            (
                PrerequisiteNode::Base("timer/monotonic".into()),
                vec![PrerequisiteNode::Driver("hosted/monotonic-clock@1".into())],
            ),
        ]),
        ..Default::default()
    }
}

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

fn target(label: &str, architecture: &str, machine: &str, os: &str) -> TargetDescriptor {
    TargetDescriptor {
        label: label.into(),
        family: "std".into(),
        architecture: architecture.into(),
        machine: machine.into(),
        board: None,
        os: Some(os.into()),
        host_core: "host-core/std@1".into(),
        presenter: None,
        host_operations: Vec::new(),
        toolchain_identity: "rustc:stable".into(),
        builder_adapter: "conduit-host-hosted/build-native@1".into(),
        deployment_adapter: Some("conduit-host-hosted/launch@1".into()),
        outputs: vec![SporeOutputKind::NativeBundle],
        default_output: SporeOutputKind::NativeBundle,
        post_build_actions: vec![PostBuildAction::Launch],
        fabrication_descriptors: Vec::new(),
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
            catalog: package_catalog(),
            targets: vec![
                target(
                    "Hosted computer · Linux · x86_64",
                    "x86_64",
                    "computer",
                    "linux",
                ),
                target(
                    "Hosted computer · Windows · x86_64",
                    "x86_64",
                    "windows-computer",
                    "windows",
                ),
                target(
                    "Hosted computer · macOS · arm64",
                    "aarch64",
                    "macos-computer",
                    "macos",
                ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_canonical_target_separates_platform_from_role() {
        let FabricationContribution::Anchor(anchor) = HostedFabricationPackage.contribution()
        else {
            panic!("hosted package must remain an anchor");
        };
        assert_eq!(anchor.targets.len(), 3);
        let expected = [
            (HOSTED_TARGET_ID, "linux", "x86_64"),
            (HOSTED_WINDOWS_X86_64_TARGET_ID, "windows", "x86_64"),
            (HOSTED_MACOS_AARCH64_TARGET_ID, "macos", "aarch64"),
        ];
        for (target_id, os, architecture) in expected {
            let target = anchor
                .targets
                .iter()
                .find(|target| target.key() == target_id)
                .expect("each supported hosted platform must own one exact target");
            assert_eq!(target.os.as_deref(), Some(os));
            assert_eq!(target.architecture, architecture);
            assert!(!target.label.to_lowercase().contains("workstation"));
            assert!(!target.label.to_lowercase().contains("server"));
        }
    }

    #[test]
    fn package_owns_explicit_platform_support_truth() {
        assert_eq!(HOSTED_PLATFORM_VARIANTS.len(), 4);
        assert!(HOSTED_PLATFORM_VARIANTS.iter().any(|variant| {
            variant.os == "linux"
                && variant.architecture == "x86_64"
                && variant.support == HostedPlatformSupport::Supported
        }));
        for os in ["windows", "macos"] {
            let variant = HOSTED_PLATFORM_VARIANTS
                .iter()
                .find(|variant| variant.os == os)
                .expect("each conceptual desktop platform must be explicit");
            assert_eq!(variant.support, HostedPlatformSupport::Supported);
            assert!(!variant.reason.is_empty());
        }
    }
}
