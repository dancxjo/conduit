//! Full authored graph parity and semantic acceptance fences across invocations.
use super::*;
use crate::runtime::session::PreparedTrajectory;
use conduit_core::bind_active_play;
fn zero_resources(schema: &SourceSchema) -> Resources {
    schema
        .resource_shapes
        .iter()
        .map(|(name, shape)| {
            (
                name.clone(),
                Resource::new(
                    schema.ty(name).clone(),
                    shape,
                    vec![0.; shape.iter().product::<u64>() as usize],
                ),
            )
        })
        .collect()
}
#[test]
fn full_frame_retains_owners_and_requires_exact_output_acceptance_before_rearm() {
    crate::on_large_stack(|| {
        let schema = schema();
        let resources = zero_resources(&schema);
        let plan = prepare_entry_plan(&schema, &source(), ENTRY);
        let state = State {
            conv: [0.1; 164],
            gru1: [0.2; 160],
            gru2: [-0.4; 128],
            gru3: [0.6; 128],
            pitch: [0.1; 256],
            deemphasis: 0.,
        };
        let initial = inputs(&schema, &[0.; 20], &[0.; 128], 80, 80, &state);
        let fresh = run_entry_plan(
            plan.clone(),
            &resources,
            initial.clone(),
            ExecutionMode::Normal,
        )
        .unwrap();
        let mut session = PreparedTrajectory::new(plan.clone(), &resources, initial.clone(), 2);
        let address = session.kernel_address();
        let first = session.run(ExecutionMode::Normal).unwrap();
        assert_eq!(first.outputs, fresh.outputs);
        assert_eq!(first.signs, fresh.signs);
        let active = session.active().clone();
        let next_active = bind_active_play(&active.plan_id, &active.host_id, &active.boot_id, 1);
        assert!(
            session
                .begin_next(next_active.clone(), initial.clone())
                .is_err(),
            "drain is not output acceptance"
        );
        let mut stale = active.clone();
        stale.play_sequence = 1;
        let names: Vec<_> = first.outputs.keys().cloned().collect();
        assert!(!session.accept_outputs(&stale, &names));
        assert!(!session.accept_outputs(&active, &names[..names.len() - 1]));
        assert!(session.accept_outputs(&active, &names));
        assert!(
            session.begin_next(active.clone(), initial.clone()).is_err(),
            "old active identity"
        );
        let mut invalid = initial.clone();
        invalid.insert("state".into(), vec![0]);
        assert!(session.begin_next(next_active.clone(), invalid).is_err());
        let next_state = State::from_result(&first.value);
        let changed = inputs(&schema, &[0.01; 20], &[0.2; 128], 81, 80, &next_state);
        let fresh_next =
            run_entry_plan(plan, &resources, changed.clone(), ExecutionMode::Normal).unwrap();
        let old = session.begin_next(next_active.clone(), changed).unwrap();
        assert_eq!(
            old.signs.events().collect::<Vec<_>>(),
            first.signs,
            "old evidence transferred intact"
        );
        let next = session.run(ExecutionMode::Normal).unwrap();
        assert_eq!(session.active(), &next_active);
        assert_eq!(session.kernel_address(), address);
        assert_eq!(next.outputs, fresh_next.outputs);
        assert_eq!(next.signs.len(), fresh_next.signs.len());
        for (actual, reference) in next.signs.iter().zip(&fresh_next.signs) {
            let mut expected = *reference;
            // Fresh owners start at their first Request; retained owners must
            // advance exactly once, preserving every correlated event.
            expected.request = expected.request.map(|r| conduit_kernel::RequestId(r.0 + 1));
            assert_eq!(*actual, expected);
        }
        assert!(!session.accept_outputs(&active, &names));
        assert!(session.accept_outputs(&next_active, &names));
        let third = bind_active_play(&active.plan_id, &active.host_id, &active.boot_id, 2);
        assert!(
            session.begin_next(third, initial).is_err(),
            "finite reservation exhausted"
        );
    });
}
#[test]
fn cancelled_full_frame_cannot_be_rearmed_or_acknowledged() {
    crate::on_large_stack(|| {
        let schema = schema();
        let resources = zero_resources(&schema);
        let state = State {
            conv: [0.; 164],
            gru1: [0.; 160],
            gru2: [0.; 128],
            gru3: [0.; 128],
            pitch: [0.; 256],
            deemphasis: 0.,
        };
        let initial = inputs(&schema, &[0.; 20], &[0.; 128], 80, 80, &state);
        let mut session = PreparedTrajectory::new(
            prepare_entry_plan(&schema, &source(), ENTRY),
            &resources,
            initial.clone(),
            2,
        );
        let active = session.active().clone();
        assert!(session.run(ExecutionMode::CancelFirstExpression).is_none());
        assert!(!session.accept_outputs(&active, &[]));
        assert!(session
            .begin_next(
                bind_active_play(&active.plan_id, &active.host_id, &active.boot_id, 1),
                initial
            )
            .is_err());
        assert!(session.run(ExecutionMode::Normal).is_none());
    });
}

#[test]
#[ignore = "requires explicit pinned model and full-frame oracle"]
fn pinned_full_frame_reusable_trajectory_matches_fresh_owners_and_scalar_oracle() {
    crate::on_large_stack(|| {
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
        let first = &oracle.cases[0];
        let initial = inputs(
            &schema,
            &first.features,
            &history,
            first.period,
            previous,
            &state,
        );
        let mut session =
            PreparedTrajectory::new(plan.clone(), &resources, initial, oracle.cases.len());
        let address = session.kernel_address();
        let (mut pcm_error, mut state_error, mut history_error) = (0f32, 0f32, 0f32);
        let (mut fresh_preparation, mut fresh_play, mut retained_play, mut rearm) = (
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
        );
        for (index, case) in oracle.cases.iter().enumerate() {
            let encoded = inputs(
                &schema,
                &case.features,
                &history,
                case.period,
                previous,
                &state,
            );
            if index > 0 {
                let prior = session.active();
                let active =
                    bind_active_play(&prior.plan_id, &prior.host_id, &prior.boot_id, index as u64);
                let start = std::time::Instant::now();
                let old = session.begin_next(active, encoded.clone()).unwrap();
                rearm += start.elapsed();
                assert_eq!(old.signs.events().count(), 6976);
            }
            let fresh = crate::runtime::run_entry_plan_at_sequence(
                plan.clone(),
                &resources,
                encoded,
                index as u64,
            )
            .unwrap();
            let result = session.run(ExecutionMode::Normal).unwrap();
            fresh_preparation += fresh.preparation;
            fresh_play += fresh.execution;
            retained_play += result.execution;
            assert_eq!(result.outputs, fresh.outputs, "frame {index}");
            assert_eq!(session.kernel_address(), address);
            assert_eq!(result.signs.len(), fresh.signs.len());
            for (actual, reference) in result.signs.iter().zip(&fresh.signs) {
                let mut expected = *reference;
                expected.request = expected
                    .request
                    .map(|r| conduit_kernel::RequestId(r.0 + index as u32));
                assert_eq!(*actual, expected, "frame {index}");
            }
            for (a, b) in pcm(&result).iter().zip(case.pcm) {
                pcm_error = pcm_error.max((a - b).abs());
            }
            // State/history advance only after the complete output set is accepted.
            let names: Vec<_> = result.outputs.keys().cloned().collect();
            let active = session.active().clone();
            assert!(session.accept_outputs(&active, &names));
            state = State::from_result(&result.value);
            history = floats(&result.outputs["next_history"]).try_into().unwrap();
            for (a, b) in state.flattened().iter().zip(case.next.flattened()) {
                state_error = state_error.max((a - b).abs());
            }
            for (a, b) in history.iter().zip(case.history) {
                history_error = history_error.max((a - b).abs());
            }
            previous = case.period;
        }
        eprintln!("prepared full-frame trajectory:24frames exact fresh outputs and correlated Signs; PCM max_abs={pcm_error}; state max_abs={state_error}; history max_abs={history_error}; fresh_preparation={fresh_preparation:?}; fresh_play={fresh_play:?}; retained_play={retained_play:?}; rearm={rearm:?}. Diagnostic build, not a throughput claim.");
        assert!(pcm_error < 0.002 && state_error < 0.05 && history_error < 0.001);
    });
}
