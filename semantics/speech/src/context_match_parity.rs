//! Independent portable evaluation of every specification carrier combination.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::StructuredInfoValue;
#[test]
fn every_context_carrier_matches_the_checked_portable_program() {
    let checked = program("speech_context_compare");
    let states = [
        ("known", SpeechSpecificationState::known),
        ("unknown", SpeechSpecificationState::unknown),
        ("unspecified", SpeechSpecificationState::unspecified),
        ("not_applicable", SpeechSpecificationState::not_applicable),
        ("variable", SpeechSpecificationState::variable),
        ("gradient", SpeechSpecificationState::gradient),
    ];
    for (requirement_tag, requirement) in states {
        for (observation_tag, observation) in states {
            for (requirement_value, observation_value) in
                (0_i32..=8).flat_map(|a| (0_i32..=8).map(move |b| (a, b)))
            {
                let input = record(
                    &checked.input_type,
                    &[
                        (
                            "requirement",
                            variant(
                                field_type(&checked.input_type, "requirement"),
                                requirement_tag,
                            ),
                        ),
                        (
                            "observation",
                            variant(
                                field_type(&checked.input_type, "observation"),
                                observation_tag,
                            ),
                        ),
                        (
                            "requirement_value",
                            StructuredInfoValue::leaf(
                                field_type(&checked.input_type, "requirement_value").clone(),
                                requirement_value.to_le_bytes().to_vec(),
                            )
                            .unwrap(),
                        ),
                        (
                            "observation_value",
                            StructuredInfoValue::leaf(
                                field_type(&checked.input_type, "observation_value").clone(),
                                observation_value.to_le_bytes().to_vec(),
                            )
                            .unwrap(),
                        ),
                    ],
                )
                .canonical_bytes()
                .unwrap();
                let result = speech_context_compare(SpeechContextComparisonInput {
                    requirement,
                    observation,
                    requirement_value,
                    observation_value,
                })
                .unwrap();
                let tag = match result {
                    SpeechContextDecision::matched => "matched",
                    SpeechContextDecision::mismatched => "mismatched",
                    SpeechContextDecision::requirement_unresolved => "requirement_unresolved",
                    SpeechContextDecision::observation_unresolved => "observation_unresolved",
                };
                assert_eq!(
                    checked.evaluate(&input).unwrap(),
                    variant(&checked.output_type, tag)
                        .canonical_bytes()
                        .unwrap()
                );
            }
        }
    }
}
