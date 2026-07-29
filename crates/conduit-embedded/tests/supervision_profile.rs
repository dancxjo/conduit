use core::mem::size_of;

use conduit_core::{
    ActionUsage, AdmittedSupervisionAction, EvidenceCursor, Id, InstancePath, RecoveryBudget,
    RetryDeclaration, SUPERVISION_CONTRACT_VERSION, SemanticHash, StopPolicy,
    SupervisionActionKind, SupervisionContract, SupervisionDecision, SupervisionEvidenceKind,
    SupervisionFailureMode, SupervisionHostProfile, SupervisionLimits, SupervisionReason,
    SupervisionScope, SupervisionState, TerminalCauseCode, TerminalClass, TerminalContext,
    TerminalObservation, TerminalPhase,
};

const ACTIONS: [AdmittedSupervisionAction<'static>; 4] = [
    AdmittedSupervisionAction {
        kind: SupervisionActionKind::Propagate,
        target: None,
        maximum_uses: 1,
        permits_effect_replay: false,
        preserves_required_guarantees: true,
        requires_new_epoch: false,
    },
    AdmittedSupervisionAction {
        kind: SupervisionActionKind::StopScope,
        target: None,
        maximum_uses: 1,
        permits_effect_replay: false,
        preserves_required_guarantees: true,
        requires_new_epoch: false,
    },
    AdmittedSupervisionAction {
        kind: SupervisionActionKind::RestartSame,
        target: None,
        maximum_uses: 2,
        permits_effect_replay: false,
        preserves_required_guarantees: true,
        requires_new_epoch: false,
    },
    AdmittedSupervisionAction {
        kind: SupervisionActionKind::ActivateDeclaredFallback,
        target: Some(Id("fallback")),
        maximum_uses: 1,
        permits_effect_replay: false,
        preserves_required_guarantees: true,
        requires_new_epoch: false,
    },
];

fn contract() -> SupervisionContract<'static> {
    SupervisionContract {
        schema_version: SUPERVISION_CONTRACT_VERSION,
        id: Id("embedded-supervisor"),
        scope: SupervisionScope::Child,
        subject: InstancePath::new("root/subject").unwrap(),
        handler: InstancePath::new("root/handler").unwrap(),
        members: &[],
        failure_mode: SupervisionFailureMode::FailTogether,
        outer: None,
        actions: &ACTIONS,
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
    }
}

fn observation() -> TerminalObservation<'static> {
    TerminalObservation {
        semantic_subject: InstancePath::new("root/subject").unwrap(),
        expanded_subject: InstancePath::new("root/subject").unwrap(),
        run: Id("run"),
        plan_identity: SemanticHash::from_bytes([1; 32]),
        plan_epoch: 1,
        generation: 1,
        attempt: 1,
        class: TerminalClass::Failed,
        code: TerminalCauseCode::NodeFailed,
        phase: TerminalPhase::Step,
        caused_by: &[],
        retry: RetryDeclaration::RestartOnly,
        context: TerminalContext::default(),
        evidence: EvidenceCursor {
            stream: Id("evidence"),
            sequence: 0,
        },
        budget: RecoveryBudget {
            remaining_observations: 2,
            remaining_decisions: 2,
            remaining_attempts: 1,
            remaining_evidence_events: 8,
            now_tick: 1,
            deadline_tick: 10,
        },
    }
}

#[test]
fn constrained_profile_uses_only_caller_owned_fixed_storage() {
    let contract = contract();
    let observation = observation();
    let mut state = SupervisionState::new();
    let mut usages = ACTIONS.map(|action| ActionUsage {
        kind: action.kind,
        target: action.target,
        uses: 0,
    });

    let admission = state.admit_observation(contract, observation).unwrap();
    assert_eq!(
        admission.observed.kind,
        SupervisionEvidenceKind::TerminalObserved
    );
    let outcome = state
        .apply_decision(
            contract,
            SupervisionHostProfile::Constrained,
            observation,
            SupervisionDecision {
                kind: SupervisionActionKind::RestartSame,
                target: None,
            },
            &mut usages,
        )
        .unwrap();
    assert_eq!(outcome.next_attempt, Some(2));
    assert_eq!(outcome.timing.attempt_not_before_tick, Some(3));
    assert!(size_of::<TerminalObservation<'static>>() <= 512);
    assert!(size_of::<SupervisionDecision<'static>>() <= 64);
}

#[test]
fn constrained_profile_reports_richer_action_as_unsupported() {
    let contract = contract();
    let observation = observation();
    let mut state = SupervisionState::new();
    let mut usages = ACTIONS.map(|action| ActionUsage {
        kind: action.kind,
        target: action.target,
        uses: 0,
    });
    state.admit_observation(contract, observation).unwrap();
    assert_eq!(
        state.apply_decision(
            contract,
            SupervisionHostProfile::Constrained,
            observation,
            SupervisionDecision {
                kind: SupervisionActionKind::ActivateDeclaredFallback,
                target: Some(Id("fallback")),
            },
            &mut usages,
        ),
        Err(SupervisionReason::UnsupportedProfile)
    );
}
