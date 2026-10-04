//! Checked portable evaluation must agree with every finite condition carrier.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::{StructuredInfoType, StructuredInfoValue};
use std::vec;
fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()).unwrap()
}
fn tag(value: SpeechContextDecision) -> &'static str {
    match value {
        SpeechContextDecision::matched => "matched",
        SpeechContextDecision::mismatched => "mismatched",
        SpeechContextDecision::requirement_unresolved => "requirement_unresolved",
        SpeechContextDecision::observation_unresolved => "observation_unresolved",
    }
}
const STATES: [(&str, SpeechSpecificationState); 6] = [
    ("known", SpeechSpecificationState::known),
    ("unknown", SpeechSpecificationState::unknown),
    ("unspecified", SpeechSpecificationState::unspecified),
    ("not_applicable", SpeechSpecificationState::not_applicable),
    ("variable", SpeechSpecificationState::variable),
    ("gradient", SpeechSpecificationState::gradient),
];
const PRESENCES: [(&str, SpeechNeighborPresence); 4] = [
    ("absent", SpeechNeighborPresence::absent),
    ("unknown", SpeechNeighborPresence::unknown),
    ("segment", SpeechNeighborPresence::segment),
    ("boundary", SpeechNeighborPresence::boundary),
];
#[test]
fn explicit_style_carriers_agree_with_checked_portable_law() {
    let checked = program("speech_not_careful_style");
    for (state_tag, observation) in STATES {
        for careful in [false, true] {
            let input = record(
                &checked.input_type,
                &[
                    (
                        "observation",
                        variant(field_type(&checked.input_type, "observation"), state_tag),
                    ),
                    (
                        "careful",
                        leaf(
                            field_type(&checked.input_type, "careful"),
                            &[u8::from(careful)],
                        ),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let actual = speech_not_careful_style(SpeechStyleConditionInput {
                observation,
                careful,
            })
            .unwrap();
            assert_eq!(
                checked.evaluate(&input).unwrap(),
                variant(&checked.output_type, tag(actual))
                    .canonical_bytes()
                    .unwrap()
            );
        }
    }
}
#[test]
fn every_stress_mask_presence_state_and_tag_agrees_with_checked_portable_law() {
    let checked = program("speech_stress_condition");
    let mut count = 0;
    for (presence_tag, presence) in PRESENCES {
        for (state_tag, observation) in STATES {
            for stress in 0i32..4 {
                for mask in 0u8..16 {
                    let allowed = core::array::from_fn::<_, 4, _>(|index| mask & (1 << index) != 0);
                    let mut fields = vec![
                        (
                            "presence",
                            variant(field_type(&checked.input_type, "presence"), presence_tag),
                        ),
                        (
                            "observation",
                            variant(field_type(&checked.input_type, "observation"), state_tag),
                        ),
                        (
                            "stress",
                            leaf(
                                field_type(&checked.input_type, "stress"),
                                &stress.to_le_bytes(),
                            ),
                        ),
                    ];
                    for (index, name) in ["allow0", "allow1", "allow2", "allow3"]
                        .into_iter()
                        .enumerate()
                    {
                        fields.push((
                            name,
                            leaf(
                                field_type(&checked.input_type, name),
                                &[u8::from(allowed[index])],
                            ),
                        ));
                    }
                    let input = record(&checked.input_type, &fields)
                        .canonical_bytes()
                        .unwrap();
                    let actual = speech_stress_condition(SpeechStressConditionInput {
                        presence,
                        observation,
                        stress,
                        allow0: allowed[0],
                        allow1: allowed[1],
                        allow2: allowed[2],
                        allow3: allowed[3],
                    })
                    .unwrap();
                    assert_eq!(
                        checked.evaluate(&input).unwrap(),
                        variant(&checked.output_type, tag(actual))
                            .canonical_bytes()
                            .unwrap()
                    );
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 1536);
}
#[test]
fn feature_presence_policy_agrees_with_checked_portable_law() {
    let checked = program("speech_feature_condition");
    for (presence_tag, presence) in PRESENCES {
        for feature in [
            SpeechContextDecision::matched,
            SpeechContextDecision::mismatched,
            SpeechContextDecision::requirement_unresolved,
            SpeechContextDecision::observation_unresolved,
        ] {
            let input = record(
                &checked.input_type,
                &[
                    (
                        "presence",
                        variant(field_type(&checked.input_type, "presence"), presence_tag),
                    ),
                    (
                        "feature",
                        variant(field_type(&checked.input_type, "feature"), tag(feature)),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let actual =
                speech_feature_condition(SpeechFeatureConditionInput { presence, feature })
                    .unwrap();
            assert_eq!(
                checked.evaluate(&input).unwrap(),
                variant(&checked.output_type, tag(actual))
                    .canonical_bytes()
                    .unwrap()
            );
        }
    }
}
