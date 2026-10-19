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
    compact_local: bool,
) {
    assert_eq!(
        (reference.nodes, reference.cords),
        (candidate.nodes, candidate.cords)
    );
    if compact_local {
        assert_eq!((candidate.nodes, candidate.cords), (768, 1211));
        assert_eq!(candidate.work.expression_calls, 0);
        assert_eq!(candidate.work.local_expression_calls, 207);
        assert_eq!(candidate.work.compact_values, 140);
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
        super::run(&output, 2, &schema, &resources, &oracle, &mut raw, true).unwrap();
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
