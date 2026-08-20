use conduit_ai::{
    ModelEffectProposal, ModelFollowUpTimingProposal, ModelResultProvenance,
    ProposalDecisionOutcome, ProposalGate, ProposalRefusal,
};
use conduit_core::{
    elapsed_trigger_window, BootId, HostId, KindId, MissedOccurrencePolicy, MonotonicClockIdentity,
    MonotonicDuration, MonotonicInstant, OccurrenceInstant, PlanId, RecurrenceOccurrence,
    ScheduledIntent, SignId, SuspendBehavior, TemporalScale, TriggerProfile,
};

#[test]
fn model_follow_up_timing_is_proposal_info_not_effect_authority() {
    let clock = MonotonicClockIdentity::new(
        HostId::from("host/model"),
        BootId::from("boot/model"),
        "std/monotonic@1".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let at = MonotonicInstant::new(1_000, clock).unwrap();
    let effect = ModelEffectProposal {
        proposal_id: "proposal/follow-up-effect".into(),
        plan_id: PlanId::from("plan/current"),
        operation_kind: KindId::from("process/run-bounded"),
        canonical_arguments: vec![1],
        rationale: "follow up after the bounded delay".into(),
        evidence: vec![SignId::from("sign/source/0")],
    };
    let proposal = ModelFollowUpTimingProposal {
        identity: "proposal/follow-up-time".into(),
        provenance: ModelResultProvenance::ModelDerived,
        proposed: ScheduledIntent {
            identity: "scheduled/follow-up#0".into(),
            occurrence: RecurrenceOccurrence {
                identity: "recurrence/follow-up/occurrence/0".into(),
                recurrence_identity: "recurrence/follow-up".into(),
                ordinal: 0,
                at: OccurrenceInstant::Monotonic(at.clone()),
            },
            trigger: TriggerProfile::Elapsed(
                elapsed_trigger_window(
                    at,
                    MonotonicDuration::new(100, TemporalScale::Milliseconds),
                    SuspendBehavior::ClockExcludesSuspend,
                )
                .unwrap(),
            ),
            missed: MissedOccurrencePolicy::Skip,
            payload: effect,
        },
    };
    proposal.validate().unwrap();

    let mut gate = ProposalGate::new(None, 1).unwrap();
    let disposition = gate.submit(proposal.effect_proposal().clone()).unwrap();
    assert_eq!(
        disposition.decision.outcome,
        ProposalDecisionOutcome::Refused(ProposalRefusal::MissingAuthority)
    );
    assert!(disposition.request.is_none());
}
