#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{rule_style::*, semantic::*};
#[test]
fn explicit_style_keeps_all_six_states_and_original_condition() {
    let condition = SpeechRuleCondition::not_careful_style();
    let observations = [
        SpeechCarefulStyleSpecification::known(false).unwrap(),
        SpeechCarefulStyleSpecification::known(true).unwrap(),
        SpeechCarefulStyleSpecification::unknown(),
        SpeechCarefulStyleSpecification::unspecified(),
        SpeechCarefulStyleSpecification::not_applicable(),
        SpeechCarefulStyleSpecification::variable(
            BoundedSequence::try_from_iter([false, true]).unwrap(),
        )
        .unwrap(),
        SpeechCarefulStyleSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            false,
        )
        .unwrap(),
    ];
    for (index, observation) in observations.iter().enumerate() {
        let receipt = compare_not_careful_style(&condition, observation).unwrap();
        assert!(core::ptr::eq(receipt.condition(), &condition));
        assert!(core::ptr::eq(receipt.observation(), observation));
        assert_eq!(
            receipt.decision(),
            match index {
                0 => &SpeechContextDecision::Matched,
                1 => &SpeechContextDecision::Mismatched,
                _ => &SpeechContextDecision::ObservationUnresolved,
            }
        );
    }
    let other = SpeechRuleCondition::previous_stress(SpeechStress::Primary).unwrap();
    assert!(matches!(
        compare_not_careful_style(&other, &observations[0]),
        Err(StyleConditionRefusal::WrongCondition)
    ));
}
