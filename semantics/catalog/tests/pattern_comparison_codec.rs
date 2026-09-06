//! The prepared byte codec agrees with the independent semantic value contract.
use conduit_semantic_catalog::{
    compare_normalized_patterns, normalized_value, BoundedPatternComparisonCodec,
    PatternComparisonInput::{Candidate, Template},
    PatternComparisonRefusal, MAXIMUM_ABSOLUTE_METRIC,
};

#[test]
fn either_arrival_order_preserves_exact_score_and_tolerance_boundary() {
    let candidate = normalized_value(&[500_000, 1_000_000, 760_000]).unwrap();
    let template = normalized_value(&[500_000, 1_000_000, 700_000]).unwrap();
    let candidate_bytes = candidate.canonical_bytes().unwrap();
    let template_bytes = template.canonical_bytes().unwrap();
    for tolerance in [0, 59_999, 60_000, 1_000_000] {
        let expected =
            compare_normalized_patterns(&candidate, &template, MAXIMUM_ABSOLUTE_METRIC, tolerance)
                .unwrap()
                .canonical_bytes()
                .unwrap();
        for reverse in [false, true] {
            let mut codec = BoundedPatternComparisonCodec::new(tolerance).unwrap();
            let mut inputs = [
                (Candidate, candidate_bytes.as_slice()),
                (Template, template_bytes.as_slice()),
            ];
            if reverse {
                inputs.reverse();
            }
            assert!(codec.execute(inputs[0].0, inputs[0].1).unwrap().is_none());
            assert_eq!(
                codec.execute(inputs[1].0, inputs[1].1).unwrap(),
                Some(expected.as_slice())
            );
        }
    }
}

#[test]
fn duplicate_input_and_length_mismatch_remain_distinct_refusals() {
    let one = normalized_value(&[1]).unwrap().canonical_bytes().unwrap();
    let two = normalized_value(&[1, 2])
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut codec = BoundedPatternComparisonCodec::new(0).unwrap();
    assert!(codec.execute(Candidate, &one).unwrap().is_none());
    assert_eq!(
        codec.execute(Candidate, &one),
        Err(PatternComparisonRefusal::Malformed)
    );
    assert_eq!(
        codec.execute(Template, &two),
        Err(PatternComparisonRefusal::LengthMismatch)
    );
    assert!(BoundedPatternComparisonCodec::new(1_000_001).is_err());
}
