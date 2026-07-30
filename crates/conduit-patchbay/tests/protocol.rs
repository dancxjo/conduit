use conduit_patchbay::{
    EditOperation, EditRequest, NodePosition, PATCHBAY_PROTOCOL_V1, PlanSnapshot,
    PoolProjectionInput, ProjectionLog, ProjectionUpdate, RunSnapshot, RunState, SubjectPath,
    Workspace, project_pool, project_supervision,
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
        bindings: Vec::new(),
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

#[test]
fn supervision_projection_retains_exact_origins_actions_rejection_and_gap() {
    use conduit_core::{
        AdmittedSupervisionAction, EvidenceCursor, EvidenceCursorStatus, Id, InstancePath,
        RecoveryBudget, RetryDeclaration, SemanticHash, StopPolicy, SupervisionActionKind,
        SupervisionContract, SupervisionEvidence, SupervisionEvidenceKind, SupervisionFailureMode,
        SupervisionLimits, SupervisionReason, SupervisionScope, TerminalCauseCode, TerminalClass,
        TerminalContext, TerminalObservation, TerminalPhase,
    };
    let actions = [AdmittedSupervisionAction {
        kind: SupervisionActionKind::ActivateDeclaredFallback,
        target: Some(Id("fallback")),
        maximum_uses: 1,
        permits_effect_replay: false,
        preserves_required_guarantees: true,
        requires_new_epoch: true,
    }];
    let contract = SupervisionContract {
        schema_version: 1,
        id: Id("supervision.subject"),
        scope: SupervisionScope::Child,
        subject: InstancePath::new("root/subject").unwrap(),
        handler: InstancePath::new("root/handler").unwrap(),
        members: &[],
        failure_mode: SupervisionFailureMode::FailTogether,
        outer: None,
        actions: &actions,
        limits: SupervisionLimits {
            maximum_observations: 2,
            maximum_decisions: 2,
            maximum_in_flight: 1,
            maximum_cause_depth: 2,
            maximum_nested_depth: 2,
            maximum_handler_ticks: 8,
            maximum_recovery_ticks: 16,
            restart_window_ticks: 8,
            backoff_ticks: 2,
            cooldown_ticks: 2,
            operator_wait_ticks: 8,
            maximum_evidence_events: 8,
            observation_bytes: 256,
            decision_bytes: 64,
            scratch_bytes: 64,
        },
        cleanup: StopPolicy::Abort,
        required_behavior: true,
    };
    let observation = TerminalObservation {
        semantic_subject: contract.subject,
        expanded_subject: InstancePath::new("root/subject-3").unwrap(),
        run: Id("run-1"),
        plan_identity: SemanticHash::from_bytes([7; 32]),
        plan_epoch: 4,
        generation: 3,
        attempt: 2,
        class: TerminalClass::Failed,
        code: TerminalCauseCode::NodeFailed,
        phase: TerminalPhase::Step,
        caused_by: &[],
        retry: RetryDeclaration::RestartOnly,
        context: TerminalContext {
            resource: Some(Id("gpu")),
            host: Some(Id("browser")),
            artifact: Some(Id("worker")),
            ..TerminalContext::default()
        },
        evidence: EvidenceCursor {
            stream: Id("evidence-run-1"),
            sequence: 17,
        },
        budget: RecoveryBudget {
            remaining_observations: 1,
            remaining_decisions: 1,
            remaining_attempts: 0,
            remaining_evidence_events: 2,
            now_tick: 10,
            deadline_tick: 20,
        },
    };
    let evidence = [SupervisionEvidence {
        sequence: 18,
        kind: SupervisionEvidenceKind::DecisionRejected,
        action_index: Some(0),
        reason: Some(SupervisionReason::CandidateEpochRequired),
    }];
    let projection = project_supervision(
        "sha256:source",
        observation,
        contract,
        &evidence,
        EvidenceCursorStatus::Gap { resume_at: 15 },
    );
    assert_eq!(projection.semantic_subject, "root/subject");
    assert_eq!(projection.expanded_subject, "root/subject-3");
    assert_eq!(projection.plan_epoch, 4);
    assert_eq!(projection.evidence_gap_resume_at, Some(15));
    assert_eq!(projection.actions[0].target.as_deref(), Some("fallback"));
    assert!(projection.actions[0].requires_new_epoch);
    assert_eq!(
        projection
            .latest_evidence
            .unwrap()
            .rejection_code
            .as_deref(),
        Some("CND-SUP-013")
    );
}

#[test]
fn pool_projection_retains_plan_run_generation_and_evidence_origins() {
    use conduit_core::{
        InstancePath, PlanResourceBudget, PoolAdmissionDisposition, PoolAdmissionFacts,
        PoolAdmissionPolicy, PoolCleanupPolicy, PoolContract, PoolController, PoolGeneration,
        PoolReservationProfile, PoolSupervisionPolicy, PoolWorkIdentity, SemanticHash,
    };
    let hash = |byte| SemanticHash::from_bytes([byte; 32]);
    let reservation = PoolReservationProfile {
        resources: PlanResourceBudget {
            memory_bytes: 128,
            timers: 2,
            evidence_bytes: 64,
            ..PlanResourceBudget::ZERO
        },
        child_nodes: 2,
        child_cords: 1,
        state_bytes: 32,
        scheduler_slots: 3,
        host_operations: 1,
        cancellation_scopes: 2,
    };
    let contract = PoolContract {
        pool: InstancePath::new("root/pool.workers").unwrap(),
        template_hash: hash(1),
        implementation_set_hash: hash(6),
        maximum_live: 1,
        maximum_queued: 0,
        admission: PoolAdmissionPolicy::Reject,
        supervision: PoolSupervisionPolicy::Isolate,
        cleanup: PoolCleanupPolicy::Abort,
        deadline_ticks: 100,
        idle_timeout_ticks: 20,
        cleanup_ticks: 5,
        reservation,
        total_reservation: reservation.checked_mul(3).unwrap(),
        maximum_evidence_events: 16,
    };
    let generation = PoolGeneration {
        plan: hash(2),
        epoch: 4,
        generation: 3,
        template_hash: hash(1),
    };
    let mut runtime = PoolController::<1, 16>::new(contract, generation).unwrap();
    let PoolAdmissionDisposition::Started { slot } = runtime
        .offer(
            PoolWorkIdentity {
                request: hash(3),
                work_unit: hash(4),
                correlation: hash(5),
            },
            PoolAdmissionFacts {
                authority_granted: true,
                sensitivity_allowed: true,
                template_hash: hash(1),
                implementation_set_hash: hash(6),
                available: reservation,
            },
            10,
        )
        .unwrap()
    else {
        panic!("pool starts")
    };
    runtime.mark_running(slot, 11).unwrap();
    let projection = project_pool(PoolProjectionInput {
        source_semantic_hash: "sha256:source",
        plan_identity: hash(2),
        plan_epoch: 4,
        run_id: "run-1",
        evidence_stream_id: "evidence/run-1",
        generation,
        generation_identity: runtime.generation_identity(),
        contract,
        population: runtime.population(),
        evidence: runtime.evidence(),
    });
    assert_eq!(projection.source_semantic_hash, "sha256:source");
    assert_eq!(projection.run_id, "run-1");
    assert_eq!(projection.pool, "root/pool.workers");
    assert_eq!(projection.generation, 3);
    assert_eq!(projection.live, 1);
    assert_eq!(projection.evidence_cursor, 2);
    assert_eq!(projection.latest_evidence.unwrap().to, "running");
}
