use conduit_core::Quantity;
use conduit_data::{
    checked_quantity_observation_batch_type, quantity_observation_input_preparation_backing_bound,
    resolved_quantity_observation_batch_backing_bound,
    resolved_quantity_observation_batch_backing_bytes,
    resolved_quantity_observation_batch_byte_bound, validate_quantity_observation_batch,
    PreparedQuantityObservationInput, QuantityObservationGap, QuantityObservationInputRefusal as R,
    QuantityObservationOutcome as Outcome, ResolvedQuantityObservation as Observation,
    ResolvedQuantityObservationBatch as Batch,
};
use conduit_data::{
    checked_quantity_observation_snapshot_type,
    quantity_observation_snapshot_input_preparation_backing_bound,
    resolved_quantity_observation_snapshot_backing_bound,
    resolved_quantity_observation_snapshot_backing_bytes,
    resolved_quantity_observation_snapshot_byte_bound, validate_quantity_observation_snapshot,
    PreparedQuantityObservationSnapshotInput, ResolvedQuantityObservationSnapshot as Snapshot,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn batch(observations: Vec<Observation>) -> Batch {
    Batch {
        source_identity: "meter-history".into(),
        source_name: "Reported meter history".into(),
        channel_identity: "meter-voltage".into(),
        channel_name: "Supply voltage".into(),
        producer_host: "reported-host/meter".into(),
        producer_boot: "reported-boot/meter".into(),
        producer_plan: "reported-plan/meter".into(),
        producer_play: "reported-play/meter".into(),
        samples_outside_view: 2,
        observations: BoundedSequence::try_from_iter(observations).unwrap(),
    }
}
fn sample(sequence: u64, time: u64, quantity: Option<&str>) -> Observation {
    Observation {
        sequence,
        sampled_at_boot_ms: time,
        gap_before: None,
        outcome: quantity.map_or_else(
            || Outcome::Unavailable("provider-unavailable".into()),
            |text| Outcome::Available(Box::new(Quantity::parse_plot_literal(text).unwrap())),
        ),
    }
}
fn fixture() -> Batch {
    let mut last = sample(4, 11000, Some("12000mV"));
    last.gap_before = Some(QuantityObservationGap {
        first_sequence: 3,
        last_sequence: 3,
    });
    batch(vec![
        sample(1, 1000, Some("10V")),
        sample(2, 2000, None),
        last,
    ])
}

#[test]
fn canonical_native_input_preserves_irregular_time_gaps_units_and_reported_provenance() {
    let original = fixture();
    let encoded = original.clone().encode().unwrap();
    let prepared = PreparedQuantityObservationInput::prepare().unwrap();
    assert!(encoded.len() <= prepared.maximum_bytes() as usize);
    let decoded = prepared.decode(&encoded).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(decoded.observations.as_slice()[2].sampled_at_boot_ms, 11000);
    assert!(
        matches!(&decoded.observations.as_slice()[1].outcome, Outcome::Unavailable(reason)
        if reason == "provider-unavailable")
    );
    assert!(
        matches!(&decoded.observations.as_slice()[2].outcome, Outcome::Available(value)
        if **value == Quantity::parse_plot_literal("12000mV").unwrap())
    );
    assert!(checked_quantity_observation_batch_type()
        .unwrap()
        .invariants
        .is_empty());
    assert_eq!(
        resolved_quantity_observation_batch_byte_bound().unwrap(),
        prepared.maximum_bytes()
    );
    assert!(prepared.retained_backing_bytes().unwrap() > 0);
    assert!(
        quantity_observation_input_preparation_backing_bound().unwrap()
            > prepared.projection_workspace_bytes().unwrap()
    );
    assert!(
        resolved_quantity_observation_batch_backing_bytes(&decoded).unwrap()
            <= resolved_quantity_observation_batch_backing_bound()
    );
}

#[test]
fn exact_sequence_time_and_preceding_gap_laws_refuse_inconsistent_inputs() {
    for (observations, expected) in [
        (vec![sample(0, 1, Some("1V"))], R::Order),
        (vec![sample(1, 2, Some("1V")), sample(1, 3, None)], R::Order),
        (vec![sample(1, 2, Some("1V")), sample(2, 1, None)], R::Order),
        (vec![sample(1, 2, Some("1V")), sample(3, 3, None)], R::Gap),
    ] {
        assert_eq!(
            validate_quantity_observation_batch(&batch(observations)),
            Err(expected)
        );
    }
    let mut wrong = sample(3, 3, Some("1V"));
    wrong.gap_before = Some(QuantityObservationGap {
        first_sequence: 1,
        last_sequence: 2,
    });
    assert_eq!(
        validate_quantity_observation_batch(&batch(vec![sample(1, 1, None), wrong])),
        Err(R::Gap)
    );
    let mut wrong = sample(2, 2, Some("1V"));
    wrong.gap_before = Some(QuantityObservationGap {
        first_sequence: 1,
        last_sequence: 1,
    });
    assert_eq!(
        validate_quantity_observation_batch(&batch(vec![sample(1, 1, None), wrong])),
        Err(R::Gap)
    );
    let mut overflow = sample(u64::MAX, 3, None);
    overflow.gap_before = Some(QuantityObservationGap {
        first_sequence: 1,
        last_sequence: u64::MAX,
    });
    assert_eq!(
        validate_quantity_observation_batch(&batch(vec![overflow])),
        Err(R::Gap)
    );
    assert!(validate_quantity_observation_batch(&batch(vec![
        sample(1, 1, None),
        sample(2, 1, None)
    ]))
    .is_ok());
}

#[test]
fn incompatible_dimensions_roles_empty_reasons_and_identity_collisions_refuse() {
    assert_eq!(
        validate_quantity_observation_batch(&batch(vec![
            sample(1, 1, Some("1V")),
            sample(2, 2, Some("1m"))
        ])),
        Err(R::Quantity)
    );
    let mut delta = sample(2, 2, None);
    delta.outcome = Outcome::Available(Box::new(
        conduit_core::Quantity::parse_plot_literal("TemperatureDelta(1, °C)").unwrap(),
    ));
    assert_eq!(
        validate_quantity_observation_batch(&batch(vec![sample(1, 1, Some("1°C")), delta])),
        Err(R::Quantity)
    );
    let mut unavailable = sample(1, 1, None);
    unavailable.outcome = Outcome::Unavailable(String::new());
    assert_eq!(
        validate_quantity_observation_batch(&batch(vec![unavailable])),
        Err(R::Encoding)
    );
    let mut same = fixture();
    same.channel_identity = same.source_identity.clone();
    assert_eq!(validate_quantity_observation_batch(&same), Err(R::Identity));
}

#[test]
fn maximum_and_empty_windows_are_finite_and_malformed_encoding_has_no_value() {
    let prepared = PreparedQuantityObservationInput::prepare().unwrap();
    for length in [0, 16] {
        let mut input = batch((1..=length).map(|n| sample(n, n * 1000, None)).collect());
        for field in [
            &mut input.source_identity,
            &mut input.source_name,
            &mut input.channel_identity,
            &mut input.channel_name,
        ] {
            *field = "a".repeat(128);
        }
        input.channel_identity = "b".repeat(128);
        for field in [
            &mut input.producer_host,
            &mut input.producer_boot,
            &mut input.producer_plan,
            &mut input.producer_play,
        ] {
            *field = "p".repeat(256);
        }
        let bytes = input.clone().encode().unwrap();
        assert!(bytes.len() <= prepared.maximum_bytes() as usize);
        assert_eq!(prepared.decode(&bytes).unwrap(), input);
    }
    assert!(prepared.decode(b"not canonical").is_err());
    let bytes = fixture().encode().unwrap();
    assert!(prepared.decode(&bytes[..bytes.len() - 1]).is_err());
    assert!(prepared
        .decode(&vec![0; prepared.maximum_bytes() as usize + 1])
        .is_err());
    let mut oversized = fixture();
    oversized.producer_host = "p".repeat(257);
    assert!(prepared.decode(&oversized.encode().unwrap()).is_err());
}

fn snapshot(batches: Vec<Batch>) -> Snapshot {
    Snapshot {
        batches: BoundedSequence::try_from_iter(batches).unwrap(),
    }
}
fn second_batch() -> Batch {
    let mut second = batch(vec![
        sample(1, 900_000, Some("21°C")),
        sample(2, 901_000, None),
    ]);
    second.source_identity = "second-history".into();
    second.channel_identity = "second-temperature".into();
    second.producer_host = "reported-host/other".into();
    second.producer_boot = "reported-boot/other".into();
    second
}

#[test]
fn snapshot_keeps_independent_producer_boot_axes_and_existing_single_batch_api() {
    let first = fixture();
    let second = second_batch();
    assert!(first.clone().encode().unwrap().len() <= 4096);
    assert!(second.clone().encode().unwrap().len() <= 4096);
    let original = snapshot(vec![first.clone(), second.clone()]);
    let input = PreparedQuantityObservationSnapshotInput::prepare().unwrap();
    let bytes = original.clone().encode().unwrap();
    let decoded = input.decode(&bytes).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(decoded.batches.as_slice()[0], first);
    assert_eq!(decoded.batches.as_slice()[1], second);
    assert!(bytes.len() <= input.maximum_bytes() as usize);
    assert!(input.maximum_bytes() <= 16 * 1024);
    assert_eq!(
        input.maximum_bytes(),
        resolved_quantity_observation_snapshot_byte_bound().unwrap()
    );
    assert!(checked_quantity_observation_snapshot_type()
        .unwrap()
        .invariants
        .is_empty());
    assert!(input.retained_backing_bytes().unwrap() > 0);
    assert!(
        quantity_observation_snapshot_input_preparation_backing_bound().unwrap()
            > input.projection_workspace_bytes().unwrap()
    );
    assert!(
        resolved_quantity_observation_snapshot_backing_bytes(&decoded).unwrap()
            <= resolved_quantity_observation_snapshot_backing_bound()
    );
    let single = PreparedQuantityObservationInput::prepare().unwrap();
    assert_eq!(
        single.decode(&first.clone().encode().unwrap()).unwrap(),
        first
    );
    assert!(input.decode(&first.encode().unwrap()).is_err());
    assert!(single.decode(&bytes).is_err());
    eprintln!(
        "snapshot bytes: actual {}; admitted {}",
        bytes.len(),
        input.maximum_bytes()
    );
}

#[test]
fn snapshot_refuses_all_root_channel_and_derived_sample_identity_collisions() {
    let first = fixture();
    for (source, channel) in [
        ("meter-history", "other-channel"),
        ("other-source", "meter-voltage"),
        ("meter-voltage", "other-channel"),
        ("meter-history/sample/1", "other-channel"),
        ("other-source", "meter-history/sample/2"),
    ] {
        let mut second = second_batch();
        second.source_identity = source.into();
        second.channel_identity = channel.into();
        assert_eq!(
            validate_quantity_observation_snapshot(&snapshot(vec![first.clone(), second])),
            Err(R::Identity)
        );
    }
    let mut within = first;
    within.channel_identity = "meter-history/sample/1".into();
    assert_eq!(
        validate_quantity_observation_snapshot(&snapshot(vec![within])),
        Err(R::Identity)
    );
}

#[test]
fn snapshot_storage_profile_refuses_large_members_without_shrinking_single_batch_semantics() {
    let input = PreparedQuantityObservationSnapshotInput::prepare().unwrap();
    let mut member = batch(
        (1..=16)
            .map(|n| {
                let mut observation = sample(n, n * 1000, None);
                observation.outcome = Outcome::Unavailable("u".repeat(128));
                observation
            })
            .collect(),
    );
    member.source_name = "s".repeat(128);
    member.channel_name = "c".repeat(128);
    for identity in [
        &mut member.producer_host,
        &mut member.producer_boot,
        &mut member.producer_plan,
        &mut member.producer_play,
    ] {
        *identity = "p".repeat(256);
    }
    let bytes = member.clone().encode().unwrap();
    assert!(bytes.len() > 4096);
    assert!(PreparedQuantityObservationInput::prepare()
        .unwrap()
        .decode(&bytes)
        .is_ok());
    assert_eq!(
        validate_quantity_observation_snapshot(&snapshot(vec![member])),
        Err(R::Capacity)
    );
    assert_eq!(
        validate_quantity_observation_snapshot(&snapshot(vec![])),
        Err(R::Capacity)
    );
    assert!(input.decode(b"not canonical").is_err());
    let valid = snapshot(vec![fixture()]).encode().unwrap();
    assert!(input.decode(&valid[..valid.len() - 1]).is_err());
    assert!(input
        .decode(&vec![0; input.maximum_bytes() as usize + 1])
        .is_err());
}
