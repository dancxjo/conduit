#![cfg(feature = "semantic-bindings")]
use conduit_core::{IeeeF32, IeeeF64};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{feature_match::*, semantic::*};
fn values() -> Vec<SpeechFeatureValue> {
    vec![
        SpeechFeatureValue::boolean(true).unwrap(),
        SpeechFeatureValue::category("voiced".into()).unwrap(),
        SpeechFeatureValue::number(IeeeF64::from_value(0.5)).unwrap(),
        SpeechFeatureValue::vector(
            BoundedSequence::try_from_iter([IeeeF32::from_value(0.5)]).unwrap(),
        )
        .unwrap(),
        SpeechFeatureValue::text("voiced".into()).unwrap(),
    ]
}
#[test]
fn full_feature_value_domain_uses_native_equality_without_cross_domain_coercion() {
    let values = values();
    for (i, expected) in values.iter().enumerate() {
        for (j, actual) in values.iter().enumerate() {
            let requirement = FeatureSpecification::known(expected.clone()).unwrap();
            let observation = FeatureSpecification::known(actual.clone()).unwrap();
            let receipt = compare_feature(&requirement, &observation).unwrap();
            assert!(core::ptr::eq(receipt.requirement(), &requirement));
            assert!(core::ptr::eq(receipt.observation(), &observation));
            assert_eq!(
                receipt.decision(),
                if i == j {
                    &SpeechContextDecision::Matched
                } else {
                    &SpeechContextDecision::Mismatched
                }
            );
        }
    }
}
fn states(value: SpeechFeatureValue) -> [FeatureSpecification; 6] {
    [
        FeatureSpecification::known(value.clone()).unwrap(),
        FeatureSpecification::unknown(),
        FeatureSpecification::unspecified(),
        FeatureSpecification::not_applicable(),
        FeatureSpecification::variable(BoundedSequence::try_from_iter([value.clone()]).unwrap())
            .unwrap(),
        FeatureSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            value,
        )
        .unwrap(),
    ]
}
#[test]
fn every_feature_specification_state_keeps_its_original_evidence() {
    let requirements = states(SpeechFeatureValue::boolean(true).unwrap());
    for observations in [
        states(SpeechFeatureValue::boolean(true).unwrap()),
        states(SpeechFeatureValue::boolean(false).unwrap()),
    ] {
        for (r, requirement) in requirements.iter().enumerate() {
            for (o, observation) in observations.iter().enumerate() {
                let receipt = compare_feature(requirement, observation).unwrap();
                let expected = if r == 2 {
                    SpeechContextDecision::Matched
                } else if r != 0 {
                    SpeechContextDecision::RequirementUnresolved
                } else if o != 0 {
                    SpeechContextDecision::ObservationUnresolved
                } else if requirement == observation {
                    SpeechContextDecision::Matched
                } else {
                    SpeechContextDecision::Mismatched
                };
                assert_eq!(receipt.decision(), &expected);
                assert!(core::ptr::eq(receipt.requirement(), requirement));
                assert!(core::ptr::eq(receipt.observation(), observation));
            }
        }
    }
}

#[test]
fn native_feature_law_preserves_ieee_bits_and_full_vector_bound() {
    let values = [
        0_u64,
        1_u64 << 63,
        0x7ff8_0000_0000_0001,
        0x7ff8_0000_0000_0002,
    ];
    for expected in values {
        for actual in values {
            let requirement = FeatureSpecification::known(
                SpeechFeatureValue::number(IeeeF64::from_bits(expected)).unwrap(),
            )
            .unwrap();
            let observation = FeatureSpecification::known(
                SpeechFeatureValue::number(IeeeF64::from_bits(actual)).unwrap(),
            )
            .unwrap();
            assert_eq!(
                compare_feature(&requirement, &observation)
                    .unwrap()
                    .decision(),
                if expected == actual {
                    &SpeechContextDecision::Matched
                } else {
                    &SpeechContextDecision::Mismatched
                }
            );
        }
    }
    let vector = SpeechFeatureValue::vector(
        BoundedSequence::try_from_iter([IeeeF32::from_bits(0x7fc0_0001); 8]).unwrap(),
    )
    .unwrap();
    let requirement = FeatureSpecification::known(vector.clone()).unwrap();
    let observation = FeatureSpecification::known(vector).unwrap();
    assert_eq!(
        compare_feature(&requirement, &observation)
            .unwrap()
            .decision(),
        &SpeechContextDecision::Matched
    );
    assert!(BoundedSequence::<IeeeF32, 8>::try_from_iter([IeeeF32::from_value(0.0); 9]).is_err());
}
