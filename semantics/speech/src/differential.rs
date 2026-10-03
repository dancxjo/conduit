//! Exact arithmetic and selected-branch parity with the portable evaluator.
use crate::generated::*;
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_plot::PortableExpressionProgram;
use std::{vec, vec::Vec};
pub(super) fn program(name: &str) -> PortableExpressionProgram {
    PortableExpressionProgram::from_canonical_hex(
        PROGRAMS
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .unwrap()
            .1,
    )
    .unwrap()
}
fn record(program: &PortableExpressionProgram, values: &[(&str, i32)]) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|field| {
                let value = values
                    .iter()
                    .find(|(name, _)| *name == field.name())
                    .unwrap()
                    .1;
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::leaf(
                        field.value_type().clone(),
                        value.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn compiled_integer_operations_match_checked_overflow_and_boundary_behavior() {
    for value in [i32::MIN, -65536, -1, 0, 1, 65535, i32::MAX] {
        let noise = program("speech_noise");
        assert_eq!(
            noise
                .evaluate(&value.to_le_bytes())
                .ok()
                .map(|out| i32::from_le_bytes(out.try_into().unwrap())),
            speech_noise(value)
        );
        let limit = program("speech_limit");
        assert_eq!(
            limit
                .evaluate(&value.to_le_bytes())
                .ok()
                .map(|out| i32::from_le_bytes(out.try_into().unwrap())),
            speech_limit(value)
        );
    }
    let resonance = program("speech_resonator");
    for (drive, b, c, y1, y2) in [
        (1000, 16300, -12000, 500, -200),
        (i32::MAX, 1, 1, 0, 0),
        (0, i32::MAX, 0, 2, 0),
        (0, 0, i32::MIN, 0, -1),
    ] {
        let bytes = record(
            &resonance,
            &[("drive", drive), ("b", b), ("c", c), ("y1", y1), ("y2", y2)],
        );
        assert_eq!(
            resonance
                .evaluate(&bytes)
                .ok()
                .map(|out| i32::from_le_bytes(out.try_into().unwrap())),
            speech_resonator(ResonatorInput {
                drive,
                b,
                c,
                y1,
                y2
            })
        );
    }
}
#[test]
fn all_compiled_phone_targets_match_the_portable_typed_program() {
    let (programs, result_index) = crate::prosody_parity::graph("speech_voice_target");
    let input_type = &programs[0].1.input_type;
    let StructuredInfoTypeShape::Variant { cases, .. } = input_type.shape() else {
        panic!("phone")
    };
    // The native enums are generated in exact checked case order. Derive each
    // concrete case by matching the generated evaluator's semantic discriminator.
    for (phone, tag) in PHONES {
        let case = cases.iter().find(|case| case.tag() == *tag).unwrap();
        let payload = StructuredInfoValue::leaf(case.payload_type().clone(), vec![]).unwrap();
        let value = StructuredInfoValue::variant(input_type.clone(), case.tag(), payload).unwrap();
        let output = StructuredInfoValue::from_canonical_bytes(
            &crate::prosody_parity::evaluate(
                &programs,
                result_index,
                &value.canonical_bytes().unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let conduit_core::StructuredInfoValueShape::Record(fields) = output.shape() else {
            panic!("target record")
        };
        let actual = target_fields(speech_voice_target(*phone).unwrap());
        for (name, number) in actual {
            let value = fields
                .iter()
                .find(|field| field.name() == name)
                .unwrap()
                .value();
            let conduit_core::StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                panic!("integer")
            };
            assert_eq!(
                i32::from_le_bytes(bytes.try_into().unwrap()),
                number,
                "{tag}/{name}"
            );
        }
    }
}
