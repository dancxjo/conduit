#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    neighbor_match::NeighborObservation, rule_conditions::*, rule_stress::StressObservation,
    semantic::*,
};
fn evidence(style: &SpeechCarefulStyleSpecification) -> ConditionEvidence<'_> {
    ConditionEvidence {
        before: NeighborObservation::Unknown,
        after: NeighborObservation::Absent,
        before_stress: StressObservation::Unknown,
        after_stress: StressObservation::Absent,
        careful_style: style,
    }
}
#[test]
fn empty_and_full_condition_sets_retain_every_original_and_do_not_short_circuit() {
    let style = SpeechCarefulStyleSpecification::known(false).unwrap();
    let empty = BoundedSequence::try_from_iter([]).unwrap();
    let compared = compare_conditions(&empty, evidence(&style)).unwrap();
    assert_eq!(compared.decision(), &SpeechContextDecision::Matched);
    assert_eq!(compared.receipts().count(), 0);
    let conditions = BoundedSequence::try_from_iter([
        SpeechRuleCondition::next_stress(SpeechStress::Primary).unwrap(),
        SpeechRuleCondition::previous_stress(SpeechStress::Primary).unwrap(),
        SpeechRuleCondition::not_careful_style(),
        SpeechRuleCondition::not_careful_style(),
        SpeechRuleCondition::not_careful_style(),
        SpeechRuleCondition::not_careful_style(),
        SpeechRuleCondition::not_careful_style(),
        SpeechRuleCondition::not_careful_style(),
    ])
    .unwrap();
    let compared = compare_conditions(&conditions, evidence(&style)).unwrap();
    assert!(core::ptr::eq(compared.conditions(), &conditions));
    assert_eq!(compared.decision(), &SpeechContextDecision::Mismatched);
    assert_eq!(compared.receipts().count(), 8);
    for (receipt, original) in compared.receipts().zip(conditions.as_slice()) {
        assert!(core::ptr::eq(receipt.condition(), original));
    }
    let decisions: Vec<_> = compared
        .receipts()
        .map(|receipt| receipt.decision().unwrap())
        .collect();
    assert_eq!(decisions[0], SpeechContextDecision::Mismatched);
    assert_eq!(decisions[1], SpeechContextDecision::ObservationUnresolved);
    assert_eq!(decisions[7], SpeechContextDecision::Matched);
    assert!(BoundedSequence::<_, 8>::try_from_iter(
        (0..9).map(|_| SpeechRuleCondition::not_careful_style())
    )
    .is_err());
}
#[test]
fn unsupported_syntax_after_mismatch_is_an_indexed_refusal() {
    let style = SpeechCarefulStyleSpecification::known(true).unwrap();
    let conditions = BoundedSequence::try_from_iter([
        SpeechRuleCondition::not_careful_style(),
        SpeechRuleCondition::current_word_has_syntactic_link(SpeechSyntacticLinkKind::Vocative)
            .unwrap(),
    ])
    .unwrap();
    assert!(matches!(
        compare_conditions(&conditions, evidence(&style)),
        Err(ConditionRefusal {
            index: 1,
            reason: ConditionRefusalReason::UnsupportedSyntax
        })
    ));
}
#[test]
fn missing_style_keeps_uncertainty_instead_of_defaulting_to_careful_or_casual() {
    let style = SpeechCarefulStyleSpecification::unknown();
    let conditions =
        BoundedSequence::try_from_iter([SpeechRuleCondition::not_careful_style()]).unwrap();
    assert_eq!(
        compare_conditions(&conditions, evidence(&style))
            .unwrap()
            .decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
}
