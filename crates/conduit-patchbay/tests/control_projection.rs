use conduit_core::{DescriptorRef, Id, SemanticHash, TypeContractRef};
use conduit_patchbay::{
    CancellableActionProjectionInput, RequestReplyProjectionInput, project_cancellable_action,
    project_request_reply,
};
use conduit_std::control::{
    ActionAdmission, ActionContract, ActionLimits, ActionState, ControlIdentity,
    FeedbackDisposition, FeedbackPressurePolicy, ReferenceAction, ReferenceRequestReply,
    RequestReplyContract, RequestReplyLimits, TransitionPolicy,
};

const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("test/control-value"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0x41; 32]),
};

const fn descriptor(kind: &'static str, byte: u8) -> DescriptorRef<'static> {
    DescriptorRef {
        kind: Id(kind),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn identity(subject: u64) -> ControlIdentity {
    ControlIdentity {
        subject,
        attempt: 1,
        correlation: subject + 100,
        idempotency: subject + 200,
    }
}

fn request_reply_contract() -> RequestReplyContract<'static> {
    RequestReplyContract {
        id: Id("test/request-reply"),
        schema_version: 0,
        request: TYPE,
        reply: TYPE,
        domain_error: TYPE,
        limits: RequestReplyLimits {
            maximum_in_flight: 1,
            maximum_request_bytes: 64,
            maximum_reply_bytes: 64,
            maximum_domain_error_bytes: 32,
            maximum_deadline_ticks: 100,
            maximum_retries: 1,
            maximum_replay_outcomes: 1,
            maximum_timers: 1,
            maximum_evidence_events: 16,
            maximum_work_per_step: 1,
        },
        clock: descriptor("test/clock", 1),
        correlation: descriptor("test/correlation", 2),
        cancellation: descriptor("test/cancellation", 3),
        idempotency: descriptor("test/idempotency", 4),
    }
}

fn action_contract() -> ActionContract<'static> {
    ActionContract {
        id: Id("test/cancellable-action"),
        schema_version: 0,
        goal: TYPE,
        feedback: TYPE,
        result: TYPE,
        domain_failure: TYPE,
        limits: ActionLimits {
            maximum_concurrent_goals: 1,
            maximum_queued_admissions: 1,
            maximum_goal_bytes: 64,
            maximum_result_bytes: 64,
            maximum_domain_failure_bytes: 32,
            maximum_feedback_items_per_goal: 2,
            maximum_feedback_bytes_per_goal: 12,
            maximum_replay_outcomes: 1,
            maximum_deadline_ticks: 100,
            maximum_retries_per_goal: 1,
            maximum_cancellations: 2,
            maximum_timers: 2,
            maximum_evidence_events: 32,
            maximum_work_per_step: 1,
        },
        feedback_pressure: FeedbackPressurePolicy::DropOldest,
        transition_policy: TransitionPolicy::ExplicitDiscontinuity,
        clock: descriptor("test/clock", 1),
        correlation: descriptor("test/correlation", 2),
        idempotency: descriptor("test/idempotency", 3),
        cancellation: descriptor("test/cancellation", 4),
        admission_authority: descriptor("test/admission", 5),
        workload_admission: descriptor("test/workload", 6),
        placement: descriptor("test/placement", 7),
        resource_commit_cleanup: descriptor("test/resource-commit-cleanup", 8),
        transition: descriptor("test/transition", 9),
        inhibit: Some(descriptor("test/inhibit", 10)),
        checkpoint: Some(descriptor("test/checkpoint", 11)),
    }
}

fn admission(contract: ActionContract<'_>) -> ActionAdmission {
    ActionAdmission {
        authority: Some(contract.admission_authority.semantic_hash),
        workload: Some(contract.workload_admission.semantic_hash),
        placement: Some(contract.placement.semantic_hash),
        resource_commit_cleanup: Some(contract.resource_commit_cleanup.semantic_hash),
        inhibit: contract.inhibit.map(|value| value.semantic_hash),
        domain_policy_allows: true,
    }
}

#[test]
fn request_reply_projection_uses_exact_state_plan_run_and_evidence() {
    let contract = request_reply_contract();
    let mut reference = ReferenceRequestReply::new(contract).unwrap();
    reference.request(identity(7), 8, 0, 20).unwrap();
    reference.reply(7, 12).unwrap();

    let projection = project_request_reply(RequestReplyProjectionInput {
        source_semantic_hash: "sha256:source",
        plan_identity: hash(0x51),
        plan_epoch: 3,
        run_id: "run-7",
        evidence_stream_id: "evidence-7",
        contract,
        snapshot: reference.snapshot(7).unwrap(),
        evidence: reference.evidence(),
        provider_observation: Some(descriptor("test/provider-observation", 0x52)),
    });

    assert_eq!(projection.source_semantic_hash, "sha256:source");
    assert_eq!(projection.plan_identity, hash(0x51).to_string());
    assert_eq!(projection.run_id, "run-7");
    assert_eq!(projection.evidence_stream_id, "evidence-7");
    assert_eq!(projection.subject, 7);
    assert_eq!(projection.correlation, 107);
    assert_eq!(projection.terminal_outcome.as_deref(), Some("reply"));
    assert_eq!(
        projection.latest_evidence.as_ref().unwrap().payload_bytes,
        12
    );
    assert_eq!(
        projection.provider_observation.as_ref().unwrap().id,
        "test/provider-observation"
    );
    let json = serde_json::to_value(&projection).unwrap();
    for field in conduit_patchbay::REQUEST_REPLY_PATCHBAY_FIELDS {
        assert!(
            has_field_path(&json, field),
            "request/reply projects {field}"
        );
    }
}

#[test]
fn action_projection_reports_real_pressure_and_causal_terminal_evidence() {
    let contract = action_contract();
    let mut reference = ReferenceAction::new(contract).unwrap();
    reference.submit(identity(9), 8, 0, 20).unwrap();
    assert_eq!(
        reference.admit(9, admission(contract)).unwrap(),
        ActionState::Accepted
    );
    reference.feedback(9, 3).unwrap();
    reference.feedback(9, 4).unwrap();
    assert_eq!(
        reference.feedback(9, 5).unwrap(),
        FeedbackDisposition::DroppedOldest
    );

    let pressure = project_cancellable_action(CancellableActionProjectionInput {
        source_semantic_hash: "sha256:source",
        plan_identity: hash(0x61),
        plan_epoch: 4,
        run_id: "run-9",
        evidence_stream_id: "evidence-9",
        contract,
        snapshot: reference.snapshot(9).unwrap(),
        cancellations: reference.cancellations(),
        evidence: reference.evidence(),
        provider_observation: None,
    });
    assert_eq!(pressure.state, "accepted");
    assert_eq!(pressure.retained_feedback_items, 2);
    assert_eq!(pressure.retained_feedback_bytes, 9);
    assert_eq!(pressure.feedback_pressure, "drop-oldest");
    assert_eq!(pressure.provider_observation, None);
    let latest = pressure.latest_evidence.as_ref().unwrap();
    assert_eq!(
        latest.feedback_disposition.as_deref(),
        Some("dropped-oldest")
    );
    assert_eq!(latest.feedback_items_affected, 1);
    let json = serde_json::to_value(&pressure).unwrap();
    for field in conduit_patchbay::CANCELLABLE_ACTION_PATCHBAY_FIELDS {
        assert!(has_field_path(&json, field), "action projects {field}");
    }

    reference.cancel(9).unwrap();
    let terminal = project_cancellable_action(CancellableActionProjectionInput {
        source_semantic_hash: "sha256:source",
        plan_identity: hash(0x61),
        plan_epoch: 4,
        run_id: "run-9",
        evidence_stream_id: "evidence-9",
        contract,
        snapshot: reference.snapshot(9).unwrap(),
        cancellations: reference.cancellations(),
        evidence: reference.evidence(),
        provider_observation: None,
    });
    assert_eq!(terminal.state, "terminal");
    assert_eq!(terminal.terminal_outcome.as_deref(), Some("cancelled"));
    assert_eq!(terminal.retained_feedback_items, 0);
    assert_eq!(terminal.cancellations, 1);
    assert!(terminal.latest_evidence.unwrap().causal_sequence.is_some());
}

fn has_field_path(value: &serde_json::Value, path: &str) -> bool {
    path.split('.')
        .try_fold(value, |current, segment| current.get(segment))
        .is_some()
}
