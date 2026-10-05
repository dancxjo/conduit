//! Exhaustive carrier parity for the native aspiration realization Plot.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::{StructuredInfoTypeShape, StructuredInfoValue};
use std::vec;
#[test]
fn every_aspiration_carrier_matches_the_portable_plot() {
    let checked = program("speech_aspiration_realize");
    let states = [
        ("known", SpeechSpecificationState::known),
        ("unknown", SpeechSpecificationState::unknown),
        ("unspecified", SpeechSpecificationState::unspecified),
        ("not_applicable", SpeechSpecificationState::not_applicable),
        ("variable", SpeechSpecificationState::variable),
        ("gradient", SpeechSpecificationState::gradient),
    ];
    for &(phone, phone_tag) in PHONES {
        for &(state_tag, state) in &states {
            for boolean_kind in [false, true] {
                for boolean_value in [false, true] {
                    let boolean = |name, value| {
                        StructuredInfoValue::leaf(
                            field_type(&checked.input_type, name).clone(),
                            vec![u8::from(value)],
                        )
                        .unwrap()
                    };
                    let input = record(
                        &checked.input_type,
                        &[
                            (
                                "phone",
                                variant(field_type(&checked.input_type, "phone"), phone_tag),
                            ),
                            (
                                "state",
                                variant(field_type(&checked.input_type, "state"), state_tag),
                            ),
                            ("boolean_kind", boolean("boolean_kind", boolean_kind)),
                            ("boolean_value", boolean("boolean_value", boolean_value)),
                        ],
                    );
                    let result = speech_aspiration_realize(SpeechAspirationInput {
                        phone,
                        state,
                        boolean_kind,
                        boolean_value,
                    })
                    .unwrap();
                    let expected = match result {
                        SpeechAspirationResult::realized(realized) => {
                            let StructuredInfoTypeShape::Variant { cases, .. } =
                                checked.output_type.shape()
                            else {
                                panic!("variant")
                            };
                            let payload_type = cases
                                .iter()
                                .find(|case| case.tag() == "realized")
                                .unwrap()
                                .payload_type();
                            let tag = PHONES
                                .iter()
                                .find(|(phone, _)| *phone == realized)
                                .unwrap()
                                .1;
                            StructuredInfoValue::variant(
                                checked.output_type.clone(),
                                "realized",
                                variant(payload_type, tag),
                            )
                            .unwrap()
                        }
                        SpeechAspirationResult::unresolved => {
                            variant(&checked.output_type, "unresolved")
                        }
                        SpeechAspirationResult::unsupported_value => {
                            variant(&checked.output_type, "unsupported_value")
                        }
                        SpeechAspirationResult::unsupported_phone => {
                            variant(&checked.output_type, "unsupported_phone")
                        }
                    };
                    assert_eq!(
                        checked.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
                        expected.canonical_bytes().unwrap()
                    );
                }
            }
        }
    }
}
