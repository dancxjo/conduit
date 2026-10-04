#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{rule_stress::*, semantic::*};
#[test]
fn stress_singletons_and_sets_keep_exact_sides_and_original_specs() {
    let stresses = [
        SpeechStress::Primary,
        SpeechStress::Secondary,
        SpeechStress::Unstressed,
        SpeechStress::Reduced,
    ];
    for expected in stresses {
        let condition = SpeechRuleCondition::previous_stress(expected).unwrap();
        for actual in stresses {
            let specification = StressSpecification::known(actual).unwrap();
            let receipt = compare_stress_condition(
                &condition,
                StressObservation::Segment(&specification),
                StressObservation::Absent,
            )
            .unwrap();
            assert!(core::ptr::eq(receipt.condition(), &condition));
            assert!(
                matches!(receipt.observation(), StressObservation::Segment(value) if core::ptr::eq(*value, &specification))
            );
            assert_eq!(
                receipt.decision(),
                if expected == actual {
                    &SpeechContextDecision::Matched
                } else {
                    &SpeechContextDecision::Mismatched
                }
            );
        }
    }
    let condition = SpeechRuleCondition::next_stress_in(
        BoundedSequence::try_from_iter([SpeechStress::Primary, SpeechStress::Reduced]).unwrap(),
    )
    .unwrap();
    for actual in stresses {
        let specification = StressSpecification::known(actual).unwrap();
        assert_eq!(
            compare_stress_condition(
                &condition,
                StressObservation::Absent,
                StressObservation::Segment(&specification)
            )
            .unwrap()
            .decision(),
            if matches!(actual, SpeechStress::Primary | SpeechStress::Reduced) {
                &SpeechContextDecision::Matched
            } else {
                &SpeechContextDecision::Mismatched
            }
        );
    }
}
#[test]
fn absent_boundary_and_all_unresolved_stress_states_remain_explicit() {
    let condition = SpeechRuleCondition::previous_stress(SpeechStress::Primary).unwrap();
    let boundary = SpeechBoundarySpecification::unknown();
    for observation in [
        StressObservation::Absent,
        StressObservation::Boundary(&boundary),
    ] {
        assert_eq!(
            compare_stress_condition(&condition, observation, StressObservation::Unknown)
                .unwrap()
                .decision(),
            &SpeechContextDecision::Mismatched
        );
    }
    let specifications = [
        StressSpecification::unknown(),
        StressSpecification::unspecified(),
        StressSpecification::not_applicable(),
        StressSpecification::variable(
            BoundedSequence::try_from_iter([SpeechStress::Primary]).unwrap(),
        )
        .unwrap(),
        StressSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            SpeechStress::Primary,
        )
        .unwrap(),
    ];
    for specification in &specifications {
        assert_eq!(
            compare_stress_condition(
                &condition,
                StressObservation::Segment(specification),
                StressObservation::Absent
            )
            .unwrap()
            .decision(),
            &SpeechContextDecision::ObservationUnresolved
        );
    }
    assert_eq!(
        compare_stress_condition(
            &condition,
            StressObservation::Unknown,
            StressObservation::Absent
        )
        .unwrap()
        .decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
    assert!(matches!(
        compare_stress_condition(
            &SpeechRuleCondition::not_careful_style(),
            StressObservation::Absent,
            StressObservation::Absent
        ),
        Err(StressConditionRefusal::WrongCondition)
    ));
}
