//! Explicit CPU membership/start fixtures, never authenticated physical starts.
use conduit_core as c;
pub fn body_owner(
    resident: conduit_body::ResidentPlot,
    host: &c::HostId,
    boot: &c::BootId,
    wrong_source: bool,
) -> conduit_body::BodyLifecycleSession {
    use conduit_body::*;
    let body = Body::born(
        if wrong_source {
            "fixture/different-source".into()
        } else {
            resident.source_document_id
        },
        resident.checked_plot_id,
        1,
        c::bind_sign(host, boot, None, 1).sign_id,
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 0).unwrap();
    let proof = MembershipProofId::bind("fixture/Make-lifecycle").unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            c::bind_sign(host, boot, None, 2).sign_id,
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
                offer_generation: c::OfferGeneration(1),
                proof_id: proof,
                sequence: 0,
            },
            c::bind_sign(host, boot, None, 3).sign_id,
        )
        .unwrap();
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone()).unwrap(),
        "Mica Quail".into(),
    )
    .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    BodyLifecycleSession::open_admitted(evidence, host, boot).unwrap()
}

/// Simulated Root start receipt for CPU lifecycle proof, not a hardware start.
pub fn start_body(
    owner: &mut conduit_body::BodyLifecycleSession,
    play: &conduit_body::BodyPlayIdentity,
    host: &c::HostId,
    boot: &c::BootId,
) {
    let current = owner.realization().unwrap();
    let wake = current
        .wake
        .body_plan_ready(
            &current.plan,
            c::bind_sign(host, boot, Some(&play.active_play_id), 0).sign_id,
        )
        .unwrap()
        .body_play_started(
            &current.plan,
            play,
            c::bind_sign(host, boot, Some(&play.active_play_id), 1).sign_id,
        )
        .unwrap();
    owner.started(host, boot, play.clone(), wake).unwrap();
}
