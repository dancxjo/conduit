//! Independent portable evaluation of every specification carrier combination.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::StructuredInfoValue;
use std::vec;
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
            for known_values_equal in [false, true] {
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
                            "known_values_equal",
                            StructuredInfoValue::leaf(
                                field_type(&checked.input_type, "known_values_equal").clone(),
                                vec![u8::from(known_values_equal)],
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
                    known_values_equal,
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
