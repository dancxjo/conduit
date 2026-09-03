use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationOffer, PackageCatalogContribution, PostBuildAction, PrerequisiteNode,
    PresenterMetadata, SporeOutputKind, TargetDescriptor,
};
use std::collections::BTreeMap;

mod inventory;

pub use inventory::{
    default_configuration_bases, validate_browser_inventory, BrowserImplementationDescriptor,
    BrowserInventoryDiagnostic, BrowserRuntimePrerequisite, BROWSER_IMPLEMENTATIONS,
    REVIEWED_DISTRIBUTION_ID, REVIEWED_RUNTIME_ARTIFACT,
};

pub struct BrowserFabricationPackage;

fn package_catalog() -> PackageCatalogContribution {
    let implementations = BROWSER_IMPLEMENTATIONS
        .iter()
        .map(|descriptor| {
            (
                descriptor.implementation_id.into(),
                conduit_host_fabrication::ImplementationMetadata {
                    kind: descriptor.base_kind.into(),
                    contract_revision: "1".into(),
                    targets: vec!["browser/wasm32/page".into()],
                    prerequisites: Vec::new(),
                },
            )
        })
        .collect();
    PackageCatalogContribution {
        implementations,
        presenters: BTreeMap::from([(
            "presenter/browser-dom-svg@1".into(),
            PresenterMetadata {
                targets: vec!["browser/wasm32/page".into()],
                prerequisites: vec![
                    PrerequisiteNode::HostOperation("conduit.host/present@1".into()),
                    PrerequisiteNode::Resource("presentation/surface".into()),
                    PrerequisiteNode::Base("browser/dom".into()),
                ],
            },
        )]),
        ..Default::default()
    }
}

impl HostFabricationPackage for BrowserFabricationPackage {
    fn contribution(&self) -> FabricationContribution {
        FabricationContribution::Anchor(FabricationAnchor {
            package_id: "browser-wasm@1".into(),
            package_revision: 1,
            catalog: package_catalog(),
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
                toolchain_identity: "conduit.browser/reviewed-distribution@1".into(),
                builder_adapter: "conduit-host-browser/bind-prebuilt@1".into(),
                deployment_adapter: Some("conduit-host-browser/load@1".into()),
                outputs: vec![SporeOutputKind::BrowserBundle],
                default_output: SporeOutputKind::BrowserBundle,
                post_build_actions: vec![PostBuildAction::Load, PostBuildAction::Launch],
                fabrication_descriptors: Vec::new(),
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
            offers: BROWSER_IMPLEMENTATIONS
                .iter()
                .map(|descriptor| ImplementationOffer {
                    base_kind: descriptor.base_kind.into(),
                    implementation_id: descriptor.implementation_id.into(),
                    implementation_revision: descriptor.implementation_revision,
                    target_patterns: vec!["browser/wasm32/page".into()],
                    prerequisites: descriptor
                        .prerequisites
                        .iter()
                        .map(|item| item.kind.into())
                        .collect(),
                    build_feature: Some(format!("profile:{}", descriptor.implementation_id)),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_fabrication::{FabricationCatalog, FabricationPackageSet};
    use std::collections::BTreeSet;

    #[test]
    fn reviewed_inventory_is_exact_finite_and_catalog_authoritative() {
        validate_browser_inventory(BROWSER_IMPLEMENTATIONS).unwrap();
        let packages = FabricationPackageSet::compose(&[&BrowserFabricationPackage]).unwrap();
        let catalog = FabricationCatalog::canonical().with_packages(&packages);
        assert!(BROWSER_IMPLEMENTATIONS.len() >= 10);
        let identities = BROWSER_IMPLEMENTATIONS
            .iter()
            .map(|item| item.implementation_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(identities.len(), BROWSER_IMPLEMENTATIONS.len());
        for descriptor in BROWSER_IMPLEMENTATIONS {
            assert_eq!(descriptor.artifact, REVIEWED_RUNTIME_ARTIFACT);
            assert!(descriptor.maximum_instances > 0);
            assert!(descriptor.maximum_buffered_bytes > 0);
            assert!(catalog
                .base_kinds
                .iter()
                .any(|kind| kind == descriptor.base_kind));
            assert!(catalog
                .implementations
                .contains_key(descriptor.implementation_id));
        }
    }

    #[test]
    fn malformed_inventory_fails_closed_before_catalog_use() {
        let mut duplicate = BROWSER_IMPLEMENTATIONS.to_vec();
        duplicate.push(BROWSER_IMPLEMENTATIONS[0].clone());
        assert!(matches!(
            validate_browser_inventory(&duplicate)
                .unwrap_err()
                .as_slice(),
            [BrowserInventoryDiagnostic::DuplicateImplementation(_)]
        ));

        let mut missing_artifact = BROWSER_IMPLEMENTATIONS[0].clone();
        missing_artifact.artifact = "";
        assert!(matches!(
            validate_browser_inventory(&[missing_artifact])
                .unwrap_err()
                .as_slice(),
            [BrowserInventoryDiagnostic::MissingArtifactBinding(_)]
        ));

        let malformed_prerequisites = BrowserImplementationDescriptor {
            group: "Devices",
            label: "Broken device",
            base_kind: "device/usb",
            implementation_id: "browser/broken-usb@1",
            implementation_revision: 1,
            artifact: REVIEWED_RUNTIME_ARTIFACT,
            maximum_instances: 1,
            maximum_buffered_bytes: 1,
            prerequisites: &[BrowserRuntimePrerequisite {
                kind: "device-acquisition",
                detail: "missing its authority prerequisites",
            }],
        };
        assert!(matches!(
            validate_browser_inventory(&[malformed_prerequisites])
                .unwrap_err()
                .as_slice(),
            [BrowserInventoryDiagnostic::ContradictoryPrerequisites(_)]
        ));
    }

    #[test]
    fn authority_sensitive_mechanisms_keep_runtime_prerequisites() {
        for id in [
            "browser/media-devices-camera@1",
            "browser/media-devices-microphone@1",
            "browser/webserial@1",
            "browser/webusb@1",
        ] {
            let descriptor = BROWSER_IMPLEMENTATIONS
                .iter()
                .find(|item| item.implementation_id == id)
                .unwrap();
            let prerequisites = descriptor
                .prerequisites
                .iter()
                .map(|item| item.kind)
                .collect::<BTreeSet<_>>();
            assert!(prerequisites.contains("user-activation"));
            assert!(prerequisites.contains("permission"));
            assert!(prerequisites.contains("device-acquisition"));
        }
    }

    #[test]
    fn checked_rich_configuration_uses_the_same_package_catalog() {
        let packages = FabricationPackageSet::compose(&[&BrowserFabricationPackage]).unwrap();
        let catalog = FabricationCatalog::canonical().with_packages(&packages);
        let source =
            include_str!("../../../../../profiles/host-configurations/browser-rich.host.conduit");
        let configuration =
            conduit_host_fabrication::parse_host_configuration_conduit(source).unwrap();
        let checked =
            conduit_host_fabrication::check_host_configuration(configuration, &catalog, &packages)
                .unwrap();
        assert_eq!(checked.resolved_bases().len(), 7);
        assert!(checked
            .resolved_bases()
            .iter()
            .any(|(kind, implementation)| {
                kind == "storage/durable" && implementation == "browser/indexeddb@1"
            }));
    }
}
