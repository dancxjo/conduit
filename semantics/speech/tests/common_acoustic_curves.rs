#![cfg(feature = "semantic-bindings")]
#[path = "common_acoustic/common.rs"]
mod common;
use common::*;
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::semantic::*;
use conduit_speech::*;
#[test]
fn all_six_spec_states_survive_exact_step_selection_and_provenance() {
    let known = SpeechProbabilitySpecification::known(4, 3).unwrap();
    let variants = [
        known,
        SpeechProbabilitySpecification::Unknown,
        SpeechProbabilitySpecification::Unspecified,
        SpeechProbabilitySpecification::NotApplicable,
        SpeechProbabilitySpecification::variable(
            BoundedSequence::try_from_iter(vec![
                SpeechUnitInterval::new(4, 1).unwrap(),
                SpeechUnitInterval::new(4, 3).unwrap(),
            ])
            .unwrap(),
        )
        .unwrap(),
        SpeechProbabilitySpecification::gradient(
            SpeechConfidence::new(0.75.into()).unwrap(),
            SpeechUnitInterval::new(4, 3).unwrap(),
        )
        .unwrap(),
    ];
    let mut encodings = vec![];
    for value in variants {
        let curve = probability_curve(vec![probability_segment(
            0,
            1,
            value.clone(),
            AudioTrajectoryInterpolation::Step,
        )]);
        let original = curve.clone().encode().unwrap();
        let prepared = prepare_speech_probability_step_curve(&original).unwrap();
        let receipt = prepared.query(&query(1, 2).encode().unwrap()).unwrap();
        assert_eq!(receipt.original(), &curve);
        assert_eq!(receipt.original_canonical(), original);
        assert_eq!(
            SpeechProbabilitySpecification::decode(receipt.result_canonical()).unwrap(),
            value
        );
        assert_eq!(receipt.result_canonical(), value.clone().encode().unwrap());
        reproduce(receipt.executions());
        encodings.push(receipt.result_canonical().to_vec());
    }
    for i in 0..encodings.len() {
        for j in 0..i {
            assert_ne!(encodings[i], encodings[j]);
        }
    }
}
#[test]
fn endpoint_right_continuity_gaps_foreign_anchor_overlap_and_profile_refusals() {
    let a = SpeechProbabilitySpecification::known(4, 1).unwrap();
    let b = SpeechProbabilitySpecification::known(4, 3).unwrap();
    let curve = probability_curve(vec![
        probability_segment(0, 1, a.clone(), AudioTrajectoryInterpolation::Step),
        probability_segment(1, 2, b.clone(), AudioTrajectoryInterpolation::Step),
    ]);
    let prepared = prepare_speech_probability_step_curve(&curve.encode().unwrap()).unwrap();
    for t in [1, 2] {
        assert_eq!(
            SpeechProbabilitySpecification::decode(
                prepared
                    .query(&query(t, 1).encode().unwrap())
                    .unwrap()
                    .result_canonical()
            )
            .unwrap(),
            b
        );
    }
    assert!(matches!(
        prepared.query(&query(3, 1).encode().unwrap()),
        Err(SpeechCommonAcousticRefusal::MissingCoverage)
    ));
    let foreign = AudioTrajectoryQuery::new(
        AudioTrajectoryAnchor::new(
            AudioOriginIdentity::new(2).unwrap(),
            AudioTimelineIdentity::new(1).unwrap(),
        )
        .unwrap(),
        AudioExactTimeOffset::new(1, 1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepared.query(&foreign.encode().unwrap()),
        Err(SpeechCommonAcousticRefusal::ForeignAnchor)
    ));
    let gap = probability_curve(vec![
        probability_segment(0, 1, a.clone(), AudioTrajectoryInterpolation::Step),
        probability_segment(2, 3, b.clone(), AudioTrajectoryInterpolation::Step),
    ]);
    assert!(matches!(
        prepare_speech_probability_step_curve(&gap.encode().unwrap())
            .unwrap()
            .query(&query(3, 2).encode().unwrap()),
        Err(SpeechCommonAcousticRefusal::MissingCoverage)
    ));
    let overlap = probability_curve(vec![
        probability_segment(0, 2, a.clone(), AudioTrajectoryInterpolation::Step),
        probability_segment(1, 3, b, AudioTrajectoryInterpolation::Step),
    ]);
    assert!(matches!(
        prepare_speech_probability_step_curve(&overlap.encode().unwrap()),
        Err(SpeechCommonAcousticRefusal::UnorderedOrOverlapping)
    ));
    for (start, end, policy) in [
        (1, 1, AudioTrajectoryInterpolation::Step),
        (2, 1, AudioTrajectoryInterpolation::Step),
        (0, 1, AudioTrajectoryInterpolation::Linear),
    ] {
        let curve = probability_curve(vec![probability_segment(start, end, a.clone(), policy)]);
        assert!(prepare_speech_probability_step_curve(&curve.encode().unwrap()).is_err());
    }
}
#[test]
fn forged_known_probability_and_temporal_overflow_profile_refuse() {
    assert!(SpeechUnitInterval::new(1, 2).is_err());
    // The existing generic Known constructor flattens the payload; explicit
    // source-owner re-admission must refuse this forged semantic probability.
    let Ok(forged) = SpeechProbabilitySpecification::known(1, 2) else {
        return;
    };
    let curve = probability_curve(vec![probability_segment(
        0,
        1,
        forged,
        AudioTrajectoryInterpolation::Step,
    )]);
    assert!(matches!(
        prepare_speech_probability_step_curve(&curve.encode().unwrap()),
        Err(SpeechCommonAcousticRefusal::Admission(_))
    ));
    let huge = SpeechProbabilityCurveSegment::new(
        time(u64::MAX, 1),
        AudioTrajectoryInterpolation::Step,
        time(0, 1),
        SpeechProbabilitySpecification::Unknown,
    )
    .unwrap();
    let curve = probability_curve(vec![huge]);
    assert!(prepare_speech_probability_step_curve(&curve.encode().unwrap()).is_err());
    let mut corrupt = probability_curve(vec![probability_segment(
        0,
        1,
        SpeechProbabilitySpecification::Unknown,
        AudioTrajectoryInterpolation::Step,
    )])
    .encode()
    .unwrap();
    corrupt.pop();
    assert!(prepare_speech_probability_step_curve(&corrupt).is_err());
}
#[test]
fn independent_voicing_and_periodicity_controls_overlap_without_aliasing() {
    let a = probability_curve(vec![probability_segment(
        0,
        2,
        SpeechProbabilitySpecification::known(4, 1).unwrap(),
        AudioTrajectoryInterpolation::Step,
    )]);
    let b = probability_curve(vec![probability_segment(
        1,
        3,
        SpeechProbabilitySpecification::known(4, 3).unwrap(),
        AudioTrajectoryInterpolation::Step,
    )]);
    let a = prepare_speech_probability_step_curve(&a.encode().unwrap()).unwrap();
    let b = prepare_speech_probability_step_curve(&b.encode().unwrap()).unwrap();
    let q = query(3, 2).encode().unwrap();
    let ar = a.query(&q).unwrap();
    let br = b.query(&q).unwrap();
    assert_ne!(ar.result_canonical(), br.result_canonical());
    reproduce(ar.executions());
    reproduce(br.executions());
}

#[path = "common_acoustic/probability_parent_law.rs"]
mod probability_parent_law;
#[test]
fn variable_and_gradient_cannot_reinject_invalid_known_parent_through_curve() {
    use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
    for candidate in probability_parent_law::forged_candidates() {
        let curve = probability_curve(vec![probability_segment(
            0,
            1,
            SpeechProbabilitySpecification::Unknown,
            AudioTrajectoryInterpolation::Step,
        )])
        .into_structured()
        .unwrap();
        let segments = conduit_plot::rust_binding::record_field_value(&curve, "segments").unwrap();
        let StructuredInfoValueShape::Collection(values) = segments.shape() else {
            unreachable!()
        };
        let segment = probability_parent_law::replace_record(values[0].clone(), "value", candidate);
        let segments =
            StructuredInfoValue::sequence(segments.value_type().clone(), vec![segment]).unwrap();
        let curve = probability_parent_law::replace_record(curve, "segments", segments);
        assert!(prepare_speech_probability_step_curve(&curve.canonical_bytes().unwrap()).is_err());
    }
}
