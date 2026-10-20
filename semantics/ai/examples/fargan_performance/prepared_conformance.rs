//! Comparison boundaries for matched full-frame physical realizations.
use crate::runtime::EncodedResultAndTiming;
use conduit_core::Plan;
use conduit_kernel::{KernelEvent, KernelEventKind, NodeId};

pub(super) fn same_source(reference: &Plan, candidate: &Plan) {
    assert_eq!(reference.source_document_id, candidate.source_document_id);
    assert_eq!(reference.checked_plot_id, candidate.checked_plot_id);
    assert_eq!(reference.expanded_plot_id, candidate.expanded_plot_id);
}
fn transactions(
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
pub(super) fn compare(
    reference: &EncodedResultAndTiming,
    candidate: &EncodedResultAndTiming,
    index: usize,
    profile: super::Profile,
) {
    assert_eq!(
        (reference.nodes, reference.cords),
        (candidate.nodes, candidate.cords)
    );
    if profile.local() {
        assert_eq!((candidate.nodes, candidate.cords), (768, 1211));
        assert_eq!(candidate.work.expression_calls, 0);
        assert_eq!(candidate.work.local_expression_calls, 207);
        assert_eq!(
            candidate.work.compact_values + candidate.work.owned_values,
            profile.native_values()
        );
        assert!(!candidate.signs.iter().any(|event| matches!(
            event.kind,
            KernelEventKind::HostCallRequested
                | KernelEventKind::HostCallCompleted
                | KernelEventKind::HostCallCancellationRequested
        )));
        for node in 0..768 {
            assert_eq!(
                transactions(&reference.signs, node),
                transactions(&candidate.signs, node),
                "frame {index} node {node}"
            );
        }
    } else {
        assert_eq!(candidate.signs.len(), reference.signs.len());
        for (actual, expected) in candidate.signs.iter().zip(&reference.signs) {
            let mut expected = *expected;
            expected.request = expected
                .request
                .map(|r| conduit_kernel::RequestId(r.0 + index as u32));
            assert_eq!(*actual, expected);
        }
    }
}

#[test]
#[ignore = "requires explicit private pinned fixture and new diagnostic capture directory"]
fn pinned_compact_local_capture_covers_all_trajectory_phases() {
    crate::on_large_stack(|| {
        let fixture = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("explicit fixture"),
        );
        let output = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DIAGNOSTIC_OUTPUT")
                .expect("explicit new output directory"),
        );
        std::fs::create_dir(&output).unwrap();
        let schema = crate::frame::schema();
        let resources = crate::frame::resources(&fixture, &schema);
        let oracle = crate::frame::oracle(&fixture);
        let mut raw = crate::artifact::Raw::new(&output.join("raw.jsonl")).unwrap();
        super::run(
            &output,
            2,
            &schema,
            &resources,
            &oracle,
            &mut raw,
            super::Profile::CompactLocal,
        )
        .unwrap();
        raw.finish().unwrap();
        let rows = std::fs::read_to_string(output.join("raw.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = rows
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        for (phase, count) in [
            ("paired-fargan-frame", 96),
            ("paired-frame-conformance", 48),
            ("prepared-rearm", 46),
            ("paired-evidence-reconstruction-and-write", 48),
            ("paired-frame-result-release", 48),
            ("prepared-trajectory-release", 2),
        ] {
            assert_eq!(
                rows.iter().filter(|row| row["phase"] == phase).count(),
                count,
                "{phase}"
            );
        }
        for row in rows
            .iter()
            .filter(|row| row["phase"] == "paired-fargan-frame")
        {
            assert!(row["outputs_sha256"].as_str().is_some());
            assert!(row["signs_sha256"].as_str().is_some());
            if row["profile"] == "prepared-compact-local" {
                assert_eq!(row["local_expression_calls"], 207);
                assert_eq!(row["expression_calls"], 0);
                assert_eq!(row["native_values"], 140);
                assert_eq!(row["canonical_bytes_avoided"], 207056);
            }
        }
    });
}

#[test]
#[ignore = "requires explicit private pinned fixture and new diagnostic capture directory"]
fn pinned_owned_matrix_reuses_real_provider_buffers_without_play_allocations() {
    crate::on_large_stack(|| {
        let fixture = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("explicit fixture"),
        );
        let output = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DIAGNOSTIC_OUTPUT")
                .expect("explicit new output directory"),
        );
        std::fs::create_dir(&output).unwrap();
        let schema = crate::frame::schema();
        let resources = crate::frame::resources(&fixture, &schema);
        let oracle = crate::frame::oracle(&fixture);
        let first = &oracle.cases[0];
        let inputs = crate::frame::inputs(
            &schema,
            &first.features,
            &oracle.history,
            first.period,
            oracle.period,
            &oracle.state,
        );
        let plan = crate::plan::prepare_entry_plan_profile(
            &schema,
            &crate::plan::source(),
            crate::frame::ENTRY,
            crate::plan::NumericProfile::CompactMatrixDagLocalExpressions,
        );
        eprintln!("owned diagnostic: exact matrix Plan prepared");
        let mut copied = crate::runtime::session::PreparedTrajectory::new(
            plan.clone(),
            &resources,
            inputs.clone(),
            2,
        );
        eprintln!("owned diagnostic: copied owner prepared");
        let mut owned = crate::runtime::session::PreparedTrajectory::new_owned_compact(
            plan,
            &resources,
            inputs.clone(),
            2,
        );
        eprintln!("owned diagnostic: owned owner prepared");
        let mut observations = Vec::new();
        for sequence in 0..2 {
            eprintln!("owned diagnostic: invocation {sequence}");
            if sequence != 0 {
                for session in [&mut copied, &mut owned] {
                    let current = session.active();
                    let next = conduit_core::bind_active_play(
                        &current.plan_id,
                        &current.host_id,
                        &current.boot_id,
                        sequence,
                    );
                    let _prior = session.begin_next(next, inputs.clone()).unwrap();
                }
            }
            let (reference, reference_metrics) = super::observe(|before, after| {
                copied.run_observed(crate::runtime::ExecutionMode::Normal, before, after)
            })
            .unwrap();
            let (candidate, candidate_metrics) = super::observe(|before, after| {
                owned.run_observed(crate::runtime::ExecutionMode::Normal, before, after)
            })
            .unwrap();
            assert_eq!(candidate.outputs, reference.outputs);
            assert_eq!(candidate.signs, reference.signs);
            assert!(candidate.work.owned_values > 0);
            assert!(candidate.work.compact_bytes < reference.work.compact_bytes);
            assert_eq!(
                candidate.work.compact_values + candidate.work.owned_values,
                reference.work.compact_values
            );
            assert_eq!(
                candidate.work.compact_bytes + candidate.work.owned_bytes,
                reference.work.compact_bytes
            );
            assert_eq!(
                candidate.work.compact_logical_bytes + candidate.work.owned_logical_bytes,
                reference.work.compact_logical_bytes
            );
            for session in [&mut copied, &mut owned] {
                let names: Vec<_> = candidate.outputs.keys().cloned().collect();
                let active = session.active().clone();
                assert!(session.accept_outputs(&active, &names));
            }
            observations.push(serde_json::json!({"sequence":sequence,"copied_publication_bytes":reference.work.compact_bytes,"owned_profile_copied_publication_bytes":candidate.work.compact_bytes,"transferred_publication_bytes":candidate.work.owned_bytes,"owned_values":candidate.work.owned_values,"outputs_and_events_equal":true,"copied_metrics":reference_metrics,"owned_metrics":candidate_metrics}));
        }
        crate::artifact::json(&output.join("repeated-real-frame.json"), &serde_json::json!({"scope":"same pinned first frame repeated twice with fresh active Play identity; diagnostic, not calibrated full-owner performance","observations":observations})).unwrap();
    });
}
