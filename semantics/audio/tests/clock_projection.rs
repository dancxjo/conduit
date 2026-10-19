use conduit_audio::*;
use conduit_data::{ClockRelation, ClockRelationQuality};
use conduit_plot::rust_binding::NativeRustBinding;

fn anchor(origin: u64) -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(origin).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn relation(source_ticks: u64, target_ticks: u64, target_anchor: u64) -> ClockRelation {
    ClockRelation::new(
        "declared-seconds-to-control@1".into(),
        ClockRelationQuality::Exact,
        37,
        "authored-seconds".into(),
        source_ticks,
        target_anchor,
        "declared-control".into(),
        target_ticks,
    )
    .unwrap()
}
fn binding(relation: ClockRelation) -> AudioClockBinding {
    AudioClockBinding::new(
        anchor(2),
        relation,
        "authored-seconds".into(),
        37,
        AudioClockSourceUnit::Seconds,
        "declared-control".into(),
    )
    .unwrap()
}
fn query(origin: u64, numerator: u64, denominator: u64) -> Vec<u8> {
    AudioTrajectoryQuery::new(
        anchor(origin),
        AudioExactTimeOffset::new(denominator, numerator).unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
}

#[test]
fn nonzero_original_anchors_and_unreduced_fraction_survive_exact_clock_projection() {
    let original = binding(relation(1, 100, 900));
    let canonical = original.encode().unwrap();
    let prepared = PreparedAudioClockBinding::new(&canonical).unwrap();
    let q = query(2, 2, 6);
    let receipt = prepared.project(&q).unwrap();
    assert_eq!(receipt.binding(), &original);
    assert_eq!(receipt.binding_canonical(), canonical);
    assert_eq!(receipt.query_canonical(), q);
    let raw = receipt.result().raw();
    assert_eq!(
        (
            *raw.whole_ticks(),
            *raw.remainder_numerator(),
            *raw.remainder_denominator()
        ),
        (933, 2, 6)
    );
    assert_eq!(*receipt.binding().relation().source_anchor(), 37);
    assert_eq!(receipt.binding_executions().len(), 1);
    assert_eq!(receipt.executions().len(), 2);
    assert_eq!(
        AudioClockProjectionResult::decode(receipt.admitted_canonical()).unwrap(),
        *receipt.result()
    );
    assert!(receipt
        .executions()
        .iter()
        .all(|e| !e.source_program_hex().is_empty()
            && !e.input_canonical().is_empty()
            && !e.output_canonical().is_empty()));
}

#[test]
fn independent_clock_ratios_match_wide_reference_and_preserve_origins() {
    for (source_ticks, target_ticks, numerator, denominator) in [
        (1, 8000, 1, 10),
        (1, 16000, 1, 10),
        (1, 48000, 1, 10),
        (3, 100, 1, 3),
        (
            u32::MAX as u64,
            u32::MAX as u64,
            u32::MAX as u64,
            u32::MAX as u64,
        ),
    ] {
        let original = binding(relation(source_ticks, target_ticks, 7));
        let prepared = PreparedAudioClockBinding::new(&original.encode().unwrap()).unwrap();
        let receipt = prepared.project(&query(2, numerator, denominator)).unwrap();
        let n = i128::from(numerator) * i128::from(target_ticks);
        let d = i128::from(denominator) * i128::from(source_ticks);
        assert_eq!(*receipt.result().raw().whole_ticks(), 7 + n / d);
        assert_eq!(*receipt.result().raw().remainder_numerator(), n % d);
        assert_eq!(*receipt.result().raw().remainder_denominator(), d);
        assert_eq!(receipt.binding(), &original);
    }
}

#[test]
fn foreign_missing_estimated_and_out_of_profile_relations_refuse() {
    let prepared =
        PreparedAudioClockBinding::new(&binding(relation(1, 100, 0)).encode().unwrap()).unwrap();
    assert!(matches!(
        prepared.project(&query(3, 1, 3)),
        Err(AudioClockProjectionRefusal::ForeignOrUnsupportedQuery)
    ));
    assert!(matches!(
        prepared.project(&query(2, u64::MAX, 3)),
        Err(AudioClockProjectionRefusal::ForeignOrUnsupportedQuery)
    ));
    assert!(PreparedAudioClockBinding::new(&[]).is_err());
    let estimated = ClockRelation::new(
        "estimated".into(),
        ClockRelationQuality::estimated(conduit_core::Quantity::new(
            1,
            conduit_core::Unit::Millisecond,
        ))
        .unwrap(),
        37,
        "authored-seconds".into(),
        1,
        0,
        "declared-control".into(),
        100,
    )
    .unwrap();
    assert!(matches!(
        PreparedAudioClockBinding::new(&binding(estimated).encode().unwrap()),
        Err(AudioClockProjectionRefusal::UnsupportedBinding)
    ));
    assert!(matches!(
        PreparedAudioClockBinding::new(&binding(relation(u64::MAX, 100, 0)).encode().unwrap()),
        Err(AudioClockProjectionRefusal::UnsupportedBinding)
    ));
    let foreign = ClockRelation::new(
        "foreign".into(),
        ClockRelationQuality::Exact,
        37,
        "foreign-source".into(),
        1,
        0,
        "declared-control".into(),
        100,
    )
    .unwrap();
    assert!(matches!(
        PreparedAudioClockBinding::new(&binding(foreign).encode().unwrap()),
        Err(AudioClockProjectionRefusal::UnsupportedBinding)
    ));
    let overflow =
        PreparedAudioClockBinding::new(&binding(relation(1, 100, u64::MAX)).encode().unwrap())
            .unwrap();
    assert!(matches!(
        overflow.project(&query(2, 1, 1)),
        Err(AudioClockProjectionRefusal::Admission(_))
    ));
}

#[test]
fn mismatched_origin_and_forged_projection_cannot_reenter_the_checked_result() {
    let original = binding(relation(1, 100, 900));
    let foreign_origin = AudioClockBinding::new(
        anchor(2),
        original.relation().clone(),
        "authored-seconds".into(),
        38,
        AudioClockSourceUnit::Seconds,
        "declared-control".into(),
    )
    .unwrap();
    assert!(matches!(
        PreparedAudioClockBinding::new(&foreign_origin.encode().unwrap()),
        Err(AudioClockProjectionRefusal::UnsupportedBinding)
    ));
    let prepared = PreparedAudioClockBinding::new(&original.encode().unwrap()).unwrap();
    let receipt = prepared.project(&query(2, 1, 3)).unwrap();
    let forged = AudioClockProjectionRaw::new(3, 1, 934).unwrap();
    assert!(AudioClockProjectionResult::new(receipt.result().eligible().clone(), forged).is_err());
}
