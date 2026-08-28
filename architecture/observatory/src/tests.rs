use alloc::vec;

use super::{
    build_report, render_text_report, unsupported_state, BootProofClass, BuildInclusionPathReport,
    CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport,
    ImageBuildTraceReport, LineReport, MemoryMapSummary, ObservatorySnapshot, OfferFreshness,
    OperationalState, PlanLifecycle, RetentionReport, SealedBootProvenanceReport, SNAPSHOT_SCHEMA,
};
use conduit_core::{ArtifactId, TerminalDisposition};
use conduit_signal_conformance::exact_std_pico_usb_plan;

#[test]
fn status_vocabulary_keeps_failure_modes_distinct() {
    assert_ne!(OperationalState::Stale, OperationalState::Unreachable);
    assert_ne!(OperationalState::Failed, OperationalState::Denied);
    assert_ne!(OperationalState::Unknown, unsupported_state());
    assert_ne!(CapabilitySupport::Unsupported, CapabilitySupport::Unknown);
    assert_ne!(
        CapabilityAvailability::Unavailable,
        CapabilityAvailability::Unknown
    );
    assert_ne!(PlanLifecycle::Failed, PlanLifecycle::Cancelled);
    assert_ne!(
        TerminalDisposition::Completed,
        TerminalDisposition::Failed {
            reason: conduit_core::FailureReason::UnsupportedKind,
        }
    );
}

#[test]
fn projects_exact_std_pico_usb_arrangement_without_promoting_physical_proof() {
    let exact = exact_std_pico_usb_plan().expect("current std/Pico USB plan resolves");
    let hosts = [
        exact.source_advertisement.clone(),
        exact.sink_advertisement.clone(),
    ]
    .into_iter()
    .map(|advertisement| HostReport {
        capabilities: advertisement
            .capabilities
            .iter()
            .map(|capability| CapabilityStatusReport {
                capability_id: capability.capability_id.clone(),
                freshness: OfferFreshness::Fresh,
                support: CapabilitySupport::Supported,
                availability: CapabilityAvailability::Available,
            })
            .collect(),
        advertisement,
        state: OperationalState::Available,
    })
    .collect();
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts,
        bases: vec![],
        lines: vec![LineReport {
            offer: exact.line_offer.clone(),
            state: OperationalState::Available,
        }],
        plans: vec![exact.plan],
        plays: vec![],
        observations: vec![],
        historical_observations: vec![],
        sealed_boot_provenance: vec![],
        retention: RetentionReport {
            item_capacity: 256,
            retained_items: 0,
            dropped_items: 0,
        },
    };

    let report = build_report(&snapshot).expect("exact S4 arrangement projects");
    assert_eq!(report.hosts.len(), 2);
    assert_eq!(report.fragments.len(), 2);
    assert_eq!(report.lines.len(), 1);
    assert_eq!(report.lines[0].offer, exact.line_offer);
    let rendered = render_text_report(&report);
    assert!(rendered.contains("base=conduit.base/usb-cdc-acm@1"));
    assert!(rendered.contains("s4/std-pico-usb-cdc-link"));
    assert!(rendered.contains("profile=rust-std-kernel"));
    assert!(rendered.contains("profile=rp2040-kernel"));
    assert!(rendered.contains("plays 0"));
}

#[test]
fn traces_current_profile_build_image_host_boot_and_inclusion_without_owning_truth() {
    let exact = exact_std_pico_usb_plan().expect("current std/Pico USB plan resolves");
    let advertisement = exact.sink_advertisement;
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: vec![HostReport {
            advertisement: advertisement.clone(),
            state: OperationalState::Available,
            capabilities: advertisement
                .capabilities
                .iter()
                .map(|capability| CapabilityStatusReport {
                    capability_id: capability.capability_id.clone(),
                    freshness: OfferFreshness::Fresh,
                    support: CapabilitySupport::Supported,
                    availability: CapabilityAvailability::Available,
                })
                .collect(),
        }],
        bases: vec![],
        lines: vec![],
        plans: vec![],
        plays: vec![],
        observations: vec![],
        historical_observations: vec![],
        sealed_boot_provenance: vec![SealedBootProvenanceReport {
            host_id: advertisement.host_id,
            boot_id: advertisement.boot_id,
            firmware_environment: "rp2040-current".into(),
            adapter_name: "rp2040-fabrication-package".into(),
            adapter_version: "1".into(),
            adapter_revision: "1".into(),
            image_id: ArtifactId::from("image:sha256:exact"),
            build_id: ArtifactId::from("build:sha256:exact"),
            image_build_trace: Some(ImageBuildTraceReport {
                profile_id: "sha256:profile".into(),
                inclusions: vec![BuildInclusionPathReport {
                    request: "capability:signal/show".into(),
                    path: vec!["package:rp2040".into(), "artifact:signal-show".into()],
                }],
            }),
            memory_map: MemoryMapSummary {
                normalized_region_count: 1,
                runtime_arena_bytes: 4096,
            },
            boot_artifacts: vec![ArtifactId::from("artifact:signal-show")],
            initial_plan_artifact_id: None,
            recovery_plan_artifact_id: None,
            framebuffers: vec![],
            proof_class: BootProofClass::FirmwareExecution,
        }],
        retention: RetentionReport {
            item_capacity: 16,
            retained_items: 0,
            dropped_items: 0,
        },
    };

    let report = build_report(&snapshot).expect("current provenance projects");
    let rendered = render_text_report(&report);
    assert!(rendered.contains("rp2040-current"));
    assert!(rendered.contains("build:sha256:exact"));
    assert!(rendered.contains("profile=sha256:profile"));
    assert!(rendered.contains("inclusion_paths=1"));
}
