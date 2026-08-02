use conduit_core::{DescriptorRef, Id, SemanticHash, TypeContractRef};
use conduit_std::control::*;
use sha2::{Digest as _, Sha256};

const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

const TEXT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};

const fn descriptor(kind: &'static str, byte: u8) -> DescriptorRef<'static> {
    DescriptorRef {
        kind: Id(kind),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn request_reply() -> RequestReplyContract<'static> {
    RequestReplyContract {
        id: Id("control/test-request-reply"),
        schema_version: 0,
        request: TEXT,
        reply: TEXT,
        domain_error: TEXT,
        limits: RequestReplyLimits {
            maximum_in_flight: 2,
            maximum_request_bytes: 64,
            maximum_reply_bytes: 64,
            maximum_domain_error_bytes: 32,
            maximum_deadline_ticks: 100,
            maximum_retries: 1,
            maximum_replay_outcomes: 2,
            maximum_timers: 2,
            maximum_evidence_events: 32,
            maximum_work_per_step: 2,
        },
        clock: descriptor("control/clock", 1),
        correlation: descriptor("control/correlation", 2),
        cancellation: descriptor("control/cancellation", 3),
        idempotency: descriptor("control/idempotency", 4),
    }
}

fn action(policy: FeedbackPressurePolicy, transition: TransitionPolicy) -> ActionContract<'static> {
    ActionContract {
        id: Id("control/test-action"),
        schema_version: 0,
        goal: TEXT,
        feedback: TEXT,
        result: TEXT,
        domain_failure: TEXT,
        limits: ActionLimits {
            maximum_concurrent_goals: 1,
            maximum_queued_admissions: 2,
            maximum_goal_bytes: 64,
            maximum_result_bytes: 64,
            maximum_domain_failure_bytes: 32,
            maximum_feedback_items_per_goal: 2,
            maximum_feedback_bytes_per_goal: 8,
            maximum_replay_outcomes: 2,
            maximum_deadline_ticks: 100,
            maximum_retries_per_goal: 1,
            maximum_cancellations: 4,
            maximum_timers: 3,
            maximum_evidence_events: 64,
            maximum_work_per_step: 2,
        },
        feedback_pressure: policy,
        transition_policy: transition,
        clock: descriptor("control/clock", 1),
        correlation: descriptor("control/correlation", 2),
        idempotency: descriptor("control/idempotency", 3),
        cancellation: descriptor("control/cancellation", 11),
        admission_authority: descriptor("control/admission", 4),
        workload_admission: descriptor("control/workload", 5),
        placement: descriptor("control/placement", 6),
        resource_commit_cleanup: descriptor("control/resource-commit-cleanup", 7),
        transition: descriptor("control/transition", 8),
        inhibit: Some(descriptor("control/inhibit", 9)),
        checkpoint: (transition == TransitionPolicy::CompatibleCheckpointHandoff)
            .then_some(descriptor("control/checkpoint", 10)),
    }
}

fn identity(subject: u64) -> ControlIdentity {
    ControlIdentity {
        subject,
        attempt: 1,
        correlation: 100 + subject,
        idempotency: 200 + subject,
    }
}

fn admitted(contract: ActionContract<'_>) -> ActionAdmission {
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
fn published_catalog_keeps_independent_domain_types_and_exact_plan_fields() {
    assert_eq!(STANDARD_CONTROL_CATALOG.len(), 2);
    let request_reply = control_composite_contract("conduit.std/control/request-reply").unwrap();
    assert_eq!(request_reply.schema_version, 0);
    assert_eq!(request_reply.kind, ControlCompositeKind::RequestReply);
    assert_eq!(
        request_reply
            .type_parameters
            .iter()
            .map(|parameter| parameter.id.as_str())
            .collect::<Vec<_>>(),
        ["request", "reply", "domain-error"]
    );
    assert!(request_reply.plan_fields.iter().any(|field| {
        field.id.as_str() == "maximum-in-flight" && field.kind == ControlPlanFieldKind::Limit
    }));

    let action = control_composite_contract("conduit.std/control/cancellable-action").unwrap();
    assert_eq!(action.schema_version, 0);
    assert_eq!(action.kind, ControlCompositeKind::CancellableAction);
    assert_eq!(
        action
            .type_parameters
            .iter()
            .map(|parameter| parameter.id.as_str())
            .collect::<Vec<_>>(),
        ["goal", "feedback", "result", "domain-failure"]
    );
    for required in [
        "admission-authority",
        "workload-admission",
        "placement",
        "resource-commit-cleanup",
        "cancellation",
        "inhibit",
        "checkpoint",
        "feedback-pressure",
        "transition-policy",
        "maximum-concurrent-goals",
        "maximum-queued-admissions",
        "maximum-result-bytes",
        "maximum-domain-failure-bytes",
        "maximum-feedback-items-per-goal",
        "maximum-replay-outcomes",
        "maximum-deadline-ticks",
        "maximum-retries-per-goal",
        "maximum-cancellations",
        "maximum-timers",
        "maximum-evidence-events",
        "maximum-work-per-step",
    ] {
        assert!(
            action
                .plan_fields
                .iter()
                .any(|field| field.id.as_str() == required),
            "missing exact-plan control field {required}"
        );
    }
}

#[test]
fn language_neutral_package_and_fixture_cover_the_required_cross_host_contract() {
    let package: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contract-packages/conduit-std-control.json"
    ))
    .unwrap();
    assert_eq!(package["schema"], "conduit.contract-package");
    assert_eq!(package["draft"], 0);
    let exports = package["exports"].as_array().unwrap();
    assert_eq!(exports.len(), STANDARD_CONTROL_CATALOG.len());
    for export in exports {
        let bytes = serde_json::to_vec(&export["descriptor"]).unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        assert_eq!(export["descriptor_hash"], digest);
        let id = export["canonical_id"].as_str().unwrap();
        assert!(control_composite_contract(id).is_some());
        assert_eq!(export["kind"], "composite");
        assert_eq!(export["successor"], serde_json::Value::Null);
        assert_eq!(export["deprecated"], false);
    }

    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../conformance/c4/bounded-control.json")).unwrap();
    assert_eq!(fixture["schema"], "conduit.bounded-control-fixtures");
    assert_eq!(fixture["schema_version"], 0);
    let profiles = fixture["profiles"].as_array().unwrap();
    for required in [
        "rust-hosted",
        "browser-wasm",
        "process-boundary",
        "python-host",
        "allocator-free-firmware",
    ] {
        assert!(profiles.iter().any(|profile| profile == required));
    }
    let cases = fixture["cases"].as_array().unwrap();
    let ids = cases
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    for required in [
        "request-reply-success",
        "request-reply-timeout",
        "request-reply-duplicate-idempotent-replay",
        "request-reply-exhaustion",
        "action-success-with-several-feedback-items",
        "action-rejected-before-work",
        "action-timeout",
        "action-cancel-before-admission",
        "action-cancel-after-admission",
        "action-domain-failure",
        "action-duplicate-idempotent-submission",
        "action-slow-feedback-consumer-blocks",
        "action-concurrency-saturated",
        "action-admission-queue-saturated",
        "action-provider-loss-terminal",
        "action-plan-transition-discontinuity",
        "action-compatible-checkpoint-handoff",
        "semantic-import-host-unavailable",
        "robotics-safety-inhibit-denial",
        "robotics-authority-denial",
    ] {
        assert!(ids.contains(&required), "fixture covers {required}");
    }
    assert_eq!(
        fixture["evidence_contract"]["feedback_payload_durable"],
        false
    );
}

#[test]
fn request_reply_is_one_correlated_finite_outcome_without_callback_state() {
    let contract = request_reply();
    validate_request_reply_contract(&contract).unwrap();
    let mut reference = ReferenceRequestReply::new(contract).unwrap();
    let first = identity(1);
    assert_eq!(
        reference.request(first, 8, 0, 10).unwrap(),
        RequestReplySubmission::Admitted
    );
    assert_eq!(
        reference.request(first, 8, 0, 10).unwrap(),
        RequestReplySubmission::Duplicate(RequestReplyState::InFlight)
    );
    assert_eq!(
        reference.reply(1, 12).unwrap(),
        RequestReplyState::Terminal(RequestReplyOutcome::Reply)
    );
    assert_eq!(
        reference.request(first, 8, 0, 10).unwrap(),
        RequestReplySubmission::Duplicate(RequestReplyState::Terminal(RequestReplyOutcome::Reply))
    );

    reference.request(identity(2), 8, 0, 5).unwrap();
    assert_eq!(reference.advance(5).unwrap(), 1);
    reference.request(identity(3), 8, 0, 10).unwrap();
    assert_eq!(
        reference.cancel(3).unwrap(),
        RequestReplyState::Terminal(RequestReplyOutcome::Cancelled)
    );
    assert!(reference.evidence().len() <= contract.limits.maximum_evidence_events.into());
}

#[test]
fn request_reply_capacity_retry_and_correlation_are_exact() {
    let contract = request_reply();
    let mut reference = ReferenceRequestReply::new(contract).unwrap();
    reference.request(identity(1), 8, 0, 10).unwrap();
    reference.request(identity(2), 8, 0, 10).unwrap();
    let exhausted = identity(3);
    assert_eq!(
        reference.request(exhausted, 8, 0, 10).unwrap(),
        RequestReplySubmission::Exhausted
    );
    assert_eq!(
        reference.request(exhausted, 8, 0, 10).unwrap(),
        RequestReplySubmission::Duplicate(RequestReplyState::Terminal(
            RequestReplyOutcome::Exhausted
        ))
    );
    assert_eq!(reference.retry(1, 1, 11).unwrap().attempt, 2);
    assert_eq!(reference.retry(1, 2, 12), Err(ControlError::RetryExhausted));

    let mut conflict = identity(9);
    conflict.idempotency = identity(1).idempotency;
    assert_eq!(
        reference.request(conflict, 8, 0, 10),
        Err(ControlError::CorrelationConflict)
    );
}

#[test]
fn accepted_feedback_success_remains_distinct_from_rejection_and_failure() {
    let contract = action(
        FeedbackPressurePolicy::BlockProducer,
        TransitionPolicy::TerminalFailure,
    );
    let admission = admitted(contract);
    let mut reference = ReferenceAction::new(contract).unwrap();
    assert_eq!(
        reference.submit(identity(1), 8, 0, 20).unwrap(),
        ActionSubmission::Queued
    );
    assert_eq!(
        reference.admit(1, admission).unwrap(),
        ActionState::Accepted
    );
    assert_eq!(
        reference.feedback(1, 3).unwrap(),
        FeedbackDisposition::Retained
    );
    assert_eq!(
        reference.feedback(1, 3).unwrap(),
        FeedbackDisposition::Retained
    );
    assert_eq!(
        reference.feedback(1, 3).unwrap(),
        FeedbackDisposition::Blocked
    );
    assert_eq!(
        reference.result(1, 12).unwrap(),
        ActionState::Terminal(ActionOutcome::Result)
    );

    reference.submit(identity(2), 8, 0, 20).unwrap();
    let mut denied = admission;
    denied.domain_policy_allows = false;
    assert_eq!(
        reference.admit(2, denied).unwrap(),
        ActionState::Terminal(ActionOutcome::Rejected(RejectionReason::DomainPolicy))
    );
    reference.submit(identity(3), 8, 0, 20).unwrap();
    reference.admit(3, admission).unwrap();
    assert_eq!(
        reference.fail(3, 8).unwrap(),
        ActionState::Terminal(ActionOutcome::Failed)
    );

    assert!(
        reference
            .evidence()
            .iter()
            .flatten()
            .all(|event| event.feedback_bytes <= 3)
    );
    let terminal_bytes = reference
        .evidence()
        .iter()
        .flatten()
        .filter(|event| {
            matches!(
                event.kind,
                ActionEvidenceKind::Result | ActionEvidenceKind::Failed
            )
        })
        .map(|event| event.terminal_bytes)
        .collect::<Vec<_>>();
    assert_eq!(terminal_bytes, [12, 8]);

    reference.submit(identity(4), 8, 0, 20).unwrap();
    reference.admit(4, admission).unwrap();
    assert_eq!(reference.result(4, 65), Err(ControlError::ResultTooLarge));
    assert_eq!(
        reference.fail(4, 33),
        Err(ControlError::DomainFailureTooLarge)
    );
    assert_eq!(reference.state(4), Some(ActionState::Accepted));
}

#[test]
fn cancellation_before_and_after_admission_have_different_outcomes() {
    let contract = action(
        FeedbackPressurePolicy::BlockProducer,
        TransitionPolicy::TerminalFailure,
    );
    let admission = admitted(contract);
    let mut reference = ReferenceAction::new(contract).unwrap();
    reference.submit(identity(1), 8, 0, 20).unwrap();
    assert_eq!(
        reference.cancel(1).unwrap(),
        ActionState::Terminal(ActionOutcome::WithdrawnBeforeAdmission)
    );
    reference.submit(identity(2), 8, 0, 20).unwrap();
    reference.admit(2, admission).unwrap();
    assert_eq!(
        reference.cancel(2).unwrap(),
        ActionState::Terminal(ActionOutcome::Cancelled)
    );
    let causal = reference
        .evidence()
        .iter()
        .flatten()
        .filter(|event| {
            matches!(
                event.kind,
                ActionEvidenceKind::GoalCancelled | ActionEvidenceKind::GoalWithdrawn
            )
        })
        .collect::<Vec<_>>();
    assert!(causal.iter().all(|event| event.causal_sequence.is_some()));
}

#[test]
fn admission_consumes_authority_workload_placement_cleanup_and_inhibit_proofs() {
    let contract = action(
        FeedbackPressurePolicy::BlockProducer,
        TransitionPolicy::TerminalFailure,
    );
    for (missing, expected) in [
        ("authority", RejectionReason::Authority),
        ("workload", RejectionReason::WorkloadAdmission),
        ("placement", RejectionReason::Placement),
        ("cleanup", RejectionReason::ResourceCommitCleanup),
        ("inhibit", RejectionReason::Inhibited),
    ] {
        let mut reference = ReferenceAction::new(contract).unwrap();
        reference.submit(identity(1), 8, 0, 20).unwrap();
        let mut proof = admitted(contract);
        match missing {
            "authority" => proof.authority = None,
            "workload" => proof.workload = None,
            "placement" => proof.placement = None,
            "cleanup" => proof.resource_commit_cleanup = None,
            "inhibit" => proof.inhibit = None,
            _ => unreachable!(),
        }
        assert_eq!(
            reference.admit(1, proof).unwrap(),
            ActionState::Terminal(ActionOutcome::Rejected(expected)),
            "{missing}"
        );
    }
}

#[test]
fn queue_concurrency_deadline_retry_and_idempotency_bounds_are_enforced() {
    let contract = action(
        FeedbackPressurePolicy::BlockProducer,
        TransitionPolicy::TerminalFailure,
    );
    let admission = admitted(contract);
    let mut reference = ReferenceAction::new(contract).unwrap();
    reference.submit(identity(1), 8, 0, 10).unwrap();
    reference.submit(identity(2), 8, 0, 10).unwrap();
    let exhausted = identity(3);
    assert_eq!(
        reference.submit(exhausted, 8, 0, 10).unwrap(),
        ActionSubmission::Exhausted
    );
    assert_eq!(
        reference.submit(exhausted, 8, 0, 10).unwrap(),
        ActionSubmission::Duplicate(ActionState::Terminal(ActionOutcome::Rejected(
            RejectionReason::WorkloadAdmission
        )))
    );
    reference.admit(1, admission).unwrap();
    assert_eq!(
        reference.admit(2, admission).unwrap(),
        ActionState::Terminal(ActionOutcome::Rejected(
            RejectionReason::ConcurrentGoalLimit
        ))
    );
    assert_eq!(reference.retry_attempt(1).unwrap().attempt, 2);
    assert_eq!(
        reference.retry_attempt(1),
        Err(ControlError::RetryExhausted)
    );
    assert_eq!(reference.advance(10).unwrap(), 1);

    let duplicate = ControlIdentity {
        attempt: 1,
        ..identity(1)
    };
    assert_eq!(
        reference.submit(duplicate, 8, 0, 10).unwrap(),
        ActionSubmission::Duplicate(ActionState::Terminal(ActionOutcome::DeadlineExhausted))
    );
}

#[test]
fn every_feedback_pressure_policy_is_finite_and_payload_free_in_evidence() {
    for (policy, expected) in [
        (
            FeedbackPressurePolicy::BlockProducer,
            FeedbackDisposition::Blocked,
        ),
        (
            FeedbackPressurePolicy::DropOldest,
            FeedbackDisposition::DroppedOldest,
        ),
        (
            FeedbackPressurePolicy::CoalesceLatest,
            FeedbackDisposition::CoalescedLatest,
        ),
    ] {
        let contract = action(policy, TransitionPolicy::TerminalFailure);
        let mut reference = ReferenceAction::new(contract).unwrap();
        reference.submit(identity(1), 8, 0, 20).unwrap();
        reference.admit(1, admitted(contract)).unwrap();
        reference.feedback(1, 4).unwrap();
        reference.feedback(1, 4).unwrap();
        assert_eq!(reference.feedback(1, 4).unwrap(), expected);
        let snapshot = reference.snapshot(1).unwrap();
        match policy {
            FeedbackPressurePolicy::BlockProducer => {
                assert_eq!((snapshot.feedback_items, snapshot.feedback_bytes), (2, 8));
            }
            FeedbackPressurePolicy::DropOldest => {
                assert_eq!((snapshot.feedback_items, snapshot.feedback_bytes), (2, 8));
                reference.consume_feedback(1, 4).unwrap();
                assert_eq!(reference.snapshot(1).unwrap().feedback_items, 1);
            }
            FeedbackPressurePolicy::CoalesceLatest => {
                assert_eq!((snapshot.feedback_items, snapshot.feedback_bytes), (1, 4));
            }
        }
        let latest = reference.evidence().iter().flatten().last().unwrap();
        assert_eq!(latest.feedback_disposition, Some(expected));
        assert!(
            reference
                .evidence()
                .iter()
                .flatten()
                .all(|event| event.feedback_bytes <= 4)
        );
    }
}

#[test]
fn provider_loss_and_plan_transition_follow_the_declared_policy() {
    for (policy, expected) in [
        (
            TransitionPolicy::TerminalFailure,
            ActionState::Terminal(ActionOutcome::Failed),
        ),
        (
            TransitionPolicy::ExplicitDiscontinuity,
            ActionState::Terminal(ActionOutcome::Discontinued),
        ),
        (
            TransitionPolicy::CompatibleCheckpointHandoff,
            ActionState::Accepted,
        ),
    ] {
        let contract = action(FeedbackPressurePolicy::BlockProducer, policy);
        let mut reference = ReferenceAction::new(contract).unwrap();
        reference.submit(identity(1), 8, 0, 20).unwrap();
        reference.admit(1, admitted(contract)).unwrap();
        assert_eq!(
            reference
                .interrupt(
                    1,
                    ActionInterruption::ProviderLoss,
                    Some(contract.transition.semantic_hash),
                    contract.checkpoint.map(|value| value.semantic_hash),
                )
                .unwrap(),
            expected
        );
    }
}

#[test]
fn all_plan_visible_limits_are_positive_and_reference_bounded() {
    let action = action(
        FeedbackPressurePolicy::BlockProducer,
        TransitionPolicy::TerminalFailure,
    );
    validate_action_contract(&action).unwrap();
    let request_reply = request_reply();
    validate_request_reply_contract(&request_reply).unwrap();

    let mut invalid = action;
    invalid.limits.maximum_feedback_items_per_goal = 0;
    assert_eq!(
        validate_action_contract(&invalid),
        Err(ControlError::Unbounded)
    );
    let mut invalid = request_reply;
    invalid.limits.maximum_timers = 0;
    assert_eq!(
        validate_request_reply_contract(&invalid),
        Err(ControlError::Unbounded)
    );
    let mut invalid = action;
    invalid.limits.maximum_feedback_items_per_goal =
        (MAXIMUM_REFERENCE_FEEDBACK_ITEMS_PER_GOAL + 1) as u16;
    assert_eq!(
        validate_action_contract(&invalid),
        Err(ControlError::ReferenceCapacityExceeded)
    );

    let mut reference = ReferenceRequestReply::new(request_reply).unwrap();
    reference.request(identity(40), 8, 0, 10).unwrap();
    assert_eq!(
        reference.domain_error(40, 33),
        Err(ControlError::DomainFailureTooLarge)
    );
    assert_eq!(
        reference.domain_error(40, 32).unwrap(),
        RequestReplyState::Terminal(RequestReplyOutcome::DomainError)
    );

    let mut bounded_request = request_reply;
    bounded_request.limits.maximum_work_per_step = 1;
    let mut reference = ReferenceRequestReply::new(bounded_request).unwrap();
    reference.request(identity(50), 8, 0, 10).unwrap();
    reference.request(identity(51), 8, 0, 10).unwrap();
    assert_eq!(reference.advance(10).unwrap(), 1);
    assert_eq!(reference.advance(10).unwrap(), 1);

    let mut bounded_action = action;
    bounded_action.limits.maximum_concurrent_goals = 2;
    bounded_action.limits.maximum_work_per_step = 1;
    bounded_action.limits.maximum_timers = 4;
    let mut reference = ReferenceAction::new(bounded_action).unwrap();
    reference.submit(identity(60), 8, 0, 10).unwrap();
    reference.submit(identity(61), 8, 0, 10).unwrap();
    assert_eq!(reference.advance(10).unwrap(), 1);
    assert_eq!(reference.advance(10).unwrap(), 1);
}
