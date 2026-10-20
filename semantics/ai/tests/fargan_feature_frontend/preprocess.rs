//! Executes authored preprocessing; Rust only encodes inputs and carries outputs.
use super::{plan, runtime, shared, state};
use conduit_ai::fixed_numeric_catalog::*;
use conduit_core::*;
use conduit_plot::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
pub struct History {
    pub samples: Vec<f32>,
    pub epoch_histories: Vec<Vec<f32>>,
    pub programs: Vec<String>,
    pub executions: usize,
    pub receipts: Vec<(String, String)>,
    pub admission_receipts: Vec<(String, String)>,
}
fn program(source: &str, entry: &str) -> PortableExpressionProgram {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &profiles)
        .unwrap_or_else(|error| panic!("expanding {entry}: {error:?}"));
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("pure authored program")
    };
    PortableExpressionProgram::from_canonical_hex(hex).unwrap()
}
fn record(ty: &StructuredInfoType, fields: &BTreeMap<&str, Vec<f32>>) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), record(representation, fields)).unwrap()
        }
        StructuredInfoTypeShape::Record {
            fields: members, ..
        } => StructuredInfoValue::record(
            ty.clone(),
            members
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        state::vector(field.value_type(), &fields[field.name()]),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        _ => panic!("authored preprocessing record"),
    }
}
fn execute(
    program: &PortableExpressionProgram,
    fields: BTreeMap<&str, Vec<f32>>,
) -> (StructuredInfoValue, (String, String)) {
    let input = record(&program.input_type, &fields)
        .canonical_bytes()
        .unwrap();
    let mut evaluator = PreparedPortableExpressionEvaluator::new(program).unwrap();
    let output = evaluator.evaluate(&input).unwrap();
    assert_eq!(program.evaluate(&input).unwrap(), output);
    (
        StructuredInfoValue::from_canonical_bytes(output).unwrap(),
        (
            format!("{:x}", Sha256::digest(&input)),
            format!("{:x}", Sha256::digest(output)),
        ),
    )
}
pub fn prepare(source: &str, raw_pcm: &[i16]) -> History {
    assert!(!raw_pcm.is_empty() && raw_pcm.len().is_multiple_of(80));
    let resample = program(source, "speech/fargan-formant-linear2x");
    let emphasis = program(source, "speech/fargan-feature-preemphasis");
    let append = program(source, "speech/fargan-feature-wave-append");
    let schema160 = plan::SourceSchema::for_entry(source, "speech-test/fargan-admit160");
    let plan160 = plan::prepare_entry_plan(&schema160, source, "speech-test/fargan-admit160");
    let mut admission_receipts = Vec::new();
    let mut admit = |value: &StructuredInfoValue, width: usize| {
        assert_eq!(width, 160);
        let (schema, prepared) = (&schema160, &plan160);
        assert_eq!(value.value_type(), schema.ty("value"));
        let input = value.canonical_bytes().unwrap();
        let result = runtime::run_entry_plan(
            prepared.clone(),
            &shared::Resources::new(),
            BTreeMap::from([("value".into(), input.clone())]),
            runtime::ExecutionMode::Normal,
        )
        .unwrap();
        assert_eq!(result.value.value_type(), schema.ty("result"));
        assert!(!result.signs.is_empty());
        admission_receipts.push((
            format!("{:x}", Sha256::digest(&input)),
            format!(
                "{:x}",
                Sha256::digest(result.value.canonical_bytes().unwrap())
            ),
        ));
        result.value
    };
    let mut previous_raw = vec![0.];
    let mut previous_normalized = vec![0.];
    let mut history = vec![0.; 640];
    let mut receipts = Vec::new();
    let mut epoch_histories = Vec::new();
    for epoch in raw_pcm.as_chunks::<80>().0 {
        // Exact I16->F32 fixture encoding only; scaling belongs to executed Source.
        let (result, receipt) = execute(
            &resample,
            BTreeMap::from([
                (
                    "samples_raw",
                    epoch.iter().copied().map(f32::from).collect(),
                ),
                ("previous_raw", previous_raw),
            ]),
        );
        receipts.push(receipt);
        previous_raw = state::floats(state::field(&result, "next_previous_raw"));
        let (result, receipt) = execute(
            &emphasis,
            BTreeMap::from([
                ("samples", state::floats(state::field(&result, "samples"))),
                ("previous_normalized", previous_normalized),
            ]),
        );
        receipts.push(receipt);
        previous_normalized = state::floats(state::field(&result, "next_previous_normalized"));
        let samples = admit(state::field(&result, "samples"), 160);
        let (result, receipt) = execute(
            &append,
            BTreeMap::from([("history", history), ("samples", state::floats(&samples))]),
        );
        receipts.push(receipt);
        history = state::floats(&result);
        epoch_histories.push(history.clone());
    }
    // Program bytes are receipts for executed arithmetic, not physical playback.
    History {
        samples: history,
        epoch_histories,
        programs: [&resample, &emphasis, &append]
            .iter()
            .map(|p| p.canonical_hex().unwrap())
            .collect(),
        executions: receipts.len(),
        receipts,
        admission_receipts,
    }
}
