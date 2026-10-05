//! All 48 layer-presence/specification carriers match portable native meaning.
use crate::{
    differential::program,
    frame_parity::{field_type, record},
    generated::*,
    onset_parity::variant,
};
use conduit_core::StructuredInfoValue;
use std::vec;
#[test]
fn every_output_feature_layer_carrier_matches_portable_native_meaning() {
    let checked = program("speech_output_feature_layer");
    let states = [
        ("known", SpeechSpecificationState::known),
        ("unknown", SpeechSpecificationState::unknown),
        ("unspecified", SpeechSpecificationState::unspecified),
        ("not_applicable", SpeechSpecificationState::not_applicable),
        ("variable", SpeechSpecificationState::variable),
        ("gradient", SpeechSpecificationState::gradient),
    ];
    for (tag, phone) in states {
        for mask in 0..8 {
            let default_present = mask & 1 != 0;
            let definition_present = mask & 2 != 0;
            let output_present = mask & 4 != 0;
            let boolean = |name: &str, value: bool| {
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
                        variant(field_type(&checked.input_type, "phone"), tag),
                    ),
                    (
                        "default_present",
                        boolean("default_present", default_present),
                    ),
                    (
                        "definition_present",
                        boolean("definition_present", definition_present),
                    ),
                    ("output_present", boolean("output_present", output_present)),
                ],
            )
            .canonical_bytes()
            .unwrap();
            let result = speech_output_feature_layer(SpeechOutputFeatureLayerInput {
                phone,
                default_present,
                definition_present,
                output_present,
            })
            .unwrap();
            let output_tag = match result {
                SpeechOutputFeatureLayer::absent => "absent",
                SpeechOutputFeatureLayer::default_realization => "default_realization",
                SpeechOutputFeatureLayer::phone_definition => "phone_definition",
                SpeechOutputFeatureLayer::rule_output => "rule_output",
            };
            assert_eq!(
                checked.evaluate(&input).unwrap(),
                variant(&checked.output_type, output_tag)
                    .canonical_bytes()
                    .unwrap()
            );
        }
    }
}

#[test]
fn every_default_feature_presence_pair_matches_portable_native_meaning() {
    let checked = program("speech_default_feature_layer");
    for mask in 0..4 {
        let default_present = mask & 1 != 0;
        let definition_present = mask & 2 != 0;
        let boolean = |name: &str, value: bool| {
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
                    "default_present",
                    boolean("default_present", default_present),
                ),
                (
                    "definition_present",
                    boolean("definition_present", definition_present),
                ),
            ],
        );
        let result = speech_default_feature_layer(SpeechDefaultFeatureLayerInput {
            default_present,
            definition_present,
        })
        .unwrap();
        let tag = match result {
            SpeechOutputFeatureLayer::default_realization => "default_realization",
            SpeechOutputFeatureLayer::phone_definition => "phone_definition",
            SpeechOutputFeatureLayer::absent => "absent",
            _ => panic!("default inheritance cannot invent a rule output"),
        };
        assert_eq!(
            checked.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
            variant(&checked.output_type, tag)
                .canonical_bytes()
                .unwrap()
        );
    }
}
