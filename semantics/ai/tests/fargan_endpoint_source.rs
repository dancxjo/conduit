#![cfg(feature = "kernel-operation-owners")]
//! Finite authored endpoint accounting and ordinary Value PCM port composition.
#[allow(dead_code)]
#[path = "fargan_signal_graph/plan.rs"]
mod plan;
#[allow(dead_code)]
#[path = "fargan_signal_graph/runtime.rs"]
mod runtime;
#[allow(dead_code)]
#[path = "fargan_signal_graph/shared.rs"]
mod shared;
#[allow(dead_code)]
#[path = "fargan_signal_graph/state.rs"]
mod state;
use conduit_core::*;
use conduit_plot::{
    rust_binding::{validate_native_contracts, validate_native_invariants, NativeRustBinding},
    *,
};
use std::collections::BTreeMap;
const POLICY: &str = include_str!("../../speech/fargan_endpoint_policy.conduit");
const FRAMING: &str = include_str!("../../speech/fargan_pcm_framing.conduit");
fn checked_policy() -> CheckedSyntaxDocument {
    let mut startup = StartupCatalog::new();
    let types = conduit_speech::authoring::checked_types().unwrap();
    let cycle = types
        .iter()
        .find(|t| t.name == "SpeechFundamentalCycle")
        .unwrap();
    startup
        .insert_checked_native_type("SpeechFundamentalCycle", cycle)
        .unwrap();
    check_syntax_document(&parse_syntax_document(POLICY), &startup).unwrap()
}
fn program(checked: &CheckedSyntaxDocument, entry: &str) -> PortableExpressionProgram {
    let expanded =
        expand_canonical_plot_for_authoring(checked, entry, &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("one Source program")
    };
    PortableExpressionProgram::from_canonical_hex(hex).unwrap()
}
fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn number(value: &StructuredInfoValue, name: &str) -> u64 {
    let StructuredInfoValueShape::Leaf(bytes) = state::field(value, name).shape() else {
        panic!("number")
    };
    u64::from_le_bytes(bytes.try_into().unwrap())
}
#[test]
fn endpoint_source_accounts_for_all_fifty_actual_epochs_and_explicit_copies() {
    let checked = checked_policy();
    let program = program(&checked, "speech/fargan-endpoint-step50");
    let native = checked
        .native_types
        .iter()
        .find(|t| t.value_type == program.input_type)
        .unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let mut end = 0;
    for index in 0u64..=51 {
        let input = StructuredInfoValue::record(
            program.input_type.clone(),
            vec![StructuredFieldValue::new(
                "raw_index",
                StructuredInfoValue::leaf(
                    field_type(&program.input_type, "raw_index").clone(),
                    index.to_le_bytes().to_vec(),
                )
                .unwrap(),
            )
            .unwrap()],
        )
        .unwrap();
        let admission = validate_native_contracts(&input, &native.value_contracts)
            .and_then(|()| validate_native_invariants(&input, &native.invariants));
        if index == 51 {
            assert!(admission.is_err());
            continue;
        }
        admission.unwrap();
        let encoded = input.canonical_bytes().unwrap();
        let bytes = evaluator.evaluate(&encoded).unwrap();
        assert_eq!(bytes, program.evaluate(&encoded).unwrap());
        let output = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
        assert_eq!(number(&output, "raw_index"), index);
        for (name, expected) in [
            ("endpoint_feature_copy", index >= 49),
            ("endpoint_period_copy", index == 50),
        ] {
            let StructuredInfoValueShape::Leaf(bytes) = state::field(&output, name).shape() else {
                panic!("Boolean")
            };
            assert_eq!(bytes, if expected { &[1][..] } else { &[0][..] });
        }
        assert_eq!(number(&output, "audible_start"), end);
        let feature = number(&output, "feature_source_index");
        let period = number(&output, "period_source_index");
        assert!(feature < 50 && period < 50);
        assert_eq!(feature, if index < 49 { index + 1 } else { 49 });
        assert_eq!(period, if index < 50 { index } else { 49 });
        assert_eq!(
            number(&output, "keep_start"),
            if index == 0 { 80 } else { 0 }
        );
        assert_eq!(
            number(&output, "keep_count"),
            if index == 0 || index == 50 { 80 } else { 160 }
        );
        end += number(&output, "keep_count");
    }
    assert_eq!(end, 8000);
}
fn cycle_tree(
    ty: &StructuredInfoType,
    values: &[conduit_speech::semantic::SpeechFundamentalCycle],
) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), cycle_tree(representation, values)).unwrap()
        }
        StructuredInfoTypeShape::Collection { element, length } => {
            assert_eq!(length as usize, values.len());
            StructuredInfoValue::collection(
                ty.clone(),
                values
                    .iter()
                    .map(|v| {
                        let value =
                            StructuredInfoValue::from_canonical_bytes(&v.clone().encode().unwrap())
                                .unwrap();
                        assert_eq!(value.value_type(), element);
                        value
                    })
                    .collect(),
            )
            .unwrap()
        }
        _ => panic!("fixed cycle collection"),
    }
}
#[test]
fn constant_cycle_gate_executes_all_fifty_original_ratios_without_normalizing() {
    let checked = checked_policy();
    let program = program(&checked, "speech/fargan-endpoint-constant-cycles50");
    let cycle = conduit_speech::semantic::SpeechFundamentalCycle::new(120, 1).unwrap();
    let mut represented = &program.input_type;
    while let StructuredInfoTypeShape::Nominal { representation, .. } = represented.shape() {
        represented = representation;
    }
    for wrong_count in [49, 51] {
        let item =
            StructuredInfoValue::from_canonical_bytes(&cycle.clone().encode().unwrap()).unwrap();
        assert!(
            StructuredInfoValue::collection(represented.clone(), vec![item; wrong_count]).is_err()
        );
    }
    for mismatch in [None, Some(1usize), Some(49)] {
        let mut cycles = vec![cycle.clone(); 50];
        if let Some(i) = mismatch {
            cycles[i] = conduit_speech::semantic::SpeechFundamentalCycle::new(240, 2).unwrap();
        }
        let encoded = cycle_tree(&program.input_type, &cycles)
            .canonical_bytes()
            .unwrap();
        let expected = if mismatch.is_some() { vec![0] } else { vec![1] };
        assert_eq!(program.evaluate(&encoded).unwrap(), expected);
        assert_eq!(
            PreparedPortableExpressionEvaluator::new(&program)
                .unwrap()
                .evaluate(&encoded)
                .unwrap(),
            expected
        );
    }
}
#[test]
fn ordinary_source_pcm_framing_copies_previous_tail_and_current_head_exactly() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let schema = plan::SourceSchema::for_entry(FRAMING, "speech/fargan-audible-frame");
            let mut inputs = BTreeMap::new();
            let mut expected = Vec::new();
            for (channel, name) in [
                "previous_pcm2",
                "previous_pcm3",
                "current_pcm0",
                "current_pcm1",
            ]
            .into_iter()
            .enumerate()
            {
                let samples: Vec<_> = (0..40)
                    .map(|i| {
                        if channel == 0 && i == 0 {
                            -0.0
                        } else {
                            (channel * 40 + i) as f32 - 80.
                        }
                    })
                    .collect();
                expected.extend_from_slice(&samples);
                inputs.insert(
                    name.into(),
                    state::vector(schema.ty(name), &samples)
                        .canonical_bytes()
                        .unwrap(),
                );
            }
            let prepared =
                plan::prepare_entry_plan(&schema, FRAMING, "speech/fargan-audible-frame");
            let output = runtime::run_entry_plan(
                prepared.clone(),
                &shared::Resources::new(),
                inputs.clone(),
                runtime::ExecutionMode::Normal,
            )
            .unwrap();
            let values = state::floats(&output.value);
            assert_eq!(values.len(), 160);
            assert_eq!(
                values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
            assert!(!output.signs.is_empty());
            assert!(runtime::run_entry_plan(
                prepared,
                &shared::Resources::new(),
                inputs,
                runtime::ExecutionMode::StoragePressure
            )
            .is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn source_retains_both_unheard_endpoint_halves_as_real_evidence_outputs() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            for (entry, names) in [
                ("speech/fargan-unheard-leading-half", ["pcm0", "pcm1"]),
                ("speech/fargan-unheard-trailing-half", ["pcm2", "pcm3"]),
            ] {
                let schema = plan::SourceSchema::for_entry(FRAMING, entry);
                let mut inputs = BTreeMap::new();
                let mut expected = Vec::new();
                for (channel, name) in names.into_iter().enumerate() {
                    let values: Vec<_> =
                        (0..40).map(|i| (channel * 40 + i) as f32 + 0.25).collect();
                    expected.extend_from_slice(&values);
                    inputs.insert(
                        name.into(),
                        state::vector(schema.ty(name), &values)
                            .canonical_bytes()
                            .unwrap(),
                    );
                }
                let prepared = plan::prepare_entry_plan(&schema, FRAMING, entry);
                let result = runtime::run_entry_plan(
                    prepared,
                    &shared::Resources::new(),
                    inputs,
                    runtime::ExecutionMode::Normal,
                )
                .unwrap();
                assert_eq!(state::floats(&result.value), expected);
                assert!(!result.signs.is_empty());
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
