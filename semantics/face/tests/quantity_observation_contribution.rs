use conduit_core::Quantity;
use conduit_data::{
    QuantityObservationGap, QuantityObservationOutcome as Outcome,
    ResolvedQuantityObservation as Observation, ResolvedQuantityObservationBatch as Batch,
};
use conduit_face::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/inspector".into(),
        plan_id: "plan/inspector".into(),
        active_play_id: "play/inspector".into(),
        required_interaction_context: None,
    }
}
fn batch() -> Batch {
    Batch {
        source_identity: "meter-source".into(),
        source_name: "Reported meter history".into(),
        channel_identity: "meter-voltage".into(),
        channel_name: "Supply voltage".into(),
        producer_host: "reported-host/meter".into(),
        producer_boot: "reported-boot/meter".into(),
        producer_plan: "reported-plan/meter".into(),
        producer_play: "reported-play/meter".into(),
        samples_outside_view: 2,
        observations: BoundedSequence::try_from_iter(
            [
                (1, 1000, Some("10V")),
                (2, 2000, Some("11V")),
                (3, 3000, None),
                (5, 11000, Some("12V")),
            ]
            .into_iter()
            .map(|(sequence, time, value)| Observation {
                sequence,
                sampled_at_boot_ms: time,
                outcome: value.map_or_else(
                    || Outcome::Unavailable("meter-unavailable".into()),
                    |value| {
                        Outcome::Available(Box::new(Quantity::parse_plot_literal(value).unwrap()))
                    },
                ),
                gap_before: (sequence == 5).then_some(QuantityObservationGap {
                    first_sequence: 4,
                    last_sequence: 4,
                }),
            }),
        )
        .unwrap(),
    }
}
fn face(fragment: FaceFragment) -> Face {
    Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/inspector".into()),
            checked_plot_id: Some(fragment.basis.checked_plot_id),
            expanded_plot_id: Some("expanded/inspector".into()),
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
fn canonical_batch_becomes_shared_series_with_distinct_reported_and_inspecting_provenance() {
    let prepared = PreparedQuantityObservationFace::prepare().unwrap();
    let encoded = batch().encode().unwrap();
    let fragment = prepared
        .canonical_observation(basis(), &encoded, "Voltage history")
        .unwrap();
    assert!(
        inspection_fragment_backing_bytes(&fragment).unwrap()
            <= quantity_observation_fragment_backing_bound()
    );
    assert!(prepared.retained_backing_bytes().unwrap() > 0);
    assert!(
        prepared.projection_workspace_bytes().unwrap()
            > 2 * quantity_observation_fragment_backing_bound()
    );
    let face = face(fragment);
    let series = PreparedQuantitySeries::prepare(&face, "meter-voltage", 16).unwrap();
    assert_eq!(
        series.producer(),
        [
            "reported-host/meter",
            "reported-boot/meter",
            "reported-plan/meter",
            "reported-play/meter"
        ]
    );
    assert_eq!(
        face.basis.active_play_id.as_ref().unwrap().as_str(),
        "play/inspector"
    );
    let plot = series
        .project(Some(conduit_core::Unit::resolve("mV").unwrap()), 16)
        .unwrap();
    assert_eq!(
        plot.points
            .iter()
            .map(|point| point.time_millionths)
            .collect::<Vec<_>>(),
        [0, 100000, 1000000]
    );
    assert_eq!(plot.unavailable_samples, 1);
    assert_eq!(plot.gap_runs, 1);
    assert_eq!(plot.minimum.as_deref(), Some("10000 mV"));
    assert!(plot.publication_evidence.is_none() && plot.publication_reference.is_none());
    assert!(face.actions.is_empty());
}
#[test]
fn invalid_input_basis_label_and_subject_collision_refuse_before_publication() {
    let prepared = PreparedQuantityObservationFace::prepare().unwrap();
    let encoded = batch().encode().unwrap();
    assert!(matches!(
        prepared.canonical_observation(basis(), b"not canonical", "Voltage"),
        Err(QuantityObservationFaceRefusal::Input(_))
    ));
    let mut invalid = basis();
    invalid.required_interaction_context = Some("context/foreign".into());
    assert!(matches!(
        prepared.canonical_observation(invalid, &encoded, "Voltage"),
        Err(QuantityObservationFaceRefusal::Basis)
    ));
    assert!(matches!(
        prepared.canonical_observation(basis(), &encoded, ""),
        Err(QuantityObservationFaceRefusal::Label)
    ));
    let mut collision = batch();
    collision.channel_identity = "meter-source/sample/1".into();
    assert!(matches!(
        prepared.canonical_observation(basis(), &collision.encode().unwrap(), "Voltage"),
        Err(QuantityObservationFaceRefusal::Fragment)
    ));
}
#[test]
fn maximum_finite_batch_and_empty_batch_stay_within_prepared_backing() {
    let prepared = PreparedQuantityObservationFace::prepare().unwrap();
    for count in [0, 16] {
        let mut input = batch();
        input.source_identity = "s".repeat(128);
        input.channel_identity = "c".repeat(128);
        input.source_name = "S".repeat(128);
        input.channel_name = "C".repeat(128);
        input.producer_host = "h".repeat(256);
        input.producer_boot = "b".repeat(256);
        input.producer_plan = "p".repeat(256);
        input.producer_play = "a".repeat(256);
        input.observations =
            BoundedSequence::try_from_iter((1..=count).map(|sequence| Observation {
                sequence,
                sampled_at_boot_ms: sequence,
                outcome: Outcome::Available(Box::new(Quantity::parse_plot_literal("1V").unwrap())),
                gap_before: None,
            }))
            .unwrap();
        let fragment = prepared
            .canonical_observation(basis(), &input.encode().unwrap(), &"L".repeat(128))
            .unwrap();
        assert!(
            inspection_fragment_backing_bytes(&fragment).unwrap()
                <= quantity_observation_fragment_backing_bound()
        );
        let face = face(fragment);
        assert_eq!(
            PreparedQuantitySeries::prepare(&face, &"c".repeat(128), 16)
                .unwrap()
                .samples()
                .len(),
            count as usize
        );
    }
}

#[test]
fn inspection_backing_refuses_unaccounted_semantic_composition_ownership() {
    let prepared = PreparedQuantityObservationFace::prepare().unwrap();
    let mut fragment = prepared
        .canonical_observation(basis(), &batch().encode().unwrap(), "Voltage history")
        .unwrap();
    fragment.composition.push(FaceCompositionRelation {
        identity: "composition/reported".into(),
        source: "meter-source".into(),
        target: "meter-voltage".into(),
        kind: serde_json::from_str(r#"{"Semantic":"meaning/reported"}"#).unwrap(),
    });
    assert!(inspection_fragment_backing_bytes(&fragment).is_none());
}

#[test]
fn captured_snapshot_preserves_two_independent_producer_axes_and_refuses_aliases() {
    use conduit_data::ResolvedQuantityObservationSnapshot as Snapshot;
    let mut first = batch();
    first.observations =
        BoundedSequence::try_from_iter(first.observations.as_slice()[..1].iter().cloned()).unwrap();
    let mut second = first.clone();
    second.source_identity = "another-source".into();
    second.channel_identity = "another-voltage".into();
    second.channel_name = "Battery voltage".into();
    second.producer_host = "reported-host/another".into();
    second.producer_boot = "reported-boot/another".into();
    let mut observation = second.observations.as_slice()[0].clone();
    observation.sampled_at_boot_ms = 9;
    second.observations = BoundedSequence::try_from_iter([observation]).unwrap();
    let snapshot = Snapshot {
        batches: BoundedSequence::try_from_iter([first.clone(), second.clone()]).unwrap(),
    };
    let prepared = PreparedQuantityObservationSnapshotFace::prepare().unwrap();
    let fragment = prepared
        .canonical_observation(basis(), &snapshot.encode().unwrap(), "Voltage history")
        .unwrap();
    assert!(
        inspection_fragment_backing_bytes(&fragment).unwrap()
            <= quantity_observation_snapshot_fragment_backing_bound()
    );
    assert_eq!(
        fragment
            .subjects
            .iter()
            .find(|s| s.identity == "another-voltage")
            .unwrap()
            .name,
        "Battery voltage"
    );
    assert!(fragment
        .properties
        .iter()
        .any(|p| p.name == "inspection-label"
            && p.value == FacePropertyValue::Text("Voltage history".into())));
    let view = face(fragment);
    let one = PreparedQuantitySeries::prepare(&view, "meter-voltage", 16).unwrap();
    let two = PreparedQuantitySeries::prepare(&view, "another-voltage", 16).unwrap();
    assert_ne!(one.producer()[0], two.producer()[0]);
    assert_ne!(one.producer()[1], two.producer()[1]);
    assert_eq!(two.samples()[0].boot_ms, 9);
    second.channel_identity = format!("{}/sample/1", first.source_identity);
    let aliases = Snapshot {
        batches: BoundedSequence::try_from_iter([first, second]).unwrap(),
    };
    assert!(prepared
        .canonical_observation(basis(), &aliases.encode().unwrap(), "Voltage history")
        .is_err());
}

#[test]
fn snapshot_projection_refuses_single_batch_profile_and_trailing_bytes() {
    let prepared = PreparedQuantityObservationSnapshotFace::prepare().unwrap();
    let batch = batch().encode().unwrap();
    assert!(matches!(
        prepared.canonical_observation(basis(), &batch, "Voltage"),
        Err(QuantityObservationFaceRefusal::Input(_))
    ));
    let snapshot = conduit_data::ResolvedQuantityObservationSnapshot {
        batches: BoundedSequence::try_from_iter([self::batch()]).unwrap(),
    };
    let mut encoded = snapshot.encode().unwrap();
    encoded.push(0);
    assert!(matches!(
        prepared.canonical_observation(basis(), &encoded, "Voltage"),
        Err(QuantityObservationFaceRefusal::Input(_))
    ));
}
