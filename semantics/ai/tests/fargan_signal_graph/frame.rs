//! Original full-frame tests retain their established names and assertions.
pub use super::frame_support::*;
use super::{plan::*, runtime::*, shared::*, state::*};
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
