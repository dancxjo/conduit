//! Private independent comparison; product state is never seeded from C state.
use super::{on_large_stack, plan, runtime, shared, state, with_actual_feature_tape, FeatureTape};
#[allow(dead_code)]
#[path = "../fargan_signal_graph/conditioning_support.rs"]
mod conditioning;
#[allow(dead_code)]
#[path = "../fargan_signal_graph/frame_support.rs"]
mod frame;
#[allow(dead_code)]
#[path = "../fargan_signal_graph/pinned.rs"]
mod pinned;
#[allow(dead_code)]
#[path = "../fargan_signal_graph/warm_start_support.rs"]
mod warm_start;
use conduit_core::*;
use conduit_speech::fargan_carrier_epochs::PreparedCarrierEpochs;
use conduit_speech::fargan_endpoint50::prepare_fargan_endpoint50;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
const FRAMING: &str = include_str!("../../../speech/fargan_pcm_framing.conduit");
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn f32s(bytes: &[u8]) -> Vec<f32> {
    assert_eq!(bytes.len() % 4, 0);
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| {
            let n = f32::from_le_bytes(*v);
            assert!(n.is_finite());
            n
        })
        .collect()
}
fn period(bytes: &[u8]) -> u16 {
    let value = u16::try_from(i32::from_le_bytes(bytes.try_into().unwrap())).unwrap();
    assert!((32..=255).contains(&value));
    value
}
fn state_bytes(value: &state::State, history: &[f32; 128], previous: u16) -> Vec<u8> {
    let mut bytes: Vec<_> = value
        .flattened()
        .into_iter()
        .chain(history.iter().copied())
        .flat_map(f32::to_le_bytes)
        .collect();
    bytes.extend_from_slice(&i32::from(previous).to_le_bytes());
    assert_eq!(bytes.len(), 3864);
    bytes
}
fn oracle(tape: &FeatureTape, output: &Path) -> (Vec<u8>, Vec<f32>, serde_json::Value) {
    let binary = PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_ENDPOINT50_ORACLE")
            .expect("explicit built private oracle"),
    );
    let receipt_path = PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_ENDPOINT50_BUILD_RECEIPT")
            .expect("explicit pinned oracle build receipt"),
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    assert!(binary.is_absolute() && receipt_path.is_absolute());
    assert_eq!(receipt["exit_code"], 0);
    let arguments = receipt["command"].as_array().unwrap();
    for flag in [
        "-O2",
        "-std=c99",
        "-ffp-contract=off",
        "-fno-tree-vectorize",
    ] {
        assert!(arguments.iter().any(|argument| argument == flag));
    }
    assert!(!arguments.iter().any(|argument| argument
        .as_str()
        .is_some_and(|value| value.contains("DISABLE_DEBUG_FLOAT"))));
    let harness = arguments
        .iter()
        .filter_map(serde_json::Value::as_str)
        .find(|argument| argument.ends_with("endpoint50_oracle.c"))
        .unwrap();
    assert_eq!(
        receipt["harness_sha256"],
        hash(&std::fs::read(harness).unwrap())
    );
    assert_eq!(
        receipt["executable_sha256"],
        hash(&std::fs::read(&binary).unwrap())
    );
    for (path, digest) in receipt["inputs"].as_object().unwrap() {
        assert_eq!(
            digest,
            &serde_json::json!(hash(&std::fs::read(path).unwrap()))
        );
    }
    let mut input: Vec<_> = tape
        .frames
        .iter()
        .flat_map(|(_, _, f)| f.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    input.extend(
        tape.frames
            .iter()
            .flat_map(|(_, p, _)| u32::from(*p).to_le_bytes()),
    );
    assert_eq!(input.len(), 4200);
    let input_path = output.join("actual50-input.bin");
    let trace_path = output.join("oracle-trace.bin");
    let audible_path = output.join("oracle-audible.f32le");
    std::fs::write(&input_path, &input).unwrap();
    let result = Command::new(&binary)
        .args([&input_path, &trace_path, &audible_path])
        .output()
        .unwrap();
    std::fs::write(output.join("oracle.stdout"), &result.stdout).unwrap();
    std::fs::write(output.join("oracle.stderr"), &result.stderr).unwrap();
    assert!(
        result.status.success(),
        "private oracle failed; partial files are invalid"
    );
    let trace = std::fs::read(trace_path).unwrap();
    let audible = std::fs::read(audible_path).unwrap();
    assert_eq!(trace.len(), 237648);
    assert_eq!(audible.len(), 32000);
    (
        trace,
        f32s(&audible),
        serde_json::json!({"build_receipt_sha256":hash(&std::fs::read(receipt_path).unwrap()),"input_sha256":hash(&input),"trace_sha256":hash(&std::fs::read(output.join("oracle-trace.bin")).unwrap()),"audible_sha256":hash(&audible),"binary":binary}),
    )
}
struct Framing(BTreeMap<&'static str, (plan::SourceSchema, Plan)>);
impl Framing {
    fn prepare() -> Self {
        Self(
            [
                "speech/fargan-audible-frame",
                "speech/fargan-unheard-leading-half",
                "speech/fargan-unheard-trailing-half",
            ]
            .into_iter()
            .map(|entry| {
                let schema = plan::SourceSchema::for_entry(FRAMING, entry);
                let selected = plan::prepare_entry_plan(&schema, FRAMING, entry);
                (entry, (schema, selected))
            })
            .collect(),
        )
    }
    fn run(
        &self,
        entry: &str,
        values: Vec<(&str, StructuredInfoValue)>,
    ) -> runtime::EncodedResultAndTiming {
        let (schema, selected) = &self.0[entry];
        let inputs = values
            .into_iter()
            .map(|(name, value)| {
                assert_eq!(value.value_type(), schema.ty(name));
                (name.into(), value.canonical_bytes().unwrap())
            })
            .collect();
        let result = runtime::run_entry_plan(
            selected.clone(),
            &shared::Resources::new(),
            inputs,
            runtime::ExecutionMode::Normal,
        )
        .unwrap();
        assert!(!result.signs.is_empty());
        result
    }
}
#[test]
#[ignore = "requires explicit pinned private model, independently built endpoint50 C oracle and fresh private output directory"]
fn actual_carrier_endpoint50_neural_uses_own_state_and_all_source_framing_outputs() {
    on_large_stack(|| with_actual_feature_tape(run));
}
fn run(tape: FeatureTape, epochs: &PreparedCarrierEpochs<'_>) {
    let endpoint = prepare_fargan_endpoint50(epochs).unwrap();
    assert_eq!(endpoint.steps().len(), 51);
    assert_eq!(endpoint.cycles(), tape.cycles.as_slice());
    let output = PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_ENDPOINT50_OUTPUT").expect("fresh private output path"),
    );
    assert!(output.is_absolute());
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("complete-carrier.canonical"), &tape.original).unwrap();
    std::fs::write(
        output.join("cycle-gate-input.canonical"),
        endpoint.gate_input_canonical(),
    )
    .unwrap();
    std::fs::write(
        output.join("cycle-gate-output.canonical"),
        endpoint.gate_output_canonical(),
    )
    .unwrap();
    std::fs::write(
        output.join("endpoint-gate.program.hex"),
        endpoint.source_program_hex(),
    )
    .unwrap();
    std::fs::write(
        output.join("endpoint-step.program.hex"),
        endpoint.step_program_hex(),
    )
    .unwrap();
    std::fs::write(
        output.join("original-carrier.pcm8k.i16le"),
        tape.raw_pcm
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for (i, epoch) in epochs.epochs().iter().enumerate() {
        let period = epoch.period().unwrap();
        std::fs::write(
            output.join(format!("period-{i}.input.canonical")),
            period.input_canonical(),
        )
        .unwrap();
        std::fs::write(
            output.join(format!("period-{i}.output.canonical")),
            period.output_canonical(),
        )
        .unwrap();
    }
    let (reference, audible_reference, oracle_receipt) = oracle(&tape, &output);
    let model = PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE")
            .expect("explicit private model resources"),
    );
    assert!(model.is_absolute());
    let warm_source = warm_start::source();
    let warm_schema = plan::SourceSchema::for_entry(&warm_source, "speech/fargan-warm-start");
    let resources = frame::resources(&model, &warm_schema);
    let f0: [f32; 20] = tape.frames[0].2.clone().try_into().unwrap();
    let p0 = tape.frames[0].1;
    let warm = runtime::run_entry_plan(
        plan::prepare_entry_plan(&warm_schema, &warm_source, "speech/fargan-warm-start"),
        &resources,
        warm_start::inputs(&warm_schema, &[0.; 320], &[f0; 5], [p0; 5]),
        runtime::ExecutionMode::Normal,
    )
    .unwrap();
    for (name, value) in &warm.outputs {
        std::fs::write(
            output.join(format!("warm-{name}.canonical")),
            value.canonical_bytes().unwrap(),
        )
        .unwrap();
    }
    std::fs::write(output.join("warm.signs"), format!("{:?}", warm.signs)).unwrap();
    let expected_warm = state::State::from_oracle(&f32s(&reference[..3348]));
    let expected_history: [f32; 128] = f32s(&reference[3348..3860]).try_into().unwrap();
    warm_start::compare(
        &warm,
        &expected_warm,
        &expected_history,
        period(&reference[3860..3864]),
    );
    let mut own_state = state::State::from_state(&warm.outputs["next_state"]);
    let mut history: [f32; 128] = state::floats(&warm.outputs["next_history"])
        .try_into()
        .unwrap();
    let mut previous = warm_start::period(&warm.outputs["previous_period"]);
    let mut actual_trace = state_bytes(&own_state, &history, previous);
    let source = plan::source();
    let schema = frame::schema();
    let resources = frame::resources(&model, &schema);
    let selected = plan::prepare_entry_plan(&schema, &source, frame::ENTRY);
    let framing = Framing::prepare();
    let mut previous_frame: Option<runtime::EncodedResultAndTiming> = None;
    let mut audible = Vec::new();
    let mut unheard = Vec::new();
    let (mut pcm_error, mut state_error, mut history_error) = (0f32, 0f32, 0f32);
    let mut accepted = 0;
    let mut described_audible_end = 0u64;
    for (k, step) in endpoint.steps().iter().enumerate() {
        let d = step.descriptor();
        assert_eq!(*d.raw_index(), k as u64);
        assert_eq!(*d.audible_start(), described_audible_end);
        assert_eq!(*d.keep_start(), if k == 0 { 80 } else { 0 });
        assert_eq!(*d.keep_count(), if k == 0 || k == 50 { 80 } else { 160 });
        described_audible_end += *d.keep_count();
        let feature_index = usize::try_from(*d.feature_source_index()).unwrap();
        let period_index = usize::try_from(*d.period_source_index()).unwrap();
        let features: [f32; 20] = tape.frames[feature_index].2.clone().try_into().unwrap();
        let current = tape.frames[period_index].1;
        assert_eq!(feature_index, if k < 49 { k + 1 } else { 49 });
        assert_eq!(period_index, if k < 50 { k } else { 49 });
        assert_eq!(*d.endpoint_feature_copy(), k >= 49);
        assert_eq!(*d.endpoint_period_copy(), k == 50);
        let inputs = frame::inputs(&schema, &features, &history, current, previous, &own_state);
        if [0, 25, 50].contains(&k) {
            let before = state_bytes(&own_state, &history, previous);
            let frames = accepted;
            let audible_before = audible.clone();
            for mode in [
                runtime::ExecutionMode::StoragePressure,
                runtime::ExecutionMode::CancelFirstExpression,
            ] {
                assert!(runtime::run_entry_plan(
                    selected.clone(),
                    &resources,
                    inputs.clone(),
                    mode
                )
                .is_none());
                assert_eq!(state_bytes(&own_state, &history, previous), before);
                assert_eq!(accepted, frames);
                assert_eq!(audible, audible_before);
            }
        }
        let result = runtime::run_entry_plan(
            selected.clone(),
            &resources,
            inputs,
            runtime::ExecutionMode::Normal,
        )
        .unwrap();
        assert!(!result.signs.is_empty());
        for (name, value) in &result.outputs {
            std::fs::write(
                output.join(format!("frame-{k}-{name}.canonical")),
                value.canonical_bytes().unwrap(),
            )
            .unwrap();
        }
        std::fs::write(
            output.join(format!("frame-{k}.signs")),
            format!("{:?}", result.signs),
        )
        .unwrap();
        let record = &reference[3864 + k * 4584..3864 + (k + 1) * 4584];
        assert_eq!(
            features
                .iter()
                .copied()
                .map(f32::to_bits)
                .collect::<Vec<_>>(),
            f32s(&record[..80])
                .into_iter()
                .map(f32::to_bits)
                .collect::<Vec<_>>()
        );
        let pcm = frame::pcm(&result);
        for (a, b) in pcm.iter().zip(f32s(&record[80..720])) {
            pcm_error = pcm_error.max((a - b).abs());
        }
        let next = state::State::from_result(&result.value);
        for (a, b) in next.flattened().iter().zip(f32s(&record[720..4068])) {
            state_error = state_error.max((a - b).abs());
        }
        let next_history: [f32; 128] = state::floats(&result.outputs["next_history"])
            .try_into()
            .unwrap();
        for (a, b) in next_history.iter().zip(f32s(&record[4068..4580])) {
            history_error = history_error.max((a - b).abs());
        }
        let actual_period = warm_start::period(&result.outputs["next_period"]);
        assert_eq!(actual_period, period(&record[4580..4584]));
        if let Some(prior) = &previous_frame {
            let packet = framing.run(
                "speech/fargan-audible-frame",
                vec![
                    ("previous_pcm2", prior.outputs["pcm2"].clone()),
                    (
                        "previous_pcm3",
                        state::field(&prior.value, "pcm_f32").clone(),
                    ),
                    ("current_pcm0", result.outputs["pcm0"].clone()),
                    ("current_pcm1", result.outputs["pcm1"].clone()),
                ],
            );
            let samples = state::floats(&packet.value);
            assert_eq!(samples.len(), 160);
            audible.extend_from_slice(&samples);
            std::fs::write(
                output.join(format!("audible-{}.canonical", k - 1)),
                packet.value.canonical_bytes().unwrap(),
            )
            .unwrap();
        } else {
            let leading = framing.run(
                "speech/fargan-unheard-leading-half",
                vec![
                    ("pcm0", result.outputs["pcm0"].clone()),
                    ("pcm1", result.outputs["pcm1"].clone()),
                ],
            );
            unheard.extend(state::floats(&leading.value));
            std::fs::write(
                output.join("unheard-leading.canonical"),
                leading.value.canonical_bytes().unwrap(),
            )
            .unwrap();
        }
        std::fs::write(
            output.join(format!("step-{k}.input.canonical")),
            step.input_canonical(),
        )
        .unwrap();
        std::fs::write(
            output.join(format!("step-{k}.output.canonical")),
            step.output_canonical(),
        )
        .unwrap();
        actual_trace.extend(features.iter().flat_map(|v| v.to_le_bytes()));
        actual_trace.extend(pcm.iter().flat_map(|v| v.to_le_bytes()));
        actual_trace.extend(state_bytes(&next, &next_history, actual_period));
        own_state = next;
        history = next_history;
        previous = actual_period;
        previous_frame = Some(result);
        accepted += 1;
    }
    let last = previous_frame.unwrap();
    let trailing = framing.run(
        "speech/fargan-unheard-trailing-half",
        vec![
            ("pcm2", last.outputs["pcm2"].clone()),
            ("pcm3", state::field(&last.value, "pcm_f32").clone()),
        ],
    );
    unheard.extend(state::floats(&trailing.value));
    std::fs::write(
        output.join("unheard-trailing.canonical"),
        trailing.value.canonical_bytes().unwrap(),
    )
    .unwrap();
    assert_eq!(accepted, 51);
    assert_eq!(described_audible_end, 8000);
    assert_eq!(audible.len(), 8000);
    assert_eq!(unheard.len(), 160);
    for (a, b) in audible.iter().zip(&audible_reference) {
        assert!((a - b).abs() < 0.002);
    }
    let expected_unheard: Vec<_> = f32s(&reference[3864 + 80..3864 + 80 + 320])
        .into_iter()
        .chain(f32s(
            &reference[3864 + 50 * 4584 + 80 + 320..3864 + 50 * 4584 + 720],
        ))
        .collect();
    for (a, b) in unheard.iter().zip(expected_unheard) {
        assert!((a - b).abs() < 0.002);
    }
    assert!(
        pcm_error < 0.002 && state_error < 0.05 && history_error < 0.001,
        "PCM={pcm_error} state={state_error} history={history_error}"
    );
    assert_eq!(actual_trace.len(), 237648);
    std::fs::write(output.join("actual-trace.bin"), &actual_trace).unwrap();
    let audible_bytes: Vec<_> = audible.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(output.join("actual-audible.f32le"), &audible_bytes).unwrap();
    for (i, (input, period_input, output_bytes, signs)) in tape.frontend.iter().enumerate() {
        assert!(!signs.is_empty());
        std::fs::write(output.join(format!("frontend-{i}.input.canonical")), input).unwrap();
        std::fs::write(
            output.join(format!("frontend-{i}.period.canonical")),
            period_input,
        )
        .unwrap();
        std::fs::write(
            output.join(format!("frontend-{i}.output.canonical")),
            output_bytes,
        )
        .unwrap();
    }
    std::fs::write(output.join("receipt.json"),serde_json::to_vec_pretty(&serde_json::json!({"oracle":oracle_receipt,"carrier_sha256":hash(&tape.original),"frontend_source_sha256":tape.frontend_source_sha,"frontend_plan":tape.frontend_plan,"endpoint_source_document":endpoint.source_document_id(),"warm_source_sha256":hash(warm_source.as_bytes()),"frame_source_sha256":hash(source.as_bytes()),"framing_source_sha256":hash(FRAMING.as_bytes()),"model_f32_sha256":hash(&std::fs::read(model.join("resources/f32.bin")).unwrap()),"actual_trace_sha256":hash(&actual_trace),"audible_sha256":hash(&audible_bytes),"pcm_max_abs":pcm_error,"state_max_abs":state_error,"history_max_abs":history_error,"accepted_raw_frames":accepted,"audible_samples":audible.len(),"unheard_samples":unheard.len(),"scope":"same actual carrier/Source features plus own-state numerical endpoint differential; fixture-only staging, not installed atomic controller, quality or physical playback"})).unwrap()).unwrap();
}
