//! Full conditioner plus four recurrent subframes through one ordinary Plan.
use super::{conditioning, pinned, plan::*, runtime::*, shared::*, state::*};
use std::{collections::BTreeMap, path::Path};
pub const ENTRY: &str = "speech/fargan-frame";
pub fn schema() -> SourceSchema {
    SourceSchema::for_entry(&source(), ENTRY)
}
pub fn resources(root: &Path, schema: &SourceSchema) -> Resources {
    let signal = SourceSchema::prepare();
    let condition = SourceSchema::for_entry(&source(), "speech/fargan-conditioning");
    let resources: Resources = pinned::load(root, &signal)
        .into_iter()
        .map(|(n, r)| (format!("signal_{n}"), r))
        .chain(
            pinned::conditioning_resources(root, &condition)
                .into_iter()
                .map(|(n, r)| (format!("cond_{n}"), r)),
        )
        .collect();
    for (name, resource) in &resources {
        assert_eq!(&resource.value_type, schema.ty(name));
    }
    assert_eq!(resources.len(), schema.resource_shapes.len());
    resources
}
pub fn inputs(
    schema: &SourceSchema,
    features: &[f32; 20],
    history: &[f32; 128],
    period: u16,
    previous: u16,
    state: &State,
) -> BTreeMap<String, Vec<u8>> {
    let mut inputs = conditioning::input(schema, features, history, period, previous);
    inputs.insert("state".into(), state.encode(schema.ty("state")));
    inputs
}
pub fn pcm(result: &EncodedResultAndTiming) -> Vec<f32> {
    let mut pcm = Vec::with_capacity(160);
    for name in ["pcm0", "pcm1", "pcm2"] {
        pcm.extend(floats(&result.outputs[name]));
    }
    pcm.extend(floats(field(&result.value, "pcm_f32")));
    assert_eq!(pcm.len(), 160);
    pcm
}
pub struct Case {
    pub features: [f32; 20],
    pub pcm: [f32; 160],
    pub next: State,
    pub history: [f32; 128],
    pub period: u16,
}
pub struct Oracle {
    pub state: State,
    pub history: [f32; 128],
    pub period: u16,
    pub cases: Vec<Case>,
}
fn f32s(bytes: &[u8]) -> Vec<f32> {
    let v: Vec<_> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect();
    assert!(v.iter().all(|v| v.is_finite()));
    v
}
fn period(bytes: &[u8]) -> u16 {
    let p = u16::try_from(i32::from_le_bytes(bytes.try_into().unwrap())).unwrap();
    assert!((32..=255).contains(&p));
    p
}
pub fn oracle(root: &Path) -> Oracle {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(root.join("full-frame-oracle.bin")).unwrap();
    assert_eq!(bytes.len(), 3864 + 24 * 4584);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "9c676f6c76aec71281ea097fe0c477c5a3b77f5022e639a0d82dbb4a5437fcc3"
    );
    Oracle {
        state: State::from_oracle(&f32s(&bytes[..3348])),
        history: f32s(&bytes[3348..3860]).try_into().unwrap(),
        period: period(&bytes[3860..3864]),
        cases: bytes[3864..]
            .as_chunks::<4584>()
            .0
            .iter()
            .map(|r| Case {
                features: f32s(&r[..80]).try_into().unwrap(),
                pcm: f32s(&r[80..720]).try_into().unwrap(),
                next: State::from_oracle(&f32s(&r[720..4068])),
                history: f32s(&r[4068..4580]).try_into().unwrap(),
                period: period(&r[4580..]),
            })
            .collect(),
    }
}
#[cfg(test)]
#[test]
fn full_frame_is_one_checked_plan_with_all_four_state_transitions() {
    super::on_large_stack(|| {
        let schema = schema();
        let resources: Resources = schema
            .resource_shapes
            .iter()
            .map(|(name, shape)| {
                let mut values = vec![0.; shape.iter().product::<u64>() as usize];
                if name == "signal_output_bias" {
                    values.fill(0.25);
                }
                (
                    name.clone(),
                    Resource::new(schema.ty(name).clone(), shape, values),
                )
            })
            .collect();
        let state = State {
            conv: [0.1; 164],
            gru1: [0.2; 160],
            gru2: [-0.4; 128],
            gru3: [0.6; 128],
            pitch: [0.1; 256],
            deemphasis: 0.,
        };
        let plan = prepare_entry_plan(&schema, &source(), ENTRY);
        let result = run_entry_plan(
            plan,
            &resources,
            inputs(&schema, &[0.; 20], &[0.; 128], 80, 80, &state),
            ExecutionMode::Normal,
        )
        .unwrap();
        assert_eq!(State::from_result(&result.value).gru1, [0.0125; 160]);
        assert!(result
            .signs
            .iter()
            .enumerate()
            .all(|(i, event)| event.sequence == i as u32));
        assert!(!result
            .signs
            .iter()
            .any(|event| matches!(event.kind, conduit_kernel::KernelEventKind::BackFailed)));
        assert!(pcm(&result).iter().all(|v| v.is_finite()));
        assert_eq!(floats(&result.outputs["next_history"]), [0.; 128]);
        for mode in [
            ExecutionMode::StoragePressure,
            ExecutionMode::CancelFirstExpression,
        ] {
            assert!(
                run_entry_plan(
                    prepare_entry_plan(&schema, &source(), ENTRY),
                    &resources,
                    inputs(&schema, &[0.; 20], &[0.; 128], 80, 80, &state),
                    mode
                )
                .is_none(),
                "a pressured or cancelled frame must expose no complete output set"
            );
        }
        eprintln!(
            "full authored FARGAN frame: {} nodes {} cords {:?}",
            result.nodes, result.cords, result.work
        );
    });
}
#[cfg(test)]
#[test]
#[ignore = "requires private pinned model and full-frame scalar oracle"]
fn pinned_full_frame_conditioning_and_recurrent_differential() {
    super::on_large_stack(|| {
        let root = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("explicit fixture"),
        );
        let schema = schema();
        let resources = resources(&root, &schema);
        let oracle = oracle(&root);
        let plan = prepare_entry_plan(&schema, &source(), ENTRY);
        let mut state = oracle.state;
        let mut history = oracle.history;
        let mut previous = oracle.period;
        let (mut pcm_error, mut state_error, mut history_error) = (0f32, 0f32, 0f32);
        for case in oracle.cases {
            let result = run_entry_plan(
                plan.clone(),
                &resources,
                inputs(
                    &schema,
                    &case.features,
                    &history,
                    case.period,
                    previous,
                    &state,
                ),
                ExecutionMode::Normal,
            )
            .unwrap();
            for (a, b) in pcm(&result).iter().zip(case.pcm) {
                pcm_error = pcm_error.max((a - b).abs());
            }
            state = State::from_result(&result.value);
            for (a, b) in state.flattened().iter().zip(case.next.flattened()) {
                state_error = state_error.max((a - b).abs());
            }
            history = floats(&result.outputs["next_history"]).try_into().unwrap();
            for (a, b) in history.iter().zip(case.history) {
                history_error = history_error.max((a - b).abs());
            }
            previous = case.period;
        }
        eprintln!("24 full frames, own accumulated conditioning/history/state: PCM max_abs={pcm_error}, signal state max_abs={state_error}, conditioning history max_abs={history_error}");
        assert!(pcm_error < 0.002 && state_error < 0.05 && history_error < 0.001);
    });
}

#[cfg(test)]
#[path = "frame_session.rs"]
mod session_tests;

#[cfg(test)]
#[path = "frame_pointwise_eligibility.rs"]
mod pointwise_eligibility_tests;

#[cfg(test)]
#[path = "frame_numeric_dags.rs"]
mod numeric_dag_tests;

#[cfg(test)]
#[path = "frame_local_expressions.rs"]
mod local_expression_tests;

#[cfg(test)]
#[path = "frame_matrix_dags.rs"]
mod matrix_dag_tests;

#[cfg(test)]
#[path = "frame_matrix_local.rs"]
mod matrix_local_tests;

#[cfg(test)]
#[path = "frame_preparation_cache.rs"]
mod preparation_cache_tests;
