#![cfg(feature = "kernel-step")]
#[path = "fargan_signal_graph/plan.rs"]
mod plan;
#[path = "fargan_signal_graph/runtime.rs"]
mod runtime;
#[path = "fargan_signal_graph/shared.rs"]
mod shared;
#[path = "fargan_signal_graph/state.rs"]
mod state;
use plan::*;
use state::*;
fn on_large_stack(work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn authored_subframe_runs_with_exact_std_owners_and_one_complete_state_result() {
    on_large_stack(|| {
        let schema = SourceSchema::prepare();
        let resources = schema.zero_resources();
        let state = State {
            conv: [-0.2; 164],
            gru1: [0.2; 160],
            gru2: [-0.4; 128],
            gru3: [0.6; 128],
            pitch: std::array::from_fn(|i| (i as f32 - 128.) / 256.),
            deemphasis: 0.1,
        };
        let condition = std::array::from_fn(|i| i as f32 * 0.001);
        let actual = runtime::run_graph(&schema, &resources, condition, 80, &state);
        assert_eq!(actual.state.gru1, [0.1; 160]);
        assert_eq!(actual.state.gru2, [-0.2; 128]);
        assert_eq!(actual.state.gru3, [0.3; 128]);
        let raw = 0.25f64.tanh();
        let mut prior = 0.1f64;
        for sample in actual.pcm {
            prior = raw + f64::from(0.85f32) * prior;
            assert!((f64::from(sample) - prior).abs() < 2e-6);
        }
        assert!((f64::from(actual.state.deemphasis) - prior).abs() < 2e-6);
        assert_eq!(&actual.state.pitch[..216], &state.pitch[40..]);
        for value in &actual.state.pitch[216..] {
            assert!((f64::from(*value) - raw).abs() < 1e-6);
        }
        assert_eq!(&actual.state.conv[..80], &condition);
        for i in 0..44 {
            let expected = f64::from(state.pitch[174 + i]) / (1. + f64::from(1e-5f32));
            assert!((f64::from(actual.state.conv[80 + i]) - expected).abs() < 1e-7);
        }
        for i in 0..40 {
            let expected = f64::from(state.pitch[216 + i]) / (1. + f64::from(1e-5f32));
            assert!((f64::from(actual.state.conv[124 + i]) - expected).abs() < 1e-7);
        }
        eprintln!("ordinary authored subframe: {} nodes {} cords preparation={:?}, execution={:?}; per-invocation Value proof, not reusable streaming",actual.nodes,actual.cords,actual.preparation,actual.execution);
    });
}

#[path = "fargan_signal_graph/pinned.rs"]
mod pinned;
/// Private pinned tensors are not downloaded or published by ordinary CI.
#[test]
#[ignore = "requires explicitly supplied private model and pinned full-float C oracle"]
fn pinned_scalar_full_float_subframe_pcm_and_accumulated_state_differential() {
    on_large_stack(|| {
        let root = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("explicit local fixture"),
        );
        let schema = SourceSchema::prepare();
        let resources = pinned::load(&root, &schema);
        let cases = pinned::oracle(&root);
        let mut state = cases[0].prior.clone();
        let mut pcm_error = 0f32;
        let mut state_error = 0f32;
        let mut preparation = std::time::Duration::ZERO;
        let mut execution = std::time::Duration::ZERO;
        for (index, case) in cases.iter().enumerate() {
            let actual =
                runtime::run_graph(&schema, &resources, case.condition, case.period, &state);
            for (a, b) in actual.pcm.iter().zip(&case.pcm) {
                pcm_error = pcm_error.max((a - b).abs());
            }
            for (a, b) in actual.state.flattened().iter().zip(case.next.flattened()) {
                state_error = state_error.max((a - b).abs());
            }
            preparation += actual.preparation;
            execution += actual.execution;
            state = actual.state;
            if index % 16 == 15 {
                eprintln!("pinned full-float subframes={} accumulated PCM max_abs={pcm_error}, active-state max_abs={state_error}",index+1);
            }
        }
        eprintln!("96 authored ordinary Plan invocations: PCM max_abs={pcm_error}; active state max_abs={state_error}; total preparation={preparation:?}, execution={execution:?}. Uses exact oracle conditioning; upstream activation approximations differ from libm. Per-invocation Value, not streaming/realtime/intelligibility proof.");
        assert!(pcm_error < 0.002);
        assert!(state_error < 0.02);
    });
}

#[test]
fn authored_subframe_pressure_and_host_cancellation_publish_no_partial_state_or_pcm() {
    on_large_stack(|| {
        let schema = SourceSchema::prepare();
        let resources = schema.zero_resources();
        let prior = State {
            conv: [0.1; 164],
            gru1: [0.2; 160],
            gru2: [0.3; 128],
            gru3: [0.4; 128],
            pitch: [0.5; 256],
            deemphasis: 0.6,
        };
        let retained = prior.flattened();
        for mode in [
            runtime::ExecutionMode::StoragePressure,
            runtime::ExecutionMode::CancelFirstExpression,
        ] {
            assert!(
                runtime::try_run_graph(&schema, &resources, [0.01; 80], 80, &prior, mode).is_none()
            );
            assert_eq!(prior.flattened(), retained);
        }
    });
}
