//! Canonical ownership and native execution boundaries, independent of rendering.
use super::{test_support::*, *};

fn born() -> (BootIdentities, HostOffer<'static>, ProductJourney) {
    let (ids, offer, mut journey) = fixture();
    invoke(&mut journey, JourneyAction::OpenBack, &ids, &offer).unwrap();
    invoke(&mut journey, JourneyAction::Birth, &ids, &offer).unwrap();
    (ids, offer, journey)
}

#[test]
fn refused_wake_preserves_exact_born_evidence_without_a_proposal() {
    let (ids, offer, mut journey) = born();
    let before = journey.biography().unwrap().clone();
    let absent = HostOffer::new(&ids, "build", offer.cpu_features, offer.runtime_arena_bytes);
    assert_eq!(
        invoke(&mut journey, JourneyAction::Wake, &ids, &absent),
        Err(JourneyError::Workset(native_workset::WorksetRefusal::Host))
    );
    assert_eq!(journey.biography(), Some(&before));
    assert!(journey.session.as_ref().unwrap().realization().is_none());
    assert!(journey.kernel.is_none());
}

#[test]
fn preparation_preserves_proposal_and_start_records_exact_bound_signs() {
    let (ids, offer, mut journey) = born();
    invoke(&mut journey, JourneyAction::Wake, &ids, &offer).unwrap();
    let proposal = journey
        .session
        .as_ref()
        .unwrap()
        .realization()
        .unwrap()
        .clone();
    let evidence = journey.biography().unwrap().clone();
    invoke(&mut journey, JourneyAction::Plan, &ids, &offer).unwrap();
    assert_eq!(journey.current_plan(), Some(&proposal.plan));
    assert_eq!(journey.biography(), Some(&evidence));
    assert!(journey.current_play().is_none());
    invoke(&mut journey, JourneyAction::Play, &ids, &offer).unwrap();
    let play = journey.current_play().unwrap();
    let expected = |sequence| {
        conduit_core::bind_sign(
            &journey.host_id,
            &journey.boot_id,
            Some(&play.active_play_id),
            sequence,
        )
        .sign_id
    };
    assert_eq!(journey.projection().plan_sign_id, Some(expected(0)));
    assert_eq!(journey.projection().play_sign_id, Some(expected(1)));
    assert_eq!(journey.current_plan(), Some(&proposal.plan));
    invoke(&mut journey, JourneyAction::Lull, &ids, &offer).unwrap();
    assert!(journey.kernel.is_none());
    assert!(journey.session.as_ref().unwrap().realization().is_none());
    assert_eq!(journey.body().unwrap().state, BodyState::Lulled);
    assert_eq!(
        journey.biography().unwrap().wakes.last().unwrap().lifecycle,
        conduit_body::WakeLifecycle::Lulled
    );
}

#[test]
fn proof_identifier_cannot_manufacture_peer_membership() {
    let (_, _, mut journey) = born();
    let before = journey.biography().unwrap().clone();
    assert_eq!(
        journey.admit_line_peer(
            "peer".into(),
            "peer-boot".into(),
            MembershipProofId::bind("unverified").unwrap()
        ),
        Err(JourneyError::AdmissionUnsupported)
    );
    assert_eq!(journey.biography(), Some(&before));
}

#[test]
fn native_archive_boundary_refuses_before_wake_without_acknowledging_storage() {
    let (ids, offer, mut journey) = born();
    for _ in 0..128 {
        let before = journey.biography().unwrap().clone();
        match invoke(&mut journey, JourneyAction::Wake, &ids, &offer) {
            Err(JourneyError::Lifecycle(BodyLifecycleSessionError::ArchivePersistenceRequired)) => {
                assert_eq!(journey.biography(), Some(&before));
                assert!(
                    journey
                        .session
                        .as_ref()
                        .unwrap()
                        .pending_archives()
                        .is_empty()
                );
                assert!(journey.kernel.is_none());
                assert_eq!(journey.body().unwrap().state, BodyState::Lulled);
                return;
            }
            Ok(()) => journey.retire_realization().unwrap(),
            Err(error) => panic!("unexpected refusal: {error:?}"),
        }
    }
    panic!("finite native retention was silently extended");
}

#[test]
fn mask_participant_requires_exact_current_membership_before_wake() {
    let (ids, offer, mut journey) = born();
    let partitions =
        native_workset::propose_partitions(&journey.body().unwrap().workset, &ids, &offer, "build")
            .unwrap();
    let mut mask_plan = partitions[0].plan.clone();
    mask_plan.fragments[0].host_id = "host/unadmitted-mask".into();
    let mask = conduit_body::BodyMaskTopology {
        face: conduit_body::BodyFaceSelector {
            plot: Some(partitions[0].plot.clone()),
            source_placement_id: None,
        },
        chains: alloc::vec![conduit_body::BodyMaskChainPlan {
            plan: mask_plan,
            stage_placement_ids: alloc::vec!["mask".into()]
        }],
    };
    let before = journey.biography().unwrap().clone();
    let result = journey.session.as_mut().unwrap().propose_with_masks(
        partitions,
        alloc::vec![mask],
        &journey.host_id,
        &journey.boot_id,
    );
    assert_eq!(result, Err(BodyLifecycleSessionError::StaleHost));
    assert_eq!(journey.biography(), Some(&before));
}

#[test]
fn changed_offer_cannot_replace_the_sealed_proposal_during_preparation() {
    let (ids, offer, mut journey) = born();
    invoke(&mut journey, JourneyAction::Wake, &ids, &offer).unwrap();
    let proposal = journey.current_plan().unwrap().clone();
    let before = journey.biography().unwrap().clone();
    let changed = HostOffer::new(
        &ids,
        "different-build",
        offer.cpu_features,
        offer.runtime_arena_bytes,
    )
    .with_keyboard(offer.keyboard.unwrap().realization, "different-build")
    .unwrap();
    assert_eq!(
        journey.plan(&ids, &changed, "different-build"),
        Err(JourneyError::Workset(native_workset::WorksetRefusal::Plan))
    );
    assert_eq!(journey.current_plan(), Some(&proposal));
    assert_eq!(journey.biography(), Some(&before));
    assert!(journey.kernel.is_none());
}
