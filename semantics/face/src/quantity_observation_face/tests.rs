//! Allocating typed-model preparation only, never a Step or installed admission proof.
use super::*;
use alloc::{boxed::Box, vec, vec::Vec};
fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/inspection".into(),
        plan_id: "plan/inspection".into(),
        active_play_id: "play/inspection".into(),
        required_interaction_context: None,
    }
}
fn channel(source: &'static str, series: &'static str) -> ReportedQuantityChannel<'static> {
    ReportedQuantityChannel {
        source_identity: source,
        source_name: source,
        channel_identity: series,
        channel_name: series,
        producer: [
            "reported/host",
            "reported/boot",
            "reported/plan",
            "reported/play",
        ],
        samples_outside_view: 64,
        recovery: Some(ReportedQuantityRecovery {
            chunks: 2,
            samples: 128,
            preceding_history_known: false,
        }),
    }
}
fn samples(quantity: &conduit_core::Quantity) -> Vec<ReportedQuantityObservation<'_>> {
    (0..64_u64)
        .map(|index| ReportedQuantityObservation {
            sequence: 2 * index + 1,
            sampled_at_boot_ms: index * index * 7,
            outcome: if index % 2 == 0 {
                ReportedQuantityOutcome::Available(quantity)
            } else {
                ReportedQuantityOutcome::Unavailable("checksum")
            },
            gap_before: (index > 0).then_some((2 * index, 2 * index)),
        })
        .collect()
}
fn face(fragment: FaceFragment) -> Face {
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/inspection".into()),
            checked_plot_id: Some(fragment.basis.checked_plot_id),
            expanded_plot_id: Some("expanded/inspection".into()),
            plan_id: Some(fragment.basis.plan_id),
            active_play_id: Some(fragment.basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap()
}
#[test]
fn two_complete_sixty_four_sample_channels_preserve_recovery_and_reported_provenance() {
    let voltage = conduit_core::Quantity::parse_plot_literal("12V").unwrap();
    let rows = samples(&voltage);
    let fragment = project_reported_quantity_channels(
        basis(),
        &[
            (channel("source/a", "series/a"), &rows),
            (channel("source/b", "series/b"), &rows),
        ],
        "Reported history",
    )
    .unwrap();
    let backing = inspection_fragment_backing_bytes(&fragment).unwrap();
    assert!(
        backing
            <= 2 * reported_quantity_fragment_backing_bound(64, true).unwrap()
                + 2 * (core::mem::size_of::<FaceProperty>() + 128 + "inspection-label".len() + 128)
    );
    assert!(reported_quantity_projection_workspace_bytes(64, 2, true).unwrap() > backing);
    let face = face(fragment);
    for (source, channel) in [("source/a", "series/a"), ("source/b", "series/b")] {
        let series = PreparedQuantitySeries::prepare(&face, channel, 64).unwrap();
        assert_eq!(
            series.producer(),
            [
                "reported/host",
                "reported/boot",
                "reported/plan",
                "reported/play"
            ]
        );
        assert_eq!(series.samples().len(), 64);
        for (index, sample) in series.samples().iter().enumerate() {
            assert_eq!(sample.sequence, 2 * index as u64 + 1);
            assert_eq!(sample.boot_ms, (index as u64).pow(2) * 7);
            assert_eq!(sample.value.is_some(), index % 2 == 0);
            assert_eq!(
                sample.unavailable_reason,
                (index % 2 != 0).then_some("checksum")
            );
        }
        let plot = series.project(None, 16).unwrap();
        assert_eq!(plot.source_samples, 64);
        assert_eq!(plot.points.len(), 16);
        assert_eq!(plot.unavailable_samples, 32);
        assert_eq!(plot.omitted_samples, "64"); // previously recovered accepted samples, not point decimation
        assert!(plot.publication_reference.is_none() && plot.publication_evidence.is_none());
        for (name, value) in [
            ("recovered-chunks", FacePropertyValue::Count(2)),
            ("recovered-samples", FacePropertyValue::Count(128)),
            ("preceding-history-known", FacePropertyValue::Flag(false)),
        ] {
            assert!(face
                .properties
                .iter()
                .any(|p| p.subject == source && p.name == name && p.value == value));
        }
    }
    assert_eq!(
        face.basis.active_play_id.unwrap().as_str(),
        "play/inspection"
    );
}
#[test]
fn malformed_or_unbounded_typed_models_refuse_without_truncating() {
    let voltage = conduit_core::Quantity::parse_plot_literal("12V").unwrap();
    let rows = samples(&voltage);
    let metadata = channel("source/a", "series/a");
    let project = |samples: &[ReportedQuantityObservation<'_>]| {
        project_reported_quantity_channel(basis(), metadata, samples, "History")
    };
    let mut over = rows.clone();
    over.push(rows[0]);
    assert!(project(&over).is_err());
    let mut wrong = rows.clone();
    wrong[1].sequence = 1;
    assert!(project(&wrong).is_err());
    let mut wrong = rows.clone();
    wrong[1].gap_before = None;
    assert!(project(&wrong).is_err());
    let mut wrong = rows.clone();
    wrong[1].sampled_at_boot_ms = 99999;
    assert!(project(&wrong).is_err());
    let mut wrong = rows.clone();
    wrong[1].outcome = ReportedQuantityOutcome::Unavailable("");
    assert!(project(&wrong).is_err());
    let mut wrong = rows.clone();
    let seconds = conduit_core::Quantity::parse_plot_literal("1s").unwrap();
    wrong[2].outcome = ReportedQuantityOutcome::Available(&seconds);
    assert!(project(&wrong).is_err());
    let mut recovery = metadata;
    recovery.samples_outside_view = 63;
    assert!(project_reported_quantity_channel(basis(), recovery, &rows, "History").is_err());
    let mut foreign = metadata;
    foreign.channel_identity = "source/a/sample/1";
    assert!(project_reported_quantity_channel(basis(), foreign, &rows, "History").is_err());
    assert!(project_reported_quantity_channels(
        basis(),
        &[(metadata, &rows), (metadata, &rows)],
        "History"
    )
    .is_err());
    assert!(reported_quantity_fragment_backing_bound(65, true).is_none());
    assert!(reported_quantity_projection_workspace_bytes(64, 3, true).is_none());
}
#[test]
fn data_sixteen_projection_retains_its_exact_fact_order_and_separate_bound() {
    let input = conduit_data::ResolvedQuantityObservationBatch {
        source_identity: "source/a".into(),
        source_name: "source/a".into(),
        channel_identity: "series/a".into(),
        channel_name: "series/a".into(),
        producer_host: "reported/host".into(),
        producer_boot: "reported/boot".into(),
        producer_plan: "reported/plan".into(),
        producer_play: "reported/play".into(),
        samples_outside_view: 0,
        observations: conduit_plot::rust_binding::BoundedSequence::try_from_iter([
            conduit_data::ResolvedQuantityObservation {
                sequence: 1,
                sampled_at_boot_ms: 7,
                outcome: QuantityObservationOutcome::Available(Box::new(
                    conduit_core::Quantity::parse_plot_literal("12V").unwrap(),
                )),
                gap_before: None,
            },
        ])
        .unwrap(),
    };
    let fragment = project_batch(basis(), &input, "History").unwrap();
    let mut metadata = channel("source/a", "series/a");
    metadata.recovery = None;
    metadata.samples_outside_view = 0;
    let voltage = conduit_core::Quantity::parse_plot_literal("12V").unwrap();
    let typed = [ReportedQuantityObservation {
        sequence: 1,
        sampled_at_boot_ms: 7,
        outcome: ReportedQuantityOutcome::Available(&voltage),
        gap_before: None,
    }];
    assert_eq!(
        fragment,
        project_reported_quantity_channel(basis(), metadata, &typed, "History").unwrap()
    );
    assert_eq!(
        quantity_observation_fragment_backing_bound(),
        reported_quantity_fragment_backing_bound(16, false).unwrap()
    );
    assert!(
        reported_quantity_fragment_backing_bound(64, true).unwrap()
            > quantity_observation_fragment_backing_bound()
    );
    assert_eq!(
        fragment
            .properties
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        [
            "clock",
            "producer-host",
            "producer-boot",
            "producer-plan",
            "producer-play",
            "visible-samples",
            "samples-outside-view",
            "observation-origin",
            "series-source",
            "series-value-property",
            "reported-channel-name",
            "series-source",
            "sequence",
            "sampled-at-boot-ms",
            "series-available",
            "value"
        ]
    );
}
