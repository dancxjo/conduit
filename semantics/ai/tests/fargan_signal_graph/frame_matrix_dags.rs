//! Real full authored FARGAN DAG conformance; no elapsed-time speedup claim.
use super::*;
use conduit_core::verify_plan;
use conduit_kernel::{KernelEvent, KernelEventKind, NodeId};
fn logical(
    events: &[KernelEvent],
    node: u16,
) -> Vec<(Option<conduit_kernel::PortId>, KernelEventKind)> {
    events
        .iter()
        .filter(|event| {
            event.node == NodeId(node) && event.kind != KernelEventKind::StepFuelGranted
        })
        .map(|event| (event.port, event.kind))
        .collect()
}
#[test]
fn full_frame_selected_matrix_dags_match_canonical_and_unfused_outputs() {
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
        let values = inputs(&schema, &[0.01; 20], &[0.2; 128], 81, 80, &state);
        let source = source();
        let mut results = Vec::new();
        let mut source_id = None;
        for profile in [
            NumericProfile::Unfused,
            NumericProfile::CanonicalMatrixDag,
            NumericProfile::CompactMatrixDag,
        ] {
            let plan = prepare_entry_plan_profile(&schema, &source, ENTRY, profile);
            assert!(verify_plan(&plan));
            if let Some(id) = &source_id {
                assert_eq!(id, &plan.source_document_id);
            } else {
                source_id = Some(plan.source_document_id.clone());
            }
            assert_eq!(plan.fragments[0].placements.len(), 768);
            assert_eq!(plan.fragments[0].connections.len(), 1211);
            if profile != NumericProfile::Unfused {
                let dags = conduit_plan_lowering::lowering::lower_pure_value_fusion_dags(
                    &plan,
                    &plan.fragments[0].fragment_id,
                )
                .unwrap();
                assert!(!dags.is_empty());
                let matrix_members = dags
                    .iter()
                    .flat_map(|dag| &dag.nodes)
                    .filter(|node| {
                        plan.fragments[0].placements[usize::from(node.0)]
                            .kind_id
                            .as_str()
                            .starts_with("numeric/linear")
                    })
                    .count();
                assert_eq!(
                    matrix_members, 24,
                    "all reviewed GRU matrices must be selected"
                );
                assert_eq!(dags.len(), 32);
                eprintln!("selected matrix members: {matrix_members}");
                let mut counts = std::collections::BTreeMap::new();
                for dag in &dags {
                    *counts.entry(dag.nodes.len()).or_insert(0usize) += 1;
                }
                eprintln!(
                    "selected {profile:?}: {} DAGs, sizes {counts:?}",
                    dags.len()
                );
            }
            let result =
                run_entry_plan(plan, &resources, values.clone(), ExecutionMode::Normal).unwrap();
            eprintln!(
                "{profile:?}: {} retained Signs; work {:?}",
                result.signs.len(),
                result.work
            );
            results.push(result);
        }
        assert_eq!(results[0].outputs, results[1].outputs);
        assert_eq!(results[1].outputs, results[2].outputs);
        assert_eq!(results[1].signs, results[2].signs);
        assert_eq!(results[0].work.compact_values, 0);
        assert_eq!(results[1].work.compact_values, 0);
        assert_eq!(results[2].work.compact_values, 236);
        assert_eq!(
            results[1].work.canonical_values - results[2].work.canonical_values,
            results[2].work.compact_values
        );
        assert_eq!(
            results[1].work.canonical_bytes - results[2].work.canonical_bytes,
            results[2].work.compact_logical_bytes
        );
        assert!(results[2].work.compact_bytes < results[2].work.compact_logical_bytes);
        assert_eq!(
            results[1].work.logical_invocations,
            results[2].work.logical_invocations
        );
        assert_eq!(
            results[1].work.physical_decisions,
            results[2].work.physical_decisions
        );
        assert!(results[2].work.physical_decisions < results[0].work.physical_decisions);
        for node in 0..768 {
            assert_eq!(
                logical(&results[0].signs, node),
                logical(&results[2].signs, node),
                "logical node {node}"
            );
        }
    });
}

#[test]
#[ignore = "requires explicitly supplied private model and pinned full-float C oracle"]
fn pinned_matrix_dag_trajectory_matches_canonical_fresh_and_oracle() {
    pinned_profiles(
        NumericProfile::CanonicalMatrixDag,
        NumericProfile::CompactMatrixDag,
        false,
    );
}
pub(super) fn pinned_profiles(
    canonical_profile: NumericProfile,
    compact_profile: NumericProfile,
    local: bool,
) {
    crate::on_large_stack(move || {
        use crate::runtime::session::PreparedTrajectory;
        use conduit_core::bind_active_play;
        let root = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("explicit fixture"),
        );
        let schema = schema();
        let resources = resources(&root, &schema);
        let oracle = oracle(&root);
        assert_eq!(oracle.cases.len(), 24);
        let unfused = prepare_entry_plan(&schema, &source(), ENTRY);
        let canonical_plan =
            prepare_entry_plan_profile(&schema, &source(), ENTRY, canonical_profile);
        let compact_plan = prepare_entry_plan_profile(&schema, &source(), ENTRY, compact_profile);
        assert_eq!(unfused.source_document_id, compact_plan.source_document_id);
        assert_eq!(
            unfused.source_document_id,
            canonical_plan.source_document_id
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
        let mut canonical =
            PreparedTrajectory::new(canonical_plan, &resources, initial.clone(), 24);
        let mut compact = PreparedTrajectory::new(compact_plan, &resources, initial, 24);
        let addresses = (canonical.kernel_address(), compact.kernel_address());
        let (mut pcm_error, mut state_error, mut history_error) = (0f32, 0f32, 0f32);
        let (mut native_values, mut native_bytes, mut avoided_canonical_bytes) = (0u64, 0u64, 0u64);
        let mut expected_signs = None;
        let mut expected_values = None;
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
                for session in [&mut canonical, &mut compact] {
                    let prior = session.active();
                    let active = bind_active_play(
                        &prior.plan_id,
                        &prior.host_id,
                        &prior.boot_id,
                        index as u64,
                    );
                    let old = session.begin_next(active, encoded.clone()).unwrap();
                    assert_eq!(old.signs.events().count(), expected_signs.unwrap());
                }
            }
            let fresh = crate::runtime::run_entry_plan_at_sequence(
                unfused.clone(),
                &resources,
                encoded.clone(),
                index as u64,
            )
            .unwrap();
            let reference = canonical.run(ExecutionMode::Normal).unwrap();
            let result = compact.run(ExecutionMode::Normal).unwrap();
            assert_eq!(result.outputs, fresh.outputs, "unfused frame {index}");
            assert_eq!(
                result.outputs, reference.outputs,
                "canonical DAG frame {index}"
            );
            assert_eq!(
                result.signs, reference.signs,
                "all same-DAG Signs frame {index}"
            );
            for node in 0..768 {
                assert_eq!(
                    if local {
                        super::local_expression_tests::value_transactions(&fresh.signs, node)
                    } else {
                        logical(&fresh.signs, node)
                    },
                    if local {
                        super::local_expression_tests::value_transactions(&result.signs, node)
                    } else {
                        logical(&result.signs, node)
                    },
                    "unfused node {node} frame {index}"
                );
            }
            assert_eq!(
                (canonical.kernel_address(), compact.kernel_address()),
                addresses
            );
            assert_eq!(result.work.compact_values, 236);
            if local {
                assert_eq!(result.work.expression_calls, 0);
                assert_eq!(result.work.local_expression_calls, 207);
            }
            if let Some(expected) = expected_values {
                assert_eq!(result.work.compact_values, expected);
            }
            expected_values = Some(result.work.compact_values);
            expected_signs = Some(result.signs.len());
            assert_eq!(
                reference.work.canonical_values - result.work.canonical_values,
                result.work.compact_values
            );
            assert_eq!(
                reference.work.canonical_bytes - result.work.canonical_bytes,
                result.work.compact_logical_bytes
            );
            native_values += result.work.compact_values;
            native_bytes += result.work.compact_bytes;
            avoided_canonical_bytes += result.work.compact_logical_bytes;
            let names: Vec<_> = result.outputs.keys().cloned().collect();
            let active = compact.active().clone();
            let next = bind_active_play(
                &active.plan_id,
                &active.host_id,
                &active.boot_id,
                index as u64 + 1,
            );
            assert!(
                compact.begin_next(next, encoded).is_err(),
                "drain is not acceptance"
            );
            assert!(
                !compact.accept_outputs(canonical.active(), &names),
                "different physical Plan identity"
            );
            assert!(!compact.accept_outputs(&active, &names[..names.len() - 1]));
            assert!(compact.accept_outputs(&active, &names));
            assert!(canonical.accept_outputs(&canonical.active().clone(), &names));
            for (actual, expected) in pcm(&result).iter().zip(case.pcm) {
                pcm_error = pcm_error.max((actual - expected).abs());
            }
            // Temporal state/history advance only after both complete output sets are accepted.
            state = State::from_result(&result.value);
            history = floats(&result.outputs["next_history"]).try_into().unwrap();
            for (actual, expected) in state.flattened().iter().zip(case.next.flattened()) {
                state_error = state_error.max((actual - expected).abs());
            }
            for (actual, expected) in history.iter().zip(case.history) {
                history_error = history_error.max((actual - expected).abs());
            }
            previous = case.period;
        }
        assert!(pcm_error < 0.002 && state_error < 0.05 && history_error < 0.001);
        assert_eq!(native_values, 24 * expected_values.unwrap());
        eprintln!("pinned matrix DAG local={local}: 24 frames; exact all outputs/fresh per-node logical transactions/same-DAG complete Signs; {native_values} native values, {native_bytes} physical lane bytes, {avoided_canonical_bytes} avoided canonical bytes; PCM max_abs={pcm_error}; state max_abs={state_error}; history max_abs={history_error}. Diagnostic conformance, not a speed or hardware-traffic claim.");
    });
}

#[test]
fn matrix_trajectory_cancellation_retains_terminal_evidence_and_refuses_rearm() {
    crate::on_large_stack(|| {
        use crate::runtime::session::PreparedTrajectory;
        use conduit_core::bind_active_play;
        let schema = schema();
        let resources = session_tests::zero_resources(&schema);
        let plan =
            prepare_entry_plan_profile(&schema, &source(), ENTRY, NumericProfile::CompactMatrixDag);
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
