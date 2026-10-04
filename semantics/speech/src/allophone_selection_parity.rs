//! Finite compiled/checked-portable choice laws, including shared status identity.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::{StructuredInfoType, StructuredInfoValue};
fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()).unwrap()
}
fn reason(value: SpeechContextDecision) -> &'static str {
    match value {
        SpeechContextDecision::matched => "matched",
        SpeechContextDecision::mismatched => "mismatched",
        SpeechContextDecision::requirement_unresolved => "requirement_unresolved",
        SpeechContextDecision::observation_unresolved => "observation_unresolved",
    }
}
fn outcome(value: SpeechAllophoneChoiceOutcome) -> &'static str {
    match value {
        SpeechAllophoneChoiceOutcome::none => "none",
        SpeechAllophoneChoiceOutcome::selected_allophone => "selected_allophone",
        SpeechAllophoneChoiceOutcome::deferred => "deferred",
        SpeechAllophoneChoiceOutcome::selected_default => "selected_default",
    }
}
fn state(ty: &StructuredInfoType, value: SpeechAllophoneChoiceState) -> StructuredInfoValue {
    record(
        ty,
        &[
            (
                "outcome",
                variant(field_type(ty, "outcome"), outcome(value.outcome)),
            ),
            (
                "index",
                leaf(field_type(ty, "index"), &value.index.to_le_bytes()),
            ),
            (
                "reason",
                variant(field_type(ty, "reason"), reason(value.reason)),
            ),
        ],
    )
}
fn states() -> impl Iterator<Item = SpeechAllophoneChoiceState> {
    [
        SpeechAllophoneChoiceOutcome::none,
        SpeechAllophoneChoiceOutcome::selected_allophone,
        SpeechAllophoneChoiceOutcome::deferred,
        SpeechAllophoneChoiceOutcome::selected_default,
    ]
    .into_iter()
    .flat_map(|outcome| {
        (0..8).flat_map(move |index| {
            [
                SpeechContextDecision::matched,
                SpeechContextDecision::mismatched,
                SpeechContextDecision::requirement_unresolved,
                SpeechContextDecision::observation_unresolved,
            ]
            .into_iter()
            .filter(move |reason| {
                !matches!(outcome, SpeechAllophoneChoiceOutcome::deferred)
                    || matches!(
                        reason,
                        SpeechContextDecision::requirement_unresolved
                            | SpeechContextDecision::observation_unresolved
                    )
            })
            .map(move |reason| SpeechAllophoneChoiceState {
                outcome,
                index,
                reason,
            })
        })
    })
}
#[test]
fn every_legal_priority_fold_carrier_matches_checked_portable_evaluation() {
    let checked = program("speech_allophone_choice_step");
    let mut count = 0;
    for previous in states() {
        for index in 0u32..8 {
            for admitted in [false, true] {
                for decision in [
                    SpeechContextDecision::matched,
                    SpeechContextDecision::mismatched,
                    SpeechContextDecision::requirement_unresolved,
                    SpeechContextDecision::observation_unresolved,
                ] {
                    let input = record(
                        &checked.input_type,
                        &[
                            (
                                "state",
                                state(field_type(&checked.input_type, "state"), previous),
                            ),
                            (
                                "index",
                                leaf(
                                    field_type(&checked.input_type, "index"),
                                    &index.to_le_bytes(),
                                ),
                            ),
                            (
                                "admitted",
                                leaf(
                                    field_type(&checked.input_type, "admitted"),
                                    &[u8::from(admitted)],
                                ),
                            ),
                            (
                                "decision",
                                variant(
                                    field_type(&checked.input_type, "decision"),
                                    reason(decision),
                                ),
                            ),
                        ],
                    )
                    .canonical_bytes()
                    .unwrap();
                    let actual = speech_allophone_choice_step(SpeechAllophoneChoiceInput {
                        state: previous,
                        index,
                        admitted,
                        decision,
                    })
                    .unwrap();
                    assert_eq!(
                        checked.evaluate(&input).unwrap(),
                        state(&checked.output_type, actual)
                            .canonical_bytes()
                            .unwrap()
                    );
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 7168);
}
#[test]
fn every_finish_carrier_matches_checked_portable_evaluation() {
    let checked = program("speech_allophone_choice_finish");
    let mut count = 0;
    for previous in states() {
        for allow_default in [false, true] {
            for default_available in [false, true] {
                let input = record(
                    &checked.input_type,
                    &[
                        (
                            "state",
                            state(field_type(&checked.input_type, "state"), previous),
                        ),
                        (
                            "allow_default",
                            leaf(
                                field_type(&checked.input_type, "allow_default"),
                                &[u8::from(allow_default)],
                            ),
                        ),
                        (
                            "default_available",
                            leaf(
                                field_type(&checked.input_type, "default_available"),
                                &[u8::from(default_available)],
                            ),
                        ),
                    ],
                )
                .canonical_bytes()
                .unwrap();
                let actual = speech_allophone_choice_finish(SpeechAllophoneChoiceFinishInput {
                    state: previous,
                    allow_default,
                    default_available,
                })
                .unwrap();
                assert_eq!(
                    checked.evaluate(&input).unwrap(),
                    state(&checked.output_type, actual)
                        .canonical_bytes()
                        .unwrap()
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 448);
}
#[test]
fn every_status_mask_matches_portable_law_and_uses_the_original_native_type() {
    let checked = program("speech_rule_status_choice");
    assert_eq!(
        field_type(&checked.input_type, "status"),
        &crate::semantic::SpeechRuleStatus::semantic_type().unwrap()
    );
    for (tag, status) in [
        ("productive", SpeechRuleStatus::productive),
        ("lexicalized", SpeechRuleStatus::lexicalized),
        ("optional", SpeechRuleStatus::optional),
        ("style_dependent", SpeechRuleStatus::style_dependent),
        ("experimental", SpeechRuleStatus::experimental),
    ] {
        for mask in 0u8..32 {
            let policy = SpeechAllophoneChoicePolicy {
                productive: mask & 1 != 0,
                lexicalized: mask & 2 != 0,
                optional: mask & 4 != 0,
                style_dependent: mask & 8 != 0,
                experimental: mask & 16 != 0,
                allow_default: false,
            };
            let ty = field_type(&checked.input_type, "policy");
            let policy_value = record(
                ty,
                &[
                    "productive",
                    "lexicalized",
                    "optional",
                    "style_dependent",
                    "experimental",
                    "allow_default",
                ]
                .into_iter()
                .enumerate()
                .map(|(index, name)| {
                    (
                        name,
                        leaf(
                            field_type(ty, name),
                            &[u8::from(index < 5 && mask & (1 << index) != 0)],
                        ),
                    )
                })
                .collect::<std::vec::Vec<_>>(),
            );
            let input = record(
                &checked.input_type,
                &[
                    (
                        "status",
                        variant(field_type(&checked.input_type, "status"), tag),
                    ),
                    ("policy", policy_value),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let actual =
                speech_rule_status_choice(SpeechRuleStatusChoiceInput { status, policy }).unwrap();
            assert_eq!(checked.evaluate(&input).unwrap(), [u8::from(actual)]);
        }
    }
}
#[test]
fn eligibility_and_request_state_laws_match_checked_portable_evaluation() {
    let checked = program("speech_allophone_candidate_eligible");
    for status_allowed in [false, true] {
        for phone_compatible in [false, true] {
            let input = record(
                &checked.input_type,
                &[
                    (
                        "status_allowed",
                        leaf(
                            field_type(&checked.input_type, "status_allowed"),
                            &[u8::from(status_allowed)],
                        ),
                    ),
                    (
                        "phone_compatible",
                        leaf(
                            field_type(&checked.input_type, "phone_compatible"),
                            &[u8::from(phone_compatible)],
                        ),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let actual = speech_allophone_candidate_eligible(SpeechAllophoneCandidateEligibility {
                status_allowed,
                phone_compatible,
            })
            .unwrap();
            assert_eq!(checked.evaluate(&input).unwrap(), [u8::from(actual)]);
        }
    }
    let checked = program("speech_phone_choice_requirement_supported");
    for (tag, state) in [
        ("known", SpeechSpecificationState::known),
        ("unknown", SpeechSpecificationState::unknown),
        ("unspecified", SpeechSpecificationState::unspecified),
        ("not_applicable", SpeechSpecificationState::not_applicable),
        ("variable", SpeechSpecificationState::variable),
        ("gradient", SpeechSpecificationState::gradient),
    ] {
        let input = variant(&checked.input_type, tag).canonical_bytes().unwrap();
        let actual = speech_phone_choice_requirement_supported(state).unwrap();
        assert_eq!(checked.evaluate(&input).unwrap(), [u8::from(actual)]);
    }
}
