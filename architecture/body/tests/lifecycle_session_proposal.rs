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

#[test]
fn prepared_verified_proposal_retires_input_plans_before_atomic_publication() {
    use conduit_body::{SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let mut direct =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let mut staged = direct.clone();
    let token = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let inputs = [VerifiedBodyPlotPlan {
        plot: partition.plot,
        plan: &token,
    }];
    let before = staged.evidence().clone();
    let prepared = staged
        .prepare_verified_proposal(&inputs, &[], None, &host, &boot)
        .unwrap();
    assert_eq!(staged.evidence(), &before);
    assert!(staged.realization().is_none());
    direct
        .propose_verified(&inputs, &[], None, &host, &boot)
        .unwrap();
    drop(token);
    staged.commit_verified_proposal(prepared).unwrap();
    assert_eq!(staged.evidence(), direct.evidence());
    assert_eq!(staged.realization(), direct.realization());
    assert_eq!(
        postcard::to_allocvec(staged.evidence()).unwrap(),
        postcard::to_allocvec(direct.evidence()).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(staged.evidence()).unwrap(),
        serde_json::to_vec(direct.evidence()).unwrap()
    );
}

#[test]
fn stale_prepared_proposal_cannot_publish_after_membership_or_wake_changes() {
    use conduit_body::{SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let initial =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let token = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let inputs = [VerifiedBodyPlotPlan {
        plot: partition.plot,
        plan: &token,
    }];
    let mut changed = initial.clone();
    let pending = changed
        .prepare_verified_proposal(&inputs, &[], None, &host, &boot)
        .unwrap();
    changed
        .observe_host_lost(&host, &boot, &host, &boot)
        .unwrap();
    let before = changed.evidence().clone();
    assert_eq!(
        changed.commit_verified_proposal(pending),
        Err(BodyLifecycleSessionError::StaleHost)
    );
    assert_eq!(changed.evidence(), &before);
    assert!(changed.realization().is_none());
    let mut changed = initial;
    let pending = changed
        .prepare_verified_proposal(&inputs, &[], None, &host, &boot)
        .unwrap();
    changed
        .propose_verified(&inputs, &[], None, &host, &boot)
        .unwrap();
    changed.lull(&host, &boot, None).unwrap();
    let before = changed.evidence().clone();
    assert_eq!(
        changed.commit_verified_proposal(pending),
        Err(BodyLifecycleSessionError::StaleWorkload)
    );
    assert_eq!(changed.evidence(), &before);
    assert!(changed.realization().is_none());
}

#[test]
fn refused_invitation_at_membership_compaction_boundary_preserves_play_and_archives() {
    use conduit_body::{AdmissionManager, SpawnAdmissionProof, SpawnInvitationSecret};
    let (session, mut partition, host, boot) = fixture();
    let mut evidence = session.evidence().clone();
    let part = evidence.membership.parts[0].part_id.clone();
    while evidence.records.len() < conduit_body::MAX_BODY_BIOGRAPHY_RECORDS - 5 {
        let sequence = evidence.last_sequence() + 1;
        let mut membership = evidence.membership.clone();
        let change = membership
            .observe_present(
                &evidence.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: host.clone(),
                    boot_id: boot.clone(),
                    offer_generation: OfferGeneration(sequence),
                    proof_id: MembershipProofId::bind("pressure/current-root").unwrap(),
                    sequence,
                },
                format!("pressure/presence/{sequence}").into(),
            )
            .unwrap();
        evidence
            .append_membership_events(membership, &[(change, sequence)])
            .unwrap();
    }
    let generation = evidence.membership.parts[0]
        .current
        .as_ref()
        .unwrap()
        .offer_generation;
    let mut fragment = core_fixture::fragment();
    fragment.source_document_id = partition.plot.source_document_id.clone();
    fragment.checked_plot_id = partition.plot.checked_plot_id.clone();
    fragment.expanded_plot_id = "expanded/work".into();
    fragment.host_id = host.clone();
    fragment.boot_id = boot.clone();
    fragment.offer_generation = generation;
    for placement in &mut fragment.placements {
        placement.host_id = host.clone();
        placement.boot_id = boot.clone();
    }
    partition.plan = core_fixture::seal(fragment);
    let mut session = BodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    session.propose(vec![partition], &host, &boot).unwrap();
    let current = session.realization().unwrap();
    let play = conduit_body::BodyPlayIdentity::bind(&current.plan, 1);
    let started = current
        .wake
        .body_plan_ready(
            &current.plan,
            conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), 0).sign_id,
        )
        .unwrap()
        .body_play_started(
            &current.plan,
            &play,
            conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), 1).sign_id,
        )
        .unwrap();
    session.started(&host, &boot, play, started).unwrap();
    let mut manager = AdmissionManager::new(session.evidence().body_id.clone()).unwrap();
    // Proposal/start reserve five biography records. One actual successful
    // observer admission fills the remaining records while this Play runs.
    let warm_secret = SpawnInvitationSecret::from_csprng_bytes([81; 32]).unwrap();
    let warm_claim = session
        .issue_invitation(
            &mut manager,
            warm_secret.clone(),
            [82; 32],
            1,
            100,
            &host,
            &boot,
        )
        .unwrap();
    let warm_advertisement = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "host/warm-observer".into(),
        boot_id: "boot/warm-observer".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/observer".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let warm_proof = SpawnAdmissionProof {
        invitation_id: warm_claim.invitation_id.clone(),
        body_id: warm_claim.body_id.clone(),
        host_id: warm_advertisement.host_id.clone(),
        boot_id: warm_advertisement.boot_id.clone(),
        nonce: warm_claim.nonce,
        signature: warm_secret.sign(&warm_claim.signing_transcript(
            &warm_advertisement.host_id,
            &warm_advertisement.boot_id,
            warm_advertisement.offer_generation,
        )),
    };
    session
        .admit_invited_host(
            &mut manager,
            &warm_advertisement,
            &warm_proof,
            2,
            &host,
            &boot,
        )
        .unwrap();
    assert_eq!(
        session.evidence().records.len(),
        conduit_body::MAX_BODY_BIOGRAPHY_RECORDS
    );
    let secret = SpawnInvitationSecret::from_csprng_bytes([91; 32]).unwrap();
    let claim = session
        .issue_invitation(
            &mut manager,
            secret.clone(),
            [92; 32],
            10,
            100,
            &host,
            &boot,
        )
        .unwrap();
    let advertisement = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "host/observer".into(),
        boot_id: "boot/observer".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/observer".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let proof = SpawnAdmissionProof {
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        nonce: claim.nonce,
        signature: secret.sign(&claim.signing_transcript(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        )),
    };
    let before = session.evidence().clone();
    let archives = session.pending_archives().to_vec();
    let realization = session.realization().unwrap().clone();
    let admissions = manager.clone();
    for failure in 0..5 {
        let mut wrong = proof.clone();
        let mut now = 11;
        match failure {
            0 => wrong.signature[0] ^= 1,
            1 => wrong.nonce[0] ^= 1,
            2 => wrong.host_id = "host/foreign".into(),
            3 => wrong.boot_id = "boot/foreign".into(),
            _ => now = 101,
        }
        assert!(session
            .admit_invited_host(&mut manager, &advertisement, &wrong, now, &host, &boot)
            .is_err());
        assert_eq!(session.evidence(), &before);
        assert_eq!(session.pending_archives(), archives);
        assert_eq!(session.realization(), Some(&realization));
        assert_eq!(manager, admissions);
    }
    session
        .admit_invited_host(&mut manager, &advertisement, &proof, 11, &host, &boot)
        .unwrap();
    assert_eq!(session.pending_archives().len(), archives.len() + 1);
    assert_eq!(session.realization(), Some(&realization));
    assert_eq!(
        session.evidence().membership.parts.len(),
        before.membership.parts.len() + 1
    );
    session.evidence().validate().unwrap();
    session
        .pending_archives()
        .last()
        .unwrap()
        .validate_as_head_of(session.evidence())
        .unwrap();
}

#[test]
fn cold_proposal_context_releases_owner_and_verified_inputs_before_exact_commit() {
    use conduit_body::{SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let owner =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let evidence = owner.evidence().clone();
    let context = owner
        .prepare_verified_proposal_context(&host, &boot)
        .unwrap();
    drop(owner);
    let verified = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let inputs = [VerifiedBodyPlotPlan {
        plot: partition.plot,
        plan: &verified,
    }];
    let prepared = context.prepare_verified(&inputs, &[], None).unwrap();
    let mut expected =
        SealedBodyLifecycleSession::open_admitted(evidence.clone(), &host, &boot).unwrap();
    expected
        .propose_verified(&inputs, &[], None, &host, &boot)
        .unwrap();
    drop(verified);
    let mut restored = SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    restored.commit_verified_proposal(prepared).unwrap();
    assert_eq!(restored.evidence(), expected.evidence());
    assert_eq!(restored.realization(), expected.realization());
    assert_eq!(
        postcard::to_allocvec(restored.evidence()).unwrap(),
        postcard::to_allocvec(expected.evidence()).unwrap()
    );
}

#[test]
fn cold_proposal_context_keeps_membership_seal_and_complete_state_refusals() {
    use conduit_body::{SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let initial =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    assert!(matches!(
        initial.prepare_verified_proposal_context(&host, &"stale".into()),
        Err(BodyLifecycleSessionError::StaleHost)
    ));
    let mut foreign = partition.plan.fragments[0].clone();
    foreign.host_id = "foreign".into();
    for placement in &mut foreign.placements {
        placement.host_id = foreign.host_id.clone();
    }
    let foreign =
        conduit_core::verify_plan_metadata_with_configurations(core_fixture::seal(foreign), |_| {
            Ok::<_, ()>(vec![])
        })
        .unwrap();
    let foreign_input = [VerifiedBodyPlotPlan {
        plot: partition.plot.clone(),
        plan: &foreign,
    }];
    assert!(matches!(
        initial
            .prepare_verified_proposal_context(&host, &boot)
            .unwrap()
            .prepare_verified(&foreign_input, &[], None),
        Err(BodyLifecycleSessionError::StaleHost)
    ));
    assert!(initial
        .prepare_verified_proposal_context(&host, &boot)
        .unwrap()
        .prepare_verified(&[], &[], None)
        .is_err());
    let verified = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let inputs = [VerifiedBodyPlotPlan {
        plot: partition.plot,
        plan: &verified,
    }];
    let prepared = initial
        .prepare_verified_proposal_context(&host, &boot)
        .unwrap()
        .prepare_verified(&inputs, &[], None)
        .unwrap();
    let mut changed = initial;
    changed
        .propose_verified(&inputs, &[], None, &host, &boot)
        .unwrap();
    changed.lull(&host, &boot, None).unwrap();
    let evidence = changed.evidence().clone();
    assert!(matches!(
        changed.commit_verified_proposal(prepared),
        Err(BodyLifecycleSessionError::StaleWorkload)
    ));
    assert_eq!(changed.evidence(), &evidence);
    assert!(changed.realization().is_none());
}

#[test]
fn incremental_verified_plot_seal_retires_core_metadata_before_complete_proposal() {
    use conduit_body::{SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let evidence = rich.evidence().clone();
    let owner = SealedBodyLifecycleSession::open_admitted(evidence.clone(), &host, &boot).unwrap();
    let context = owner
        .prepare_verified_proposal_context(&host, &boot)
        .unwrap();
    let verified = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let input = VerifiedBodyPlotPlan {
        plot: partition.plot,
        plan: &verified,
    };
    let prepared = context.prepare_plot(&input).unwrap();
    let mut expected = owner.clone();
    expected
        .propose_verified(core::slice::from_ref(&input), &[], None, &host, &boot)
        .unwrap();
    drop(input);
    drop(verified);
    drop(owner);
    let proposal = context
        .prepare_verified_with_prepared(core::slice::from_ref(&prepared), &[], &[], None)
        .unwrap();
    drop(prepared);
    let mut restored = SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    restored.commit_verified_proposal(proposal).unwrap();
    assert_eq!(restored.evidence(), expected.evidence());
    assert_eq!(restored.realization(), expected.realization());
}

#[test]
fn incremental_plot_seal_cannot_cross_complete_body_state_contexts() {
    use conduit_body::{SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let owner =
        SealedBodyLifecycleSession::open_admitted(rich.evidence().clone(), &host, &boot).unwrap();
    let context = owner
        .prepare_verified_proposal_context(&host, &boot)
        .unwrap();
    let verified = conduit_core::verify_plan_metadata_with_configurations(partition.plan, |_| {
        Ok::<_, ()>(vec![])
    })
    .unwrap();
    let prepared = context
        .prepare_plot(&VerifiedBodyPlotPlan {
            plot: partition.plot,
            plan: &verified,
        })
        .unwrap();
    let mut evidence = owner.evidence().clone();
    evidence.friendly_name.push_str(" changed");
    let other = SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    assert!(matches!(
        other
            .prepare_verified_proposal_context(&host, &boot)
            .unwrap()
            .prepare_verified_with_prepared(&[prepared], &[], &[], None),
        Err(BodyLifecycleSessionError::StaleWorkload)
    ));
    assert!(other.realization().is_none());
}

#[test]
fn retained_context_and_proposal_survive_checked_refusals_then_commit_exact_original() {
    use conduit_body::{SealedBodyLifecycleSession, VerifiedBodyPlotPlan};
    let (rich, partition, host, boot) = fixture();
    let original = rich.evidence().clone();
    let mut owner =
        SealedBodyLifecycleSession::open_admitted(original.clone(), &host, &boot).unwrap();
    let verified =
        conduit_core::verify_plan_metadata_with_configurations(partition.plan.clone(), |_| {
            Ok::<_, ()>(vec![])
        })
        .unwrap();
    let input = VerifiedBodyPlotPlan {
        plot: partition.plot.clone(),
        plan: &verified,
    };
    let context = owner
        .prepare_verified_proposal_context(&host, &boot)
        .unwrap();
    let prepared_plot = context.prepare_plot(&input).unwrap();
    let mut foreign = partition.plan.fragments[0].clone();
    foreign.host_id = "foreign".into();
    for placement in &mut foreign.placements {
        placement.host_id = foreign.host_id.clone();
    }
    let foreign =
        conduit_core::verify_plan_metadata_with_configurations(core_fixture::seal(foreign), |_| {
            Ok::<_, ()>(vec![])
        })
        .unwrap();
    let foreign_input = [VerifiedBodyPlotPlan {
        plot: partition.plot,
        plan: &foreign,
    }];
    let (context, error) =
        match context.prepare_verified_with_prepared_retained(&[], &foreign_input, &[], None) {
            Err(refused) => refused,
            Ok(_) => panic!("foreign Host must refuse"),
        };
    assert_eq!(error, BodyLifecycleSessionError::StaleHost);
    let (context, error) =
        match context.prepare_verified_with_prepared_retained(&[], &[], &[], None) {
            Err(refused) => refused,
            Ok(_) => panic!("missing workset must refuse"),
        };
    assert!(matches!(error, BodyLifecycleSessionError::PlanSeal(_)));
    let mut changed = original.clone();
    changed.friendly_name.push_str(" changed");
    let mut changed = SealedBodyLifecycleSession::open_admitted(changed, &host, &boot).unwrap();
    let foreign_plot = changed
        .prepare_verified_proposal_context(&host, &boot)
        .unwrap()
        .prepare_plot(&input)
        .unwrap();
    let (context, error) = match context.prepare_verified_with_prepared_retained(
        core::slice::from_ref(&foreign_plot),
        &[],
        &[],
        None,
    ) {
        Err(refused) => refused,
        Ok(_) => panic!("foreign state seal must refuse"),
    };
    assert_eq!(error, BodyLifecycleSessionError::StaleWorkload);
    assert_eq!(owner.evidence(), &original);
    let prepared = context
        .prepare_verified_with_prepared_retained(
            core::slice::from_ref(&prepared_plot),
            &[],
            &[],
            None,
        )
        .map_err(|(_, error)| error)
        .unwrap();
    let before = changed.evidence().clone();
    let (prepared, error) = match changed.commit_verified_proposal_retained(prepared) {
        Err(refused) => refused,
        Ok(_) => panic!("changed state must refuse"),
    };
    assert_eq!(error, BodyLifecycleSessionError::StaleWorkload);
    assert_eq!(changed.evidence(), &before);
    assert!(changed.realization().is_none());
    let mut lost =
        SealedBodyLifecycleSession::open_admitted(original.clone(), &host, &boot).unwrap();
    lost.observe_host_lost(&host, &boot, &host, &boot).unwrap();
    let before = lost.evidence().clone();
    let (prepared, error) = match lost.commit_verified_proposal_retained(prepared) {
        Err(refused) => refused,
        Ok(_) => panic!("stale Boot must refuse"),
    };
    assert_eq!(error, BodyLifecycleSessionError::StaleHost);
    assert_eq!(lost.evidence(), &before);
    let mut proposed =
        SealedBodyLifecycleSession::open_admitted(original.clone(), &host, &boot).unwrap();
    proposed
        .propose_verified(core::slice::from_ref(&input), &[], None, &host, &boot)
        .unwrap();
    let before = proposed.evidence().clone();
    let realization = proposed.realization().cloned();
    let (prepared, error) = match proposed.commit_verified_proposal_retained(prepared) {
        Err(refused) => refused,
        Ok(_) => panic!("live proposal must refuse"),
    };
    assert_eq!(error, BodyLifecycleSessionError::NotLulled);
    assert_eq!(proposed.evidence(), &before);
    assert_eq!(proposed.realization(), realization.as_ref());
    let mut expected = SealedBodyLifecycleSession::open_admitted(original, &host, &boot).unwrap();
    expected
        .propose_verified(core::slice::from_ref(&input), &[], None, &host, &boot)
        .unwrap();
    owner
        .commit_verified_proposal_retained(prepared)
        .map_err(|(_, error)| error)
        .unwrap();
    assert_eq!(owner.evidence(), expected.evidence());
    assert_eq!(owner.realization(), expected.realization());
    assert_eq!(
        postcard::to_allocvec(owner.evidence()).unwrap(),
        postcard::to_allocvec(expected.evidence()).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(owner.evidence()).unwrap(),
        serde_json::to_vec(expected.evidence()).unwrap()
    );
}

#[path = "lifecycle_session_proposal/continuing.rs"]
mod continuing;
