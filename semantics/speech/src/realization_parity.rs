//! Every compact phoneme/stress/position realization agrees with the checked
//! portable program, including its preserved input and derivation.
use crate::{differential::program, generated::*};
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use std::{vec, vec::Vec};

fn unit(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let case = cases.iter().find(|case| case.tag() == tag).unwrap();
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap(),
    )
    .unwrap()
}
fn tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("variant")
    };
    tag
}
#[test]
fn every_realization_preserves_typed_input_phone_and_derivation() {
    let program = program("speech_realize");
    let (model_programs, model_result) = crate::prosody_parity::graph("speech_segment_model");
    assert_eq!(model_programs.len(), 4);
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("input")
    };
    for (phoneme, phoneme_tag) in PHONEMES {
        for (stress, stress_tag) in STRESSES {
            for (position, position_tag) in POSITIONS {
                let input = StructuredInfoValue::record(
                    program.input_type.clone(),
                    fields
                        .iter()
                        .map(|field| {
                            let tag = match field.name() {
                                "phoneme" => phoneme_tag,
                                "stress" => stress_tag,
                                "position" => position_tag,
                                _ => panic!("field"),
                            };
                            StructuredFieldValue::new(field.name(), unit(field.value_type(), tag))
                                .unwrap()
                        })
                        .collect::<Vec<_>>(),
                )
                .unwrap();
                let output = StructuredInfoValue::from_canonical_bytes(
                    &program.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
                )
                .unwrap();
                let actual = speech_realize(RealizationInput {
                    phoneme: *phoneme,
                    stress: *stress,
                    position: *position,
                })
                .unwrap();
                assert_eq!(
                    actual.input,
                    RealizationInput {
                        phoneme: *phoneme,
                        stress: *stress,
                        position: *position
                    }
                );
                let model = speech_segment_model(actual.input).unwrap();
                assert_eq!(model.realization, actual);
                assert_eq!(model.target, speech_voice_target(actual.phone).unwrap());
                let portable_model = StructuredInfoValue::from_canonical_bytes(
                    &crate::prosody_parity::evaluate(
                        &model_programs,
                        model_result,
                        &input.canonical_bytes().unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap();
                let StructuredInfoValueShape::Record(model_fields) = portable_model.shape() else {
                    panic!("segment model")
                };
                let model_field = |name| {
                    model_fields
                        .iter()
                        .find(|field| field.name() == name)
                        .unwrap()
                        .value()
                };
                assert_eq!(model_field("realization"), &output);
                assert_eq!(
                    model_field("target"),
                    &crate::frame_parity::integers(
                        crate::frame_parity::field_type(
                            &model_programs[model_result].1.output_type,
                            "target"
                        ),
                        &target_fields(model.target),
                    )
                );
                let StructuredInfoValueShape::Record(fields) = output.shape() else {
                    panic!("output")
                };
                let field = |name| {
                    fields
                        .iter()
                        .find(|field| field.name() == name)
                        .unwrap()
                        .value()
                };
                assert_eq!(field("input"), &input);
                assert_eq!(
                    tag(field("phone")),
                    PHONES
                        .iter()
                        .find(|(phone, _)| *phone == actual.phone)
                        .unwrap()
                        .1
                );
                assert_eq!(
                    tag(field("derivation")),
                    DERIVATIONS
                        .iter()
                        .find(|(derivation, _)| *derivation == actual.derivation)
                        .unwrap()
                        .1
                );
            }
        }
    }
}
