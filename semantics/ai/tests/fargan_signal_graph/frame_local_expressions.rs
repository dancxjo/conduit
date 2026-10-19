//! Same authored FARGAN, explicitly selected computational expressions.
use super::*;
use conduit_kernel::{KernelEvent, KernelEventKind, NodeId};
fn value_transactions(
    events: &[KernelEvent],
    node: u16,
) -> Vec<(Option<conduit_kernel::PortId>, KernelEventKind)> {
    use KernelEventKind::*;
    events
        .iter()
        .filter(|event| event.node == NodeId(node))
        .filter(|event| {
            matches!(
                event.kind,
                ValueConsumed
                    | ValueRouted
                    | BackCompleted
                    | SemanticAbnormal
                    | SemanticAbnormalRecovered
            )
        })
        .map(|event| (event.port, event.kind))
        .collect()
}
#[test]
fn full_frame_local_expression_profile_preserves_source_and_all_outputs() {
    crate::on_large_stack(|| {
        let schema = schema();
        let resources = session_tests::zero_resources(&schema);
        let state = State {
            conv: [0.1; 164],
            gru1: [0.2; 160],
            gru2: [-0.4; 128],
            gru3: [0.6; 128],
            pitch: [0.1; 256],
            deemphasis: 0.,
        };
        let input = inputs(&schema, &[0.01; 20], &[0.2; 128], 81, 80, &state);
        let mut identity = None;
        let mut results = Vec::new();
        for profile in [
            NumericProfile::Unfused,
            NumericProfile::LocalExpressions,
            NumericProfile::CompactDagLocalExpressions,
        ] {
            let plan = prepare_entry_plan_profile(&schema, &source(), ENTRY, profile);
            let current = (
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
                plan.expanded_plot_id.clone(),
            );
            if let Some(expected) = &identity {
                assert_eq!(expected, &current);
            } else {
                identity = Some(current);
            }
            assert_eq!(plan.fragments[0].placements.len(), 768);
            assert_eq!(plan.fragments[0].connections.len(), 1211);
            if profile.local_expressions() {
                assert!(plan.fragments[0]
                    .placements
                    .iter()
                    .all(|p| p.host_calls.is_empty()));
            }
            if profile.local_expressions() {
                let owner = plan.fragments[0]
                    .placements
                    .iter()
                    .find(|placement| {
                        placement.implementation_id.as_str()
                            == conduit_semantic_catalog::EXPRESSION_VALUE_IMPLEMENTATION
                    })
                    .unwrap();
                for field in 0..6 {
                    let mut forged = owner.clone();
                    match field {
                        0 => forged.artifact_id = "unselected/artifact".into(),
                        1 => forged.capability_id = "unselected/capability".into(),
                        2 => forged.execution_profile_id = "unselected/profile".into(),
                        3 => forged.semantic_contract = Default::default(),
                        4 => {
                            forged.inputs[0].temporal =
                                conduit_core::PortTemporal::Flow { closes: true }
                        }
                        5 => forged.outputs[0].value_kind = "unselected/output-kind".into(),
                        _ => unreachable!(),
                    }
                    assert!(conduit_semantic_catalog::prepare_expression_value(&forged).is_err());
                }
            }
            let result =
                run_entry_plan(plan, &resources, input.clone(), ExecutionMode::Normal).unwrap();
            eprintln!(
                "{profile:?}: {} retained Signs; work {:?}",
                result.signs.len(),
                result.work
            );
            results.push(result);
        }
        for candidate in &results[1..] {
            assert_eq!(results[0].outputs, candidate.outputs);
            assert_eq!(candidate.work.expression_calls, 0);
            assert_eq!(candidate.work.local_expression_calls, 207);
            for node in 0..768 {
                assert_eq!(
                    value_transactions(&results[0].signs, node),
                    value_transactions(&candidate.signs, node),
                    "node {node}"
                );
            }
        }
        assert_eq!(results[2].work.compact_values, 140);
        assert_eq!(
            results[1].work.canonical_values - results[2].work.canonical_values,
            140
        );
    });
}

#[test]
#[ignore = "requires explicitly supplied private model and pinned full-float C oracle"]
fn pinned_local_expression_compact_trajectory_matches_hosted_fresh_and_oracle() {
    crate::on_large_stack(|| {
        use crate::runtime::session::PreparedTrajectory;
        use conduit_core::bind_active_play;
        let root = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("explicit fixture"),
        );
        let schema = schema();
        let resources = resources(&root, &schema);
        let oracle = oracle(&root);
        assert_eq!(oracle.cases.len(), 24);
        let hosted = prepare_entry_plan(&schema, &source(), ENTRY);
        let local = prepare_entry_plan_profile(
            &schema,
            &source(),
            ENTRY,
            NumericProfile::CompactDagLocalExpressions,
        );
        assert_eq!(
            (
                &hosted.source_document_id,
                &hosted.checked_plot_id,
                &hosted.expanded_plot_id
            ),
            (
                &local.source_document_id,
                &local.checked_plot_id,
                &local.expanded_plot_id
            )
        );
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
        let mut session = PreparedTrajectory::new(local, &resources, initial, 24);
        let address = session.kernel_address();
        let (mut pcm_error, mut state_error, mut history_error) = (0f32, 0f32, 0f32);
        let mut prior_signs = None;
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
                let old = session.begin_next(active, encoded.clone()).unwrap();
                assert_eq!(
                    old.signs.events().collect::<Vec<_>>(),
                    prior_signs.take().unwrap()
                );
            }
            let reference = crate::runtime::run_entry_plan_at_sequence(
                hosted.clone(),
                &resources,
                encoded.clone(),
                index as u64,
            )
            .unwrap();
            let result = session.run(ExecutionMode::Normal).unwrap();
            assert_eq!(result.outputs, reference.outputs, "frame {index}");
            assert_eq!(session.kernel_address(), address);
            assert_eq!(result.work.expression_calls, 0);
            assert_eq!(result.work.local_expression_calls, 207);
            assert_eq!(result.work.compact_values, 140);
            assert!(!result.signs.iter().any(|event| matches!(
                event.kind,
                KernelEventKind::HostCallRequested
                    | KernelEventKind::HostCallCompleted
                    | KernelEventKind::HostCallCancellationRequested
            )));
            for node in 0..768 {
                assert_eq!(
                    value_transactions(&reference.signs, node),
                    value_transactions(&result.signs, node),
                    "node {node} frame {index}"
                );
            }
            let active = session.active().clone();
            let next = bind_active_play(
                &active.plan_id,
                &active.host_id,
                &active.boot_id,
                index as u64 + 1,
            );
            assert!(session.begin_next(next, encoded).is_err());
            let names: Vec<_> = result.outputs.keys().cloned().collect();
            assert!(!session.accept_outputs(&active, &names[..names.len() - 1]));
            assert!(session.accept_outputs(&active, &names));
            for (actual, expected) in pcm(&result).iter().zip(case.pcm) {
                pcm_error = pcm_error.max((actual - expected).abs());
            }
            state = State::from_result(&result.value);
            history = floats(&result.outputs["next_history"]).try_into().unwrap();
            for (actual, expected) in state.flattened().iter().zip(case.next.flattened()) {
                state_error = state_error.max((actual - expected).abs());
            }
            for (actual, expected) in history.iter().zip(case.history) {
                history_error = history_error.max((actual - expected).abs());
            }
            prior_signs = Some(result.signs);
            previous = case.period;
        }
        assert!(pcm_error < 0.002 && state_error < 0.05 && history_error < 0.001);
        eprintln!("local expression + compact DAG:24frames exact hosted fresh outputs and per-node value transactions;207 computations/frame,0 Host calls/frame,140 compact values/frame; PCM max_abs={pcm_error}; state max_abs={state_error}; history max_abs={history_error}. Diagnostic conformance, no accepted speedup claim.");
    });
}

#[test]
fn local_expression_trajectory_cancellation_retains_terminal_evidence_and_refuses_rearm() {
    crate::on_large_stack(|| {
        use crate::runtime::session::PreparedTrajectory;
        use conduit_core::bind_active_play;
        let schema = schema();
        let resources = session_tests::zero_resources(&schema);
        let plan = prepare_entry_plan_profile(
            &schema,
            &source(),
            ENTRY,
            NumericProfile::CompactDagLocalExpressions,
        );
        let state = State {
            conv: [0.; 164],
            gru1: [0.; 160],
            gru2: [0.; 128],
            gru3: [0.; 128],
            pitch: [0.; 256],
            deemphasis: 0.,
        };
        let encoded = inputs(&schema, &[0.; 20], &[0.; 128], 80, 80, &state);
        let mut session = PreparedTrajectory::new(plan, &resources, encoded.clone(), 2);
        let result = session.run(ExecutionMode::Normal).unwrap();
        let active = session.active().clone();
        session.cancel().unwrap();
        assert!(session
            .retained_signs()
            .any(|event| event.kind == KernelEventKind::RunCancelled));
        let names: Vec<_> = result.outputs.keys().cloned().collect();
        assert!(!session.accept_outputs(&active, &names));
        let next = bind_active_play(&active.plan_id, &active.host_id, &active.boot_id, 1);
        assert!(session.begin_next(next, encoded).is_err());
        assert!(session.run(ExecutionMode::Normal).is_none());
    });
}
