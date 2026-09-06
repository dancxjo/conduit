use super::*;

fn proposal() -> BodyPlanningSession {
    let candidate = FormCandidate::from_source(
        "Hello",
        "forms/hello/main.conduit",
        include_str!("../../../../../../forms/hello/main.conduit"),
        "canonical test Form",
        "sign/reviewed".into(),
        1,
    )
    .unwrap();
    let resident = conduit_body::ResidentForm::new(
        candidate.source_document_id.clone(),
        candidate.checked_form_id.clone(),
    );
    let workset = BodyWorkset::one(resident).unwrap();
    let host = conduit_std_host::StdHost::new();
    let forms = plan_body_workset_on_host(
        &workset,
        &[candidate],
        host.advertisement(),
        &["conduit.base/local@1".into()],
    )
    .unwrap();
    let body = Body::born_with_forms(workset, 1, "sign/born".into()).unwrap();
    BodyPlanningSession::prepare(&body, 1, "sign/wake".into(), forms).unwrap()
}

fn claim(session: &mut BodyPlanningSession) -> BodyExecutionClaim {
    let plan = session.current_plan().clone();
    let fragment = &plan.forms[0].plan.fragments[0];
    session
        .claim_execution(&plan.plan_id, &fragment.host_id, &fragment.boot_id)
        .unwrap()
}

fn sign(claim: &BodyExecutionClaim, sequence: u64) -> SignId {
    bind_sign(
        &claim.host_id,
        &claim.boot_id,
        Some(&claim.play.active_play_id),
        sequence,
    )
    .sign_id
}

fn started(session: &BodyPlanningSession, claim: &BodyExecutionClaim) -> Wake {
    session
        .wake()
        .body_plan_ready(session.current_plan(), sign(claim, 0))
        .unwrap()
        .body_play_started(session.current_plan(), &claim.play, sign(claim, 1))
        .unwrap()
}

#[test]
fn claim_pins_exact_proposal_without_inventing_a_start() {
    let mut session = proposal();
    let wake = session.wake().clone();
    let claim = claim(&mut session);
    assert_eq!(session.wake(), &wake);
    assert_eq!(claim.proof_class, RemoteProofClass::SelfReported);
    let before = session.snapshot();
    assert_eq!(
        session.claim_execution(&claim.play.plan_id, &claim.host_id, &claim.boot_id),
        Err(BodyExecutionClaimError::OutstandingClaim)
    );
    assert!(session
        .replace_proposal(session.current_plan().forms.clone())
        .is_err());
    assert_eq!(session.snapshot(), before);
    let mut wrong = claim.play.clone();
    wrong.play_sequence += 1;
    assert_eq!(
        session.report_execution_refused(&wrong, "refused"),
        Err(BodyExecutionClaimError::UnknownClaim)
    );
    assert_eq!(session.snapshot(), before);
}

#[test]
fn refused_attempts_are_bounded_and_never_reuse_play_identity() {
    let mut session = proposal();
    let mut last = None;
    for sequence in 1..=MAX_WAKE_PLANS {
        let claim = claim(&mut session);
        assert_eq!(claim.play.play_sequence, sequence as u64);
        assert_ne!(last, Some(claim.play.active_play_id.clone()));
        last = Some(claim.play.active_play_id.clone());
        let before = session.snapshot();
        assert!(session.report_execution_refused(&claim.play, "").is_err());
        assert!(session
            .report_execution_refused(&claim.play, &"x".repeat(257))
            .is_err());
        assert_eq!(session.snapshot(), before);
        session
            .report_execution_refused(&claim.play, "resource acquisition refused")
            .unwrap();
    }
    let last = session.execution_claims.last().unwrap().clone();
    assert_eq!(
        session.claim_execution(&last.play.plan_id, &last.host_id, &last.boot_id),
        Err(BodyExecutionClaimError::CapacityExhausted)
    );
    assert_eq!(session.wake().lifecycle, WakeLifecycle::AwaitingPlan);
}

#[test]
fn wrong_host_boot_plan_and_unavailable_proposals_are_refused_atomically() {
    let mut session = proposal();
    let plan = session.current_plan().clone();
    let fragment = &plan.forms[0].plan.fragments[0];
    let before = session.snapshot();
    assert_eq!(
        session.claim_execution(&plan.plan_id, &fragment.host_id, &"boot/stale".into()),
        Err(BodyExecutionClaimError::WrongHost)
    );
    assert_eq!(
        session.claim_execution(&plan.plan_id, &"host/other".into(), &fragment.boot_id),
        Err(BodyExecutionClaimError::WrongHost)
    );
    assert_eq!(
        session.claim_execution(&"plan/stale".into(), &fragment.host_id, &fragment.boot_id),
        Err(BodyExecutionClaimError::StaleProposal)
    );
    assert_eq!(session.snapshot(), before);
    session
        .mark_current_unsatisfied("sign/lost".into())
        .unwrap();
    assert_eq!(
        session.claim_execution(&plan.plan_id, &fragment.host_id, &fragment.boot_id),
        Err(BodyExecutionClaimError::StaleProposal)
    );
}

#[test]
fn exact_start_and_terminal_reports_remain_distinct_from_lull() {
    let mut session = proposal();
    let claim = claim(&mut session);
    let started = started(&session, &claim);
    let before = session.snapshot();
    assert_eq!(
        session.report_execution_started(&claim.play, &session.wake().clone()),
        Err(BodyExecutionClaimError::InvalidReport)
    );
    assert_eq!(session.snapshot(), before);
    session
        .report_execution_started(&claim.play, &started)
        .unwrap();
    assert_eq!(session.wake(), &started);
    assert!(session
        .report_execution_refused(&claim.play, "too late")
        .is_err());
    assert!(session
        .report_execution_started(&claim.play, &started)
        .is_err());
    let before = session.snapshot();
    assert!(session
        .report_execution_terminal(&claim.play, "success", &sign(&claim, 2))
        .is_err());
    assert!(session
        .report_execution_terminal(&claim.play, "completed", &sign(&claim, 3))
        .is_err());
    assert_eq!(session.snapshot(), before);
    session
        .report_execution_terminal(&claim.play, "completed", &sign(&claim, 2))
        .unwrap();
    assert!(!session.has_outstanding_execution_claim());
    assert!(session.snapshot().execution_claims[0].started_reported);
    assert_eq!(session.wake().lifecycle, WakeLifecycle::Playing);
    assert!(session
        .claim_execution(&claim.play.plan_id, &claim.host_id, &claim.boot_id)
        .is_err());
}

#[test]
fn loss_after_claim_retains_actual_start_then_marks_unsatisfied() {
    let mut session = proposal();
    let claim = claim(&mut session);
    let started = started(&session, &claim);
    session
        .mark_current_unsatisfied("sign/lost".into())
        .unwrap();
    session
        .report_execution_started(&claim.play, &started)
        .unwrap();
    assert_eq!(session.wake().lifecycle, WakeLifecycle::Unsatisfied);
    assert!(session.has_outstanding_execution_claim());
    assert!(session
        .replan(
            session.current_plan().forms.clone(),
            BodyPlanningTransition {
                unsatisfied_sign_id: None,
                plan_ready_sign_id: "sign/ready".into(),
                play_sequence: 2,
                play_started_sign_id: "sign/started".into(),
            }
        )
        .is_err());
    session
        .report_execution_terminal(&claim.play, "cancelled", &sign(&claim, 2))
        .unwrap();
    assert_eq!(session.wake().lifecycle, WakeLifecycle::Unsatisfied);
}

#[test]
fn exact_cancellation_can_retire_an_unreadable_start_envelope() {
    let mut session = proposal();
    let first = claim(&mut session);
    session
        .report_execution_terminal(&first.play, "cancelled", &sign(&first, 2))
        .unwrap();
    assert!(!session.snapshot().execution_claims[0].started_reported);
    let second = claim(&mut session);
    assert_ne!(first.play.active_play_id, second.play.active_play_id);
    assert!(session
        .report_execution_terminal(&first.play, "cancelled", &sign(&first, 2))
        .is_err());
    assert!(session.has_outstanding_execution_claim());
}
