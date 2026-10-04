use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::StructuredInfoValue;
#[test]
fn every_identity_pattern_carrier_agrees_with_checked_portable_policy() {
    let checked = program("speech_identity_pattern_compare");
    let states = [
        ("known", SpeechSpecificationState::known),
        ("unknown", SpeechSpecificationState::unknown),
        ("unspecified", SpeechSpecificationState::unspecified),
        ("not_applicable", SpeechSpecificationState::not_applicable),
        ("variable", SpeechSpecificationState::variable),
        ("gradient", SpeechSpecificationState::gradient),
    ];
    let mut count = 0;
    for (requirement_tag, requirement) in states {
        for (observation_tag, observation) in states {
            for identical in [false, true] {
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
                            "identical",
                            StructuredInfoValue::leaf(
                                field_type(&checked.input_type, "identical").clone(),
                                std::vec![u8::from(identical)],
                            )
                            .unwrap(),
                        ),
                    ],
                )
                .canonical_bytes()
                .unwrap();
                let actual =
                    speech_identity_pattern_compare(SpeechIdentityPatternComparisonInput {
                        requirement,
                        observation,
                        identical,
                    })
                    .unwrap();
                let tag = match actual {
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
                count += 1;
            }
        }
    }
    assert_eq!(count, 72);
}
