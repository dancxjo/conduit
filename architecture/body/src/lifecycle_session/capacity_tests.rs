//! Bounded synthetic lifecycle evidence; no live/private Body identity is used.
use super::*;
use crate::{
    AuthenticatedHostObservation, Body, BodyMembership, BodyPlotPlan, MembershipProofId, PartId,
};
use alloc::{format, vec};
use conduit_core::{seal_plan, OfferGeneration, PlotIdentity};

fn fixture() -> (BodyLifecycleSession, BodyPlotPlan, HostId, BootId) {
    let host: HostId = "host/capacity".into();
    let boot: BootId = "boot/capacity".into();
    let resident = ResidentPlot::new("source/capacity".into(), "checked/capacity".into());
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/birth".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence = BodyBiographyEvidence::born(
        body.clone(),
        membership.clone(),
        "Continuing fixture".into(),
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("proof/capacity").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/admit".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "sign/present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let partition = BodyPlotPlan {
        plot: resident.clone(),
        plan: seal_plan(
            PlotIdentity {
                source_document_id: resident.source_document_id,
                checked_plot_id: resident.checked_plot_id,
                expanded_plot_id: "expanded/capacity".into(),
            },
            vec![],
        ),
    };
    (
        BodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap(),
        partition,
        host,
        boot,
    )
}

// Simulate admitted current-offer observations arriving after proposal/start.
// Every appended event is a real validated membership extension, not padding.
fn fill_records(session: &mut BodyLifecycleSession, target: usize) {
    while session.evidence.records.len() < target {
        session.make_membership_room(1).unwrap();
        let at = session.next_sequence().unwrap();
        let mut membership = session.evidence.membership.clone();
        let part = membership.parts[0].clone();
        let mut observed = part.current.unwrap();
        observed.sequence += 1;
        observed.offer_generation.0 += 1;
        let change = membership
            .observe_present(
                &session.evidence.body_id,
                membership.revision,
                &part.part_id,
                observed,
                format!("sign/observation/{at}").as_str().into(),
            )
            .unwrap();
        session
            .evidence
            .append_membership_events(membership, &[(change, at)])
            .unwrap();
    }
    session.evidence.validate().unwrap();
}
fn start(session: &mut BodyLifecycleSession, host: &HostId, boot: &BootId) -> BodyPlayIdentity {
    let current = session.realization().unwrap();
    let play = BodyPlayIdentity::bind(&current.plan, 1);
    let evidence_sign =
        |sequence| bind_sign(host, boot, Some(&play.active_play_id), sequence).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, evidence_sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, evidence_sign(1))
        .unwrap();
    session.started(host, boot, play.clone(), wake).unwrap();
    play
}
fn assert_archived_same_body(
    session: &BodyLifecycleSession,
    original: &BodyBiographyEvidence,
    added: usize,
) {
    assert_eq!(session.evidence.body_id, original.body_id);
    assert_eq!(
        session.evidence.body.events.first(),
        original.body.events.first()
    );
    assert!(session.evidence.records.len() <= crate::MAX_BODY_BIOGRAPHY_RECORDS);
    assert!(session.evidence.body.sign_ids.len() <= crate::MAX_BODY_SIGNS);
    assert!(!session.pending_archives.is_empty());
    for segment in &session.pending_archives {
        segment.validate().unwrap();
    }
    assert_eq!(
        session.evidence.records.len()
            + session
                .pending_archives
                .iter()
                .map(|s| s.records.len())
                .sum::<usize>(),
        original.records.len() + added
    );
    session.evidence.validate().unwrap();
}

#[test]
fn start_and_lull_recheck_capacity_consumed_after_proposal() {
    for before_start in [true, false] {
        let (mut session, partition, host, boot) = fixture();
        session.propose(vec![partition], &host, &boot).unwrap();
        let play = if before_start {
            None
        } else {
            Some(start(&mut session, &host, &boot))
        };
        fill_records(&mut session, crate::MAX_BODY_BIOGRAPHY_RECORDS - 1);
        let original = session.evidence.clone();
        if before_start {
            start(&mut session, &host, &boot);
        } else {
            session.lull(&host, &boot, play.as_ref()).unwrap();
        }
        assert_archived_same_body(&session, &original, 2);
    }
}

#[test]
fn refusal_archives_before_its_three_record_terminal_extension() {
    let (mut session, partition, host, boot) = fixture();
    session.propose(vec![partition], &host, &boot).unwrap();
    fill_records(&mut session, crate::MAX_BODY_BIOGRAPHY_RECORDS - 1);
    let original = session.evidence.clone();
    let current = session.realization().unwrap();
    let rejection = crate::WakeRejectionEvidence {
        reason_code: "fixture/refused".into(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        plan_id: Some(current.plan.plan_id.clone()),
        checked_plot_ids: current
            .plan
            .plots
            .iter()
            .map(|p| p.plot.checked_plot_id.clone())
            .collect(),
        category: "fixture".into(),
        stage: "admission".into(),
        resource: "fixture/slot".into(),
        required: 1,
        available: 0,
    };
    session.fail(&host, &boot, vec![rejection]).unwrap();
    assert_archived_same_body(&session, &original, 3);
    assert!(session.realization().is_none());
}

#[test]
fn restart_reserves_membership_and_failed_wake_together() {
    let (mut session, partition, host, boot) = fixture();
    session.propose(vec![partition], &host, &boot).unwrap();
    start(&mut session, &host, &boot);
    fill_records(&mut session, crate::MAX_BODY_BIOGRAPHY_RECORDS - 3);
    let original = session.evidence.clone();
    let resumed =
        BodyLifecycleSession::resume_here(original.clone(), &host, &"boot/after-crash".into())
            .unwrap();
    assert_archived_same_body(&resumed, &original, 4);
    assert!(resumed.realization().is_none());
    assert!(matches!(resumed.evidence.body.state, BodyState::Lulled));
    assert!(matches!(
        resumed.evidence.wakes.last().unwrap().lifecycle,
        crate::WakeLifecycle::Failed
    ));
}

#[test]
fn stale_termination_refuses_without_compacting_or_creating_archive_debt() {
    let (mut session, partition, host, boot) = fixture();
    session.propose(vec![partition], &host, &boot).unwrap();
    fill_records(&mut session, crate::MAX_BODY_BIOGRAPHY_RECORDS - 1);
    let before = session.evidence.clone();
    let realization = session.realization.clone();
    let stale = BodyPlayIdentity::bind(&session.realization().unwrap().plan, 99);
    assert_eq!(
        session.lull(&host, &boot, Some(&stale)),
        Err(BodyLifecycleSessionError::StalePlay)
    );
    assert_eq!(session.evidence, before);
    assert_eq!(session.realization, realization);
    assert!(session.pending_archives.is_empty());
}
