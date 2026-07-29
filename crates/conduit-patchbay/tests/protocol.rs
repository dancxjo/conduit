use conduit_patchbay::{
    EditOperation, EditRequest, NodePosition, PATCHBAY_PROTOCOL_V1, PlanSnapshot, ProjectionLog,
    ProjectionUpdate, RunSnapshot, RunState, SubjectPath, Workspace,
};

const SOURCE: &str = "panel 1\nnode greeting : conduit/literal { value = \"hello\\n\" }\nnode output : conduit/stdout\ncord greeting.out -> output.in\n";
const FIXTURE: &str = include_str!("../../../conformance/c8/patchbay-protocol-v1.json");

fn request(workspace: &Workspace, operations: Vec<EditOperation>) -> EditRequest {
    EditRequest {
        protocol_version: PATCHBAY_PROTOCOL_V1,
        document_id: workspace.source().document_id.clone(),
        expected_source_revision: workspace.source().revision,
        expected_presentation_revision: workspace.presentation().revision,
        operations,
    }
}

#[test]
fn move_only_changes_presentation_identity() {
    let mut workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let semantic = workspace.semantic();
    let original_presentation = workspace.presentation().identity.clone();
    let result = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::MoveNode {
                node_id: "greeting".to_owned(),
                position: NodePosition { x: 24, y: -8 },
            }],
        ))
        .expect("move applies");
    assert_eq!(result.semantic, semantic);
    assert_eq!(result.source.revision, 0);
    assert_eq!(result.presentation.revision, 1);
    assert_ne!(result.presentation.identity, original_presentation);
}

#[test]
fn source_edit_changes_semantics_but_not_an_existing_run() {
    let mut workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let old = workspace.semantic().source_semantic_hash;
    let plan = PlanSnapshot {
        identity: "sha256:plan".to_owned(),
        source_semantic_hash: old.clone(),
    };
    let run = RunSnapshot {
        run_id: "run/1".to_owned(),
        plan_identity: plan.identity,
        source_semantic_hash: plan.source_semantic_hash,
        state: RunState::Running,
    };
    let result = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::ReplaceSource {
                source: SOURCE.replace("hello", "goodbye"),
            }],
        ))
        .expect("source edit applies");
    assert_ne!(result.semantic.source_semantic_hash, old);
    assert_eq!(run.source_semantic_hash, old);
    assert_eq!(run.plan_identity, "sha256:plan");
}

#[test]
fn stale_or_invalid_transactions_are_atomic() {
    let mut workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let stale = EditRequest {
        expected_source_revision: 1,
        ..request(&workspace, Vec::new())
    };
    assert_eq!(
        workspace.apply(stale).expect_err("stale rejected").code,
        "CND-PBY-003"
    );
    let before = workspace.source().clone();
    let error = workspace
        .apply(request(
            &workspace,
            vec![EditOperation::ReplaceSource {
                source: "panel nope".to_owned(),
            }],
        ))
        .expect_err("invalid source rejected");
    assert_eq!(error.code, "CND-PBY-004");
    assert_eq!(workspace.source(), &before);
}

#[test]
fn protocol_version_and_unknown_visual_subject_fail_closed() {
    let mut workspace = Workspace::new("tour/hello", SOURCE).expect("source parses");
    let unsupported = EditRequest {
        protocol_version: 2,
        ..request(&workspace, Vec::new())
    };
    assert_eq!(
        workspace
            .apply(unsupported)
            .expect_err("version rejected")
            .code,
        "CND-PBY-001"
    );
    let unknown = request(
        &workspace,
        vec![EditOperation::MoveNode {
            node_id: "missing".to_owned(),
            position: NodePosition { x: 0, y: 0 },
        }],
    );
    assert_eq!(
        workspace
            .apply(unknown)
            .expect_err("unknown node rejected")
            .code,
        "CND-PBY-005"
    );
}

#[test]
fn projection_gaps_require_explicit_resynchronization() {
    let mut projection = ProjectionLog::new("evidence/run-1", 2).expect("bounded retention");
    projection.append(SubjectPath::Logical("greeting".to_owned()));
    projection.append(SubjectPath::Expanded("greeting#0".to_owned()));
    projection.append(SubjectPath::Logical("output".to_owned()));
    assert_eq!(
        projection.observe_from(0),
        vec![ProjectionUpdate::Gap {
            requested: 0,
            earliest_available: 2
        }]
    );
    assert_eq!(projection.observe_from(1).len(), 2);
    assert_eq!(
        ProjectionLog::new("evidence/run-1", 0)
            .expect_err("unbounded retention rejected")
            .code,
        "CND-PBY-006"
    );
}

#[test]
fn fixture_names_each_required_protocol_boundary() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let ids = fixture["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| case["id"].as_str().expect("case id"))
        .collect::<std::collections::BTreeSet<_>>();
    for required in [
        "move-only-preserves-semantic-identity",
        "source-edit-creates-new-semantic-revision",
        "stale-edit-is-atomic-conflict",
        "invalid-source-is-atomic-diagnostic",
        "run-remains-pinned-while-source-changes",
        "logical-and-expanded-subjects-are-distinct",
        "projection-gap-requires-resync",
        "protocol-version-mismatch-fails-closed",
    ] {
        assert!(ids.contains(required), "fixture covers {required}");
    }
}
