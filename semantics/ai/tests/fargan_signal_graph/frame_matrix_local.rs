//! Composition of already admitted matrix/slice regions and local expressions.
use super::local_expression_tests::value_transactions;
use super::*;
#[test]
fn matrix_local_profile_preserves_source_outputs_and_both_evidence_boundaries() {
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
            NumericProfile::CanonicalMatrixDagLocalExpressions,
            NumericProfile::CompactMatrixDagLocalExpressions,
        ] {
            let plan = prepare_entry_plan_profile(&schema, &source(), ENTRY, profile);
            assert!(conduit_core::verify_plan(&plan));
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
            if profile.dags() {
                let dags = conduit_plan_lowering::lowering::lower_pure_value_fusion_dags(
                    &plan,
                    &plan.fragments[0].fragment_id,
                )
                .unwrap();
                assert_eq!(dags.len(), 32);
                assert_eq!(
                    dags.iter()
                        .flat_map(|d| &d.nodes)
                        .filter(|n| plan.fragments[0].placements[usize::from(n.0)]
                            .kind_id
                            .as_str()
                            .starts_with("numeric/linear"))
                        .count(),
                    24
                );
            }
            if profile.local_expressions() {
                assert!(plan.fragments[0]
                    .placements
                    .iter()
                    .all(|p| p.host_calls.is_empty()));
            }
            let result =
                run_entry_plan(plan, &resources, input.clone(), ExecutionMode::Normal).unwrap();
            eprintln!(
                "{profile:?}: {} Signs; work {:?}",
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
        assert_eq!(results[2].signs, results[3].signs);
        assert_eq!(results[2].work.compact_values, 0);
        assert_eq!(results[3].work.compact_values, 236);
        assert_eq!(
            results[2].work.canonical_values - results[3].work.canonical_values,
            236
        );
        assert_eq!(
            results[2].work.canonical_bytes - results[3].work.canonical_bytes,
            results[3].work.compact_logical_bytes
        );
        assert_eq!(
            results[2].work.physical_decisions,
            results[3].work.physical_decisions
        );
        assert!(results[3].work.physical_decisions < results[1].work.physical_decisions);
    });
}
#[test]
#[ignore = "requires explicit private pinned full-float model and oracle"]
fn pinned_matrix_local_trajectory_matches_same_dag_fresh_source_and_oracle() {
    super::matrix_dag_tests::pinned_profiles(
        NumericProfile::CanonicalMatrixDagLocalExpressions,
        NumericProfile::CompactMatrixDagLocalExpressions,
        true,
    );
}
