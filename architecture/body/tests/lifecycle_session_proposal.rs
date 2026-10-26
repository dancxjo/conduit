use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyFaceSelector,
    BodyLifecycleSession, BodyLifecycleSessionError, BodyMaskTopology, BodyMembership,
    BodyPlanError, BodyPlotPlan, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::{BootId, HostId, OfferGeneration};

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
    let mut fragment = core_fixture::fragment();
    fragment.source_document_id = resident.source_document_id.clone();
    fragment.checked_plot_id = resident.checked_plot_id.clone();
    fragment.expanded_plot_id = "expanded/work".into();
    fragment.host_id = host.clone();
    fragment.boot_id = boot.clone();
    for placement in &mut fragment.placements {
        placement.host_id = host.clone();
        placement.boot_id = boot.clone();
    }
    let plan = core_fixture::seal(fragment);
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

#[test]
fn sealed_owner_shares_proposal_start_and_lull_history_with_the_rich_owner() {
    use conduit_body::{BodyPlanEvidence, SealedBodyLifecycleSession};
    let (mut rich, partition, host, boot) = fixture();
    let mut compact =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let mut verified = compact.clone();
    let token =
        conduit_core::verify_plan_metadata_with_configurations(partition.plan.clone(), |_| {
            Ok::<_, ()>(vec![])
        })
        .unwrap();
    verified
        .propose_verified(
            &[conduit_body::VerifiedBodyPlotPlan {
                plot: partition.plot.clone(),
                plan: &token,
            }],
            &[],
            None,
            &host,
            &boot,
        )
        .unwrap();
    rich.propose(vec![partition.clone()], &host, &boot).unwrap();
    compact.propose(vec![partition], &host, &boot).unwrap();
    assert_eq!(rich.evidence(), compact.evidence());
    assert_eq!(verified.evidence(), compact.evidence());
    assert_eq!(verified.realization(), compact.realization());
    let expected = rich.realization().unwrap();
    let retained = compact.realization().unwrap();
    assert_eq!(expected.plan.plan_id, *retained.plan.plan_id());
    assert_eq!(expected.wake, retained.wake);
    let play = conduit_body::BodyPlayIdentity::bind(&expected.plan, 1);
    let started = expected
        .wake
        .body_plan_ready(
            &expected.plan,
            conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), 0).sign_id,
        )
        .unwrap()
        .body_play_started(
            &expected.plan,
            &play,
            conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), 1).sign_id,
        )
        .unwrap();
    rich.started(&host, &boot, play.clone(), started.clone())
        .unwrap();
    compact
        .started(&host, &boot, play.clone(), started.clone())
        .unwrap();
    verified
        .started(&host, &boot, play.clone(), started)
        .unwrap();
    assert_eq!(rich.evidence(), compact.evidence());
    assert_eq!(verified.evidence(), compact.evidence());
    let startup = rich
        .evidence()
        .startup_for_play(&rich.realization().unwrap().plan, &play)
        .unwrap();
    assert_eq!(
        compact
            .evidence()
            .startup_for_play(&compact.realization().unwrap().plan, &play)
            .unwrap(),
        startup
    );
    rich.lull(&host, &boot, Some(&play)).unwrap();
    compact.lull(&host, &boot, Some(&play)).unwrap();
    verified.lull(&host, &boot, Some(&play)).unwrap();
    assert_eq!(rich.evidence(), compact.evidence());
    assert_eq!(verified.evidence(), compact.evidence());
    assert!(compact.realization().is_none());
}

#[test]
fn verified_proposal_refusals_preserve_the_unpublished_body() {
    use conduit_body::{SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let mut owner =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let token = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let valid = [VerifiedBodyPlotPlan {
        plot: partition.plot,
        plan: &token,
    }];
    let before = owner.evidence().clone();
    assert_eq!(
        owner.propose_verified(&valid, &[], None, &host, &"boot/stale".into()),
        Err(BodyLifecycleSessionError::StaleHost)
    );
    assert!(owner
        .propose_verified(&[], &[], None, &host, &boot)
        .is_err());
    assert_eq!(owner.evidence(), &before);
    assert!(owner.realization().is_none());
    assert!(owner.pending_archives().is_empty());
}

#[path = "../../core/tests/common/sealed_state.rs"]
mod core_fixture;

#[test]
fn verified_fragment_and_mask_hosts_still_require_current_membership() {
    use conduit_body::{
        SealedBodyLifecycleSession, VerifiedBodyMaskChain, VerifiedBodyMaskTopology,
        VerifiedBodyPlotPlan,
    };
    let (rich, partition, host, boot) = fixture();
    let mut owner =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let local = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let mut fragment = core_fixture::fragment();
    fragment.source_document_id = partition.plot.source_document_id.clone();
    fragment.checked_plot_id = partition.plot.checked_plot_id.clone();
    let foreign = conduit_core::verify_plan_metadata_with_configurations(
        core_fixture::seal(fragment),
        |_| Ok::<_, ()>(vec![]),
    )
    .unwrap();
    let before = owner.evidence().clone();
    assert_eq!(
        owner.propose_verified(
            &[VerifiedBodyPlotPlan {
                plot: partition.plot.clone(),
                plan: &foreign
            }],
            &[],
            None,
            &host,
            &boot
        ),
        Err(BodyLifecycleSessionError::StaleHost)
    );
    let mask = VerifiedBodyMaskTopology {
        face: BodyFaceSelector {
            plot: None,
            source_placement_id: None,
        },
        chains: vec![VerifiedBodyMaskChain {
            plan: &foreign,
            stage_placement_ids: vec!["placement".into()],
        }],
    };
    assert_eq!(
        owner.propose_verified(
            &[VerifiedBodyPlotPlan {
                plot: partition.plot,
                plan: &local
            }],
            &[mask],
            None,
            &host,
            &boot
        ),
        Err(BodyLifecycleSessionError::StaleHost)
    );
    assert_eq!(owner.evidence(), &before);
    assert!(owner.realization().is_none());
}

#[test]
fn verified_seal_failure_does_not_publish_compaction_or_wake() {
    use conduit_body::{
        SealedBodyLifecycleSession, VerifiedBodyMaskTopology, VerifiedBodyPlotPlan,
    };
    let (rich, partition, host, boot) = fixture();
    let token = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let plots = [VerifiedBodyPlotPlan {
        plot: partition.plot,
        plan: &token,
    }];
    let mut owner =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let mut boundary = false;
    for _ in 0..64 {
        let mut next = owner.clone();
        next.propose_verified(&plots, &[], None, &host, &boot)
            .unwrap();
        if !next.pending_archives().is_empty() {
            boundary = true;
            break;
        }
        next.lull(&host, &boot, None).unwrap();
        owner = next;
    }
    assert!(boundary);
    let before = owner.evidence().clone();
    let pending = owner.pending_archives().to_vec();
    let invalid = VerifiedBodyMaskTopology {
        face: BodyFaceSelector {
            plot: None,
            source_placement_id: None,
        },
        chains: vec![],
    };
    assert!(owner
        .propose_verified(&plots, &[invalid], None, &host, &boot)
        .is_err());
    assert_eq!(owner.evidence(), &before);
    assert_eq!(owner.pending_archives(), pending);
    assert!(owner.realization().is_none());
}
