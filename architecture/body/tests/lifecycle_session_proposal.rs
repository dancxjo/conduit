use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyFaceSelector,
    BodyLifecycleSession, BodyLifecycleSessionError, BodyMaskTopology, BodyMembership,
    BodyPlanError, BodyPlotPlan, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::{seal_plan, BootId, HostId, OfferGeneration, PlotIdentity};

fn fixture() -> (BodyLifecycleSession, BodyPlotPlan, HostId, BootId) {
    let host: HostId = "host/local".into();
    let boot: BootId = "boot/local".into();
    let resident = ResidentPlot::new("source/work".into(), "checked/work".into());
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "born".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Test".into()).unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("local-birth").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "admitted".into(),
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
            "present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: resident.source_document_id.clone(),
            checked_plot_id: resident.checked_plot_id.clone(),
            expanded_plot_id: "expanded/work".into(),
        },
        vec![],
    );
    (
        BodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap(),
        BodyPlotPlan {
            plot: resident,
            plan,
        },
        host,
        boot,
    )
}

#[test]
fn empty_mask_proposals_are_the_same_canonical_transaction() {
    let (mut plain, partition, host, boot) = fixture();
    let mut masks = plain.clone();
    plain
        .propose(vec![partition.clone()], &host, &boot)
        .unwrap();
    masks
        .propose_with_masks(vec![partition], vec![], &host, &boot)
        .unwrap();
    assert_eq!(plain.evidence(), masks.evidence());
    assert_eq!(plain.realization(), masks.realization());
}

#[test]
fn invalid_mask_proposal_preserves_evidence_and_archive_obligations() {
    let (mut session, partition, host, boot) = fixture();
    // Reach the boundary at which proposal must archive a terminal Wake. A
    // later seal failure must not leave that compaction partially published.
    let mut reached_archive_boundary = false;
    for _ in 0..64 {
        let mut next = session.clone();
        next.propose(vec![partition.clone()], &host, &boot).unwrap();
        if !next.pending_archives().is_empty() {
            reached_archive_boundary = true;
            break;
        }
        next.lull(&host, &boot, None).unwrap();
        session = next;
    }
    assert!(reached_archive_boundary);
    let before = session.evidence().clone();
    let archives = session.pending_archives().to_vec();
    let invalid = BodyMaskTopology {
        face: BodyFaceSelector {
            plot: Some(partition.plot.clone()),
            source_placement_id: None,
        },
        chains: vec![],
    };
    assert_eq!(
        session.propose_with_masks(vec![partition], vec![invalid], &host, &boot),
        Err(BodyLifecycleSessionError::Plan(
            BodyPlanError::InvalidMaskChain
        ))
    );
    assert_eq!(session.evidence(), &before);
    assert_eq!(session.pending_archives(), archives);
    assert!(session.realization().is_none());
}

#[test]
fn stale_authority_boot_cannot_publish_a_proposal() {
    let (mut session, partition, host, _) = fixture();
    let before = session.evidence().clone();
    assert_eq!(
        session.propose_with_masks(vec![partition], vec![], &host, &"boot/stale".into()),
        Err(BodyLifecycleSessionError::StaleHost)
    );
    assert_eq!(session.evidence(), &before);
}
