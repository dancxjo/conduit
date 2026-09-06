use conduit_host_fabrication::{
    FabricationAnchor, FabricationContribution, HostBounds, HostFabricationPackage,
    ImplementationOffer, PackageCatalogContribution, PostBuildAction, PrerequisiteNode,
    PresenterMetadata, SporeOutputKind, TargetDescriptor,
};
use std::collections::BTreeMap;

mod device;
mod inventory;
mod line;
mod media;

pub use device::{BrowserDeviceRealizationDescriptor, BROWSER_DEVICE_REALIZATIONS};
pub use inventory::{
    default_configuration_bases, validate_browser_inventory, BrowserImplementationDescriptor,
    BrowserInventoryDiagnostic, BrowserRealizationDescriptor, BrowserRuntimePrerequisite,
    BrowserStorageRealizationDescriptor, BROWSER_DURABLE_STORAGE_REALIZATION,
    BROWSER_HUMAN_PRESENTATION_REALIZATIONS, BROWSER_IMPLEMENTATIONS, REVIEWED_DISTRIBUTION_ID,
    REVIEWED_RUNTIME_ARTIFACT,
};
pub use line::{BrowserLineRealizationDescriptor, BROWSER_LINE_REALIZATIONS};
pub use media::{BrowserMediaRealizationDescriptor, BROWSER_MEDIA_REALIZATIONS};

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
    fn media_entries_bind_the_accepted_two_plan_realization_exactly() {
        assert_eq!(BROWSER_MEDIA_REALIZATIONS.len(), 2);
        for realization in BROWSER_MEDIA_REALIZATIONS {
            let fabricated = BROWSER_IMPLEMENTATIONS
                .iter()
                .find(|item| item.implementation_id == realization.fabrication_implementation_id)
                .unwrap();
            assert_eq!(
                fabricated.implementation_revision,
                realization.implementation_revision
            );
            assert!(realization
                .acquisition_offer_id
                .starts_with("media/acquire-"));
            assert_eq!(realization.maximum_acquisitions_in_flight, 1);
            assert_eq!(realization.maximum_result_bytes, 1024);
            assert_eq!(realization.maximum_value_bytes, 64 * 1024);
            assert_eq!(realization.maximum_queue_items, 1);
            assert_eq!(realization.maximum_queue_bytes, 64 * 1024);
            assert!(!realization.stable_physical_device_identity);
            assert!(realization.requires_subsequent_use_plan);
        }
        assert!(
            !BROWSER_IMPLEMENTATIONS
                .iter()
                .any(|item| { item.implementation_id == "browser/web-audio-output@1" }),
            "Web Audio API presence is not an accepted audio-output realization"
        );
    }

    #[test]
    fn device_entries_bind_exact_chooser_and_transfer_realizations() {
        assert_eq!(BROWSER_DEVICE_REALIZATIONS.len(), 2);
        for realization in BROWSER_DEVICE_REALIZATIONS {
            let fabricated = BROWSER_IMPLEMENTATIONS
                .iter()
                .find(|item| item.implementation_id == realization.fabrication_implementation_id)
                .unwrap();
            assert_eq!(
                fabricated.implementation_revision,
                realization.implementation_revision
            );
            assert!(realization
                .acquisition_offer_id
                .starts_with("device/acquire-"));
            assert!(realization
                .runtime_base_implementation_id
                .starts_with("browser/web-"));
            assert_eq!(realization.maximum_active_devices, 1);
            assert_eq!(realization.maximum_transfers_in_flight, 1);
            assert_eq!(realization.maximum_transfer_bytes, 4096);
            assert_eq!(realization.maximum_reads_or_in_transfers, 8);
            assert_eq!(realization.maximum_writes_or_out_transfers, 8);
            assert!(!realization.stable_hardware_identity);
            assert!(realization.requires_subsequent_use_plan);
        }
    }

    #[test]
    fn human_and_presentation_entries_bind_exact_portable_runtime_realizations() {
        let fabricated = BROWSER_IMPLEMENTATIONS
            .iter()
            .map(|item| item.implementation_id)
            .collect::<BTreeSet<_>>();
        for realization in BROWSER_HUMAN_PRESENTATION_REALIZATIONS {
            assert!(fabricated.contains(realization.fabrication_implementation_id));
            assert!(!realization.portable_kind.contains("browser"));
            assert!(realization
                .runtime_implementation_id
                .starts_with("browser/"));
            assert!(realization
                .runtime_artifact_id
                .starts_with("conduit-browser-runtime/"));
            assert!(realization.host_operation.contains("browser"));
            assert_eq!(realization.maximum_in_flight, 1);
            assert!((1..=8).contains(&realization.maximum_queue_items));
            assert!(realization.maximum_queue_bytes > 0);
        }
        assert!(BROWSER_HUMAN_PRESENTATION_REALIZATIONS.iter().any(|item| {
            item.fabrication_implementation_id == "browser/keyboard-events@1"
                && item.portable_kind == "input/keyboard"
        }));
        assert!(BROWSER_HUMAN_PRESENTATION_REALIZATIONS.iter().any(|item| {
            item.fabrication_implementation_id == "browser/pointer-events@1"
                && item.portable_kind == "input/pointer-source"
        }));
        assert!(!BROWSER_HUMAN_PRESENTATION_REALIZATIONS.iter().any(|item| {
            item.portable_kind.contains("touch") || item.portable_kind.contains("gamepad")
        }));
    }

    #[test]
    fn checked_rich_configuration_uses_the_same_package_catalog() {
        let packages = FabricationPackageSet::compose(&[&BrowserFabricationPackage]).unwrap();
        let catalog = FabricationCatalog::canonical().with_packages(&packages);
        let source = include_str!("../../profiles/browser-rich.host.conduit");
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

    #[test]
    fn durable_storage_metadata_matches_the_shared_host_adapter_contract() {
        let storage = &BROWSER_DURABLE_STORAGE_REALIZATION;
        assert_eq!(storage.fabrication_implementation_id, "browser/indexeddb@1");
        assert_eq!(storage.portable_kind, "storage/durable");
        assert_eq!(storage.implementation_revision, 1);
        assert_eq!(storage.artifact_id, "browser-application-storage.mjs@1");
        assert_eq!(storage.maximum_key_bytes, 256);
        assert_eq!(storage.maximum_records_per_host, 1_024);
        assert_eq!(storage.maximum_bytes_per_host, 16 * 1024 * 1024);
        assert_ne!(storage.application_store, storage.host_identity_store);
    }

    #[test]
    fn line_realizations_bind_exact_contracts_limits_and_outbound_authority() {
        assert_eq!(BROWSER_LINE_REALIZATIONS.len(), 2);
        let fabricated = BROWSER_IMPLEMENTATIONS
            .iter()
            .map(|item| item.implementation_id)
            .collect::<BTreeSet<_>>();
        for line in BROWSER_LINE_REALIZATIONS {
            assert!(fabricated.contains(line.fabrication_implementation_id));
            assert_eq!(line.implementation_revision, 1);
            assert!(line.base_implementation_id.starts_with("conduit.base/"));
            assert!(line.artifact_id.ends_with(".mjs@1"));
            assert_eq!(line.maximum_sessions_per_host, 4);
            assert_eq!(line.maximum_in_flight_items, 1);
            assert_eq!(line.maximum_payload_bytes, 64 * 1024);
            assert!(line.maximum_frame_bytes >= line.maximum_payload_bytes);
            assert!(line.maximum_frame_bytes <= line.maximum_buffered_bytes);
            assert_eq!(line.maximum_buffered_bytes, 256 * 1024);
            assert!(line.maximum_received_messages > 0);
            assert!(!line.endpoint_authority.is_empty());
            assert!(!line.credential_requirement.is_empty());
            assert!(line.initiates_outbound_only);
        }
        let websocket = &BROWSER_LINE_REALIZATIONS[0];
        assert_eq!(
            websocket.contract.scope,
            conduit_core::LineScope::RoutedNetwork
        );
        assert_eq!(
            websocket.contract.security,
            conduit_core::LineSecurity::PlaintextNetwork
        );
        assert_eq!(websocket.maximum_frame_bytes, 64 * 1024);
        assert_eq!(websocket.signaling_bootstrap, None);
        let webrtc = &BROWSER_LINE_REALIZATIONS[1];
        assert_eq!(webrtc.contract.scope, conduit_core::LineScope::PointToPoint);
        assert_eq!(
            webrtc.contract.security,
            conduit_core::LineSecurity::AuthenticatedEncrypted
        );
        assert_eq!(webrtc.maximum_frame_bytes, 128 * 1024);
        assert!(webrtc.signaling_bootstrap.is_some());
    }
}
