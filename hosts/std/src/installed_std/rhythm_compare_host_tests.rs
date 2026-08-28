use super::*;
use conduit_audio::{MusicalPitch, NoteOccurrenceId};

fn host(target: i64, tolerance: u64) -> RhythmCompareHost {
    RhythmCompareHost {
        target_offset_micros: target,
        tolerance_micros: tolerance,
        beats: VecDeque::new(),
        performance: VecDeque::new(),
        performance_closed: false,
        previous_absolute_delta: None,
        beat_type_prefix: conduit_std_catalog::beat_reference_type()
            .canonical_bytes()
            .unwrap(),
        feedback_type_prefix: conduit_std_catalog::timing_feedback_type()
            .canonical_bytes()
            .unwrap(),
        output: Vec::new(),
    }
}

fn note(time: u64, gate: Gate) -> Vec<u8> {
    MusicalNoteEvent::new(
        NoteOccurrenceId(1),
        MusicalPitch::new(440_000, 440_000, 0).unwrap(),
        gate,
        u16::MAX,
        time,
        0,
    )
    .unwrap()
    .encode()
    .to_vec()
}

fn beat(index: u64, expected: u64) -> Vec<u8> {
    StructuredInfoValue::record(
        conduit_std_catalog::beat_reference_type(),
        vec![
            value_field("beat", count_leaf(index)),
            value_field("expected_time_micros", count_leaf(expected)),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn decode(value: Option<&[u8]>) -> StructuredInfoValue {
    StructuredInfoValue::from_canonical_bytes(value.expect("expected feedback")).unwrap()
}

fn property(value: &StructuredInfoValue, name: &str) -> String {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("feedback must be a record")
    };
    let StructuredInfoValueShape::Leaf(bytes) = field(fields, name).unwrap().shape() else {
        panic!("feedback field must be a leaf")
    };
    core::str::from_utf8(bytes).unwrap().to_string()
}

#[test]
fn exact_vectors_report_early_late_recovery_and_deliberate_displacement() {
    let mut comparison = host(0, 25);
    assert!(comparison
        .execute(
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
            &beat(1, 1_000),
        )
        .unwrap()
        .is_none());
    let early = decode(
        comparison
            .execute(
                conduit_std_offers::RHYTHM_PERFORMANCE_HOST_OPERATION,
                &note(800, Gate::On),
            )
            .unwrap(),
    );
    assert_eq!(property(&early, "delta_micros"), "-200");
    assert_eq!(property(&early, "classification"), "early");
    assert_eq!(property(&early, "recovery_state"), "displaced");

    comparison
        .execute(
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
            &beat(2, 2_000),
        )
        .unwrap();
    let recovering = decode(
        comparison
            .execute(
                conduit_std_offers::RHYTHM_PERFORMANCE_HOST_OPERATION,
                &note(2_100, Gate::On),
            )
            .unwrap(),
    );
    assert_eq!(property(&recovering, "classification"), "late");
    assert_eq!(property(&recovering, "recovery_state"), "recovering");

    comparison
        .execute(
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
            &beat(3, 3_000),
        )
        .unwrap();
    let recovered = decode(
        comparison
            .execute(
                conduit_std_offers::RHYTHM_PERFORMANCE_HOST_OPERATION,
                &note(3_020, Gate::On),
            )
            .unwrap(),
    );
    assert_eq!(property(&recovered, "classification"), "on-time");
    assert_eq!(property(&recovered, "recovery_state"), "recovered");

    let mut displaced = host(100, 25);
    displaced
        .execute(
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
            &beat(1, 4_000),
        )
        .unwrap();
    let feedback = decode(
        displaced
            .execute(
                conduit_std_offers::RHYTHM_PERFORMANCE_HOST_OPERATION,
                &note(4_120, Gate::On),
            )
            .unwrap(),
    );
    assert_eq!(property(&feedback, "delta_micros"), "20");
    assert_eq!(property(&feedback, "classification"), "on-time");
}

#[test]
fn note_off_is_ignored_and_drain_emits_each_missed_beat() {
    let mut comparison = host(0, 25);
    assert!(comparison
        .execute(
            conduit_std_offers::RHYTHM_PERFORMANCE_HOST_OPERATION,
            &note(900, Gate::Off),
        )
        .unwrap()
        .is_none());
    comparison
        .execute(
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
            &beat(1, 1_000),
        )
        .unwrap();
    comparison
        .execute(
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
            &beat(2, 2_000),
        )
        .unwrap();
    let first = decode(
        comparison
            .execute(conduit_std_offers::RHYTHM_DRAIN_HOST_OPERATION, b"ignored")
            .unwrap(),
    );
    assert_eq!(property(&first, "beat"), "1");
    assert_eq!(property(&first, "classification"), "missed");
    assert_eq!(property(&first, "observed"), "false");
    let second = decode(
        comparison
            .execute(conduit_std_offers::RHYTHM_DRAIN_HOST_OPERATION, b"ignored")
            .unwrap(),
    );
    assert_eq!(property(&second, "beat"), "2");
    assert!(comparison
        .execute(conduit_std_offers::RHYTHM_DRAIN_HOST_OPERATION, b"ignored")
        .unwrap()
        .is_none());
}

#[test]
fn malformed_identity_and_finite_capacity_refuse_without_output() {
    let mut comparison = host(0, 25);
    assert_eq!(
        comparison.execute(
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
            b"not structured",
        ),
        Err(RhythmCompareRefusal::MalformedReference)
    );
    assert_eq!(
        comparison.execute("conduit.host/wrong@1", &beat(1, 1)),
        Err(RhythmCompareRefusal::WrongOperation)
    );
    for index in 0..conduit_std_catalog::RHYTHM_MAXIMUM_PENDING_BEATS {
        assert!(comparison
            .execute(
                conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
                &beat(u64::from(index) + 1, u64::from(index)),
            )
            .unwrap()
            .is_none());
    }
    assert_eq!(
        comparison.execute(
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION,
            &beat(99, 99),
        ),
        Err(RhythmCompareRefusal::CapacityExhausted)
    );
}
