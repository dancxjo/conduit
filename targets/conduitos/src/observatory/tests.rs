use super::*;
use crate::{
    dual_region_plan,
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
};

fn fixture() -> (BootRecord, BootIdentities, HostOffer<'static>) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        512 * 1024,
    );
    let record = BootRecord {
        firmware: crate::boot::Firmware::X86Bios,
        timestamp: 1,
        hhdm_offset: 2,
        image_physical_start: 3,
        image_length: 4,
        memory_region_count: 5,
        artifact_count: 0,
        framebuffer_count: 0,
        command_line_bytes: 0,
        runtime_arena: crate::boot::RuntimeArena {
            physical_start: 6,
            length: 512 * 1024,
        },
    };
    (record, identities, offer)
}

#[test]
fn export_is_an_exact_bounded_v2_snapshot() {
    let (record, identities, offer) = fixture();
    let prepared = dual_region_plan::prepare(&identities, &offer, "build").unwrap();
    let export = prepare_export(
        &record,
        &identities,
        &offer,
        &prepared,
        "build",
        "image",
        None,
    )
    .unwrap();
    let snapshot: ObservatorySnapshot = serde_json::from_slice(export.as_bytes()).unwrap();

    validate_snapshot(&snapshot).unwrap();
    assert_eq!(snapshot.bases.len(), 7);
    assert_eq!(
        snapshot.hosts[0].advertisement.planner_capabilities.len(),
        0
    );
    assert_eq!(snapshot.plays[0].lifecycle, PlanLifecycle::Completed);
    assert_eq!(snapshot.observations.len(), 10);
    assert_eq!(snapshot.historical_observations.len(), 9);
    assert_eq!(snapshot.sealed_boot_provenance.len(), 1);
    assert!(
        snapshot.bases.iter().all(|base| {
            base.kind_id.as_str() != "Limine" && base.kind_id.as_str() != "x86-bios"
        })
    );
    let report = conduit_observatory::build_report(&snapshot).unwrap();
    let linear = conduit_observatory::render_text_report(&report);
    assert_eq!(report.execution_regions.len(), 2);
    assert!(linear.contains("execution_regions 2"));
    assert!(linear.contains("region=region/text"));
    assert!(linear.contains("region=region/timer"));
    assert!(linear.contains("ExecutionRegionOverlap"));
    assert!(linear.contains("physical_parallelism: false"));
    assert!(linear.contains("scheduling=CooperativeBoundedStep lane_count=1"));
    assert!(linear.contains("preemption_required=false isolation_required=false"));
    let mut rendered_copy = report.clone();
    rendered_copy.execution_regions[0].lane_count = 99;
    assert_eq!(
        snapshot.plans[0].fragments[0].execution_regions[0].lane_count,
        1
    );
    assert!(linear.contains("boot provenance [sealed] 1"));
    assert!(linear.contains("history=current"));
    assert!(linear.contains("history=historical"));
    assert!(export.as_bytes().len() <= MAX_EXPORT_BYTES);
}

#[test]
fn image_bound_provenance_refuses_stale_build_and_image_truth() {
    let (record, identities, offer) = fixture();
    let prepared = dual_region_plan::prepare(&identities, &offer, "build").unwrap();
    assert_eq!(
        prepare_image_bound_export(
            &record,
            &identities,
            &offer,
            &prepared,
            ImageBoundProvenance {
                profile_id: "profile:sha256:bound",
                build_id: "stale-build",
                image_binding: "image:sha256:bound",
            },
            None,
        )
        .err(),
        Some(ExportError::InvalidSnapshot)
    );
    let export = prepare_image_bound_export(
        &record,
        &identities,
        &offer,
        &prepared,
        ImageBoundProvenance {
            profile_id: "profile:sha256:bound",
            build_id: "build",
            image_binding: "image:sha256:bound",
        },
        None,
    )
    .unwrap();
    let snapshot: ObservatorySnapshot = serde_json::from_slice(export.as_bytes()).unwrap();
    assert_eq!(
        snapshot.sealed_boot_provenance[0]
            .image_build_trace
            .as_ref()
            .unwrap()
            .profile_id
            .as_str(),
        "profile:sha256:bound"
    );
}

#[test]
fn stale_provenance_duplicate_bases_and_signs_fail_closed_while_gaps_remain_visible() {
    let (record, identities, offer) = fixture();
    let prepared = dual_region_plan::prepare(&identities, &offer, "build").unwrap();
    let export = prepare_export(
        &record,
        &identities,
        &offer,
        &prepared,
        "build",
        "image",
        None,
    )
    .unwrap();
    let snapshot: ObservatorySnapshot = serde_json::from_slice(export.as_bytes()).unwrap();

    let mut duplicate_base = snapshot.clone();
    duplicate_base.bases.push(duplicate_base.bases[0].clone());
    assert!(validate_snapshot(&duplicate_base).is_err());

    let mut stale_provenance = snapshot.clone();
    stale_provenance.sealed_boot_provenance[0].boot_id = conduit_core::BootId::from("stale-boot");
    assert!(validate_snapshot(&stale_provenance).is_err());

    let mut duplicate_sign = snapshot.clone();
    duplicate_sign.historical_observations[0].sign_id =
        duplicate_sign.observations[0].sign_id.clone();
    assert!(validate_snapshot(&duplicate_sign).is_err());

    let mut gaps = snapshot;
    gaps.historical_observations[0].kind = ObservationKind::SignGap { dropped: 3 };
    gaps.retention.dropped_items = 2;
    let report = conduit_observatory::build_report(&gaps).unwrap();
    assert_eq!(report.retention.visible_gap_count, 5);
}

#[test]
fn unrepresentable_boot_inputs_fail_before_play() {
    let (mut record, identities, offer) = fixture();
    let prepared = dual_region_plan::prepare(&identities, &offer, "build").unwrap();
    record.artifact_count = 1;
    assert_eq!(
        prepare_export(
            &record,
            &identities,
            &offer,
            &prepared,
            "build",
            "image",
            None
        )
        .err(),
        Some(ExportError::UnsupportedBootArtifacts)
    );
    record.artifact_count = 0;
    record.framebuffer_count = 1;
    assert_eq!(
        prepare_export(
            &record,
            &identities,
            &offer,
            &prepared,
            "build",
            "image",
            None
        )
        .err(),
        Some(ExportError::UnsupportedFramebuffer)
    );
}
