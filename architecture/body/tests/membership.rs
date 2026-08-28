use conduit_body::{
    AuthenticatedHostObservation, Body, BodyId, BodyMembership, BodyMembershipRevision,
    MembershipEventKind, MembershipProofId, MembershipRefusal, MembershipState, PartId,
    MAX_BODY_PARTS, MAX_MEMBERSHIP_EVENTS,
};
use conduit_core::{BootId, CheckedFormId, HostId, OfferGeneration, SignId, SourceDocumentId};

fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source/body-membership"),
        CheckedFormId::from("checked/body-membership"),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap()
}

fn part(body_id: &BodyId, name: &str, sequence: u64) -> PartId {
    PartId::bind(body_id, name, sequence).unwrap()
}

fn proof(name: &str) -> MembershipProofId {
    MembershipProofId::bind(name).unwrap()
}

fn observation(boot: &str, generation: u64, sequence: u64) -> AuthenticatedHostObservation {
    AuthenticatedHostObservation {
        host_id: HostId::from("host/stable-a"),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(generation),
        proof_id: proof("proof/continuity-a"),
        sequence,
    }
}

#[test]
fn membership_survives_offline_without_retaining_a_fake_boot() {
    let body = body();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part_id = part(&body.body_id, "subject/a", 1);
    membership
        .admit(
            &body.body_id,
            BodyMembershipRevision(0),
            part_id.clone(),
            proof("proof/admission-a"),
            SignId::from("sign/admitted"),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            BodyMembershipRevision(1),
            &part_id,
            observation("boot/one", 4, 1),
            SignId::from("sign/present"),
        )
        .unwrap();
    membership
        .observe_offline(
            &body.body_id,
            BodyMembershipRevision(2),
            &part_id,
            &BootId::from("boot/one"),
            SignId::from("sign/offline"),
        )
        .unwrap();

    let retained = &membership.parts[0];
    assert_eq!(retained.state, MembershipState::Admitted);
    assert_eq!(retained.current, None);
    assert!(!retained.is_present());
    assert!(matches!(
        membership.events.last().unwrap().kind,
        MembershipEventKind::HostDetached { .. }
    ));
    membership.validate().unwrap();
}

#[test]
fn fresh_boot_can_return_under_same_part_only_with_fresh_observation() {
    let body = body();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part_id = part(&body.body_id, "subject/a", 1);
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part_id.clone(),
            proof("proof/admission"),
            SignId::from("sign/admit"),
        )
        .unwrap();
    let admitted_change = membership.events[0].change_id.clone();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            observation("boot/old", 8, 1),
            SignId::from("sign/old-present"),
        )
        .unwrap();
    membership
        .observe_offline(
            &body.body_id,
            membership.revision,
            &part_id,
            &BootId::from("boot/old"),
            SignId::from("sign/lost"),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: HostId::from("host/stable-a"),
                boot_id: BootId::from("boot/fresh"),
                offer_generation: OfferGeneration(1),
                proof_id: proof("proof/future-soul-policy"),
                sequence: 2,
            },
            SignId::from("sign/fresh-present"),
        )
        .unwrap();

    let retained = &membership.parts[0];
    assert_eq!(retained.part_id, part_id);
    assert_eq!(
        retained.current.as_ref().unwrap().boot_id.as_str(),
        "boot/fresh"
    );
    assert_ne!(retained.part_id.as_str(), body.body_id.as_str());
    assert_ne!(
        retained.part_id.as_str(),
        retained.current.as_ref().unwrap().host_id.as_str()
    );
    assert_ne!(
        retained.current.as_ref().unwrap().host_id.as_str(),
        retained.current.as_ref().unwrap().boot_id.as_str()
    );
    assert_ne!(admitted_change, membership.events[1].change_id);
}

#[test]
fn duplicate_stale_wrong_body_and_malformed_changes_refuse_distinctly() {
    let body = body();
    let other = Body::born(
        SourceDocumentId::from("source/other"),
        CheckedFormId::from("checked/other"),
        2,
        SignId::from("sign/other-born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part_id = part(&body.body_id, "subject/a", 1);
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part_id.clone(),
            proof("proof/a"),
            SignId::from("sign/admit"),
        )
        .unwrap();

    assert_eq!(
        membership.admit(
            &body.body_id,
            membership.revision,
            part_id.clone(),
            proof("proof/a"),
            SignId::from("sign/duplicate-part"),
        ),
        Err(MembershipRefusal::DuplicatePart)
    );
    assert_eq!(
        membership.observe_present(
            &body.body_id,
            BodyMembershipRevision(0),
            &part_id,
            observation("boot/a", 1, 1),
            SignId::from("sign/stale"),
        ),
        Err(MembershipRefusal::StaleRevision)
    );
    assert_eq!(
        membership.observe_present(
            &other.body_id,
            membership.revision,
            &part_id,
            observation("boot/a", 1, 1),
            SignId::from("sign/wrong-body"),
        ),
        Err(MembershipRefusal::WrongBody)
    );
    assert_eq!(
        membership.observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            observation("boot/a", 1, 1),
            SignId::from("sign/admit"),
        ),
        Err(MembershipRefusal::DuplicateSign)
    );

    let mut malformed = membership.clone();
    malformed.events[0].body_id = other.body_id;
    assert_eq!(malformed.validate(), Err(MembershipRefusal::MalformedState));
}

#[test]
fn stale_observation_and_wrong_boot_detach_do_not_mutate_current_truth() {
    let body = body();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part_id = part(&body.body_id, "subject/a", 1);
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part_id.clone(),
            proof("proof/a"),
            SignId::from("sign/admit"),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            observation("boot/current", 3, 7),
            SignId::from("sign/current"),
        )
        .unwrap();
    let before = membership.clone();

    assert_eq!(
        membership.observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            observation("boot/stale", 2, 6),
            SignId::from("sign/stale-observation"),
        ),
        Err(MembershipRefusal::StaleObservation)
    );
    assert_eq!(membership, before);
    assert_eq!(
        membership.observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            observation("boot/current", 3, 8),
            SignId::from("sign/stale-offer"),
        ),
        Err(MembershipRefusal::StaleOfferGeneration)
    );
    assert_eq!(membership, before);
    assert_eq!(
        membership.observe_offline(
            &body.body_id,
            membership.revision,
            &part_id,
            &BootId::from("boot/wrong"),
            SignId::from("sign/wrong-detach"),
        ),
        Err(MembershipRefusal::ObservationMismatch)
    );
    assert_eq!(membership, before);
}

#[test]
fn membership_and_event_storage_are_strictly_finite() {
    let body = body();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    for index in 0..MAX_BODY_PARTS {
        membership
            .admit(
                &body.body_id,
                membership.revision,
                part(&body.body_id, "bounded-subject", index as u64),
                proof("proof/bounded"),
                SignId::from(format!("sign/admit-{index}").as_str()),
            )
            .unwrap();
    }
    assert_eq!(
        membership.admit(
            &body.body_id,
            membership.revision,
            part(&body.body_id, "overflow", 99),
            proof("proof/overflow"),
            SignId::from("sign/overflow"),
        ),
        Err(MembershipRefusal::PartCapacityExhausted)
    );

    let first = membership.parts[0].part_id.clone();
    let mut sequence = 1u64;
    while membership.events.len() < MAX_MEMBERSHIP_EVENTS {
        if membership.parts[0].current.is_none() {
            membership
                .observe_present(
                    &body.body_id,
                    membership.revision,
                    &first,
                    observation(format!("boot/{sequence}").as_str(), sequence, sequence),
                    SignId::from(format!("sign/present-{sequence}").as_str()),
                )
                .unwrap();
        } else {
            let boot = membership.parts[0]
                .current
                .as_ref()
                .unwrap()
                .boot_id
                .clone();
            membership
                .observe_offline(
                    &body.body_id,
                    membership.revision,
                    &first,
                    &boot,
                    SignId::from(format!("sign/offline-{sequence}").as_str()),
                )
                .unwrap();
            sequence += 1;
        }
    }
    assert_eq!(
        membership.revoke(
            &body.body_id,
            membership.revision,
            &first,
            SignId::from("sign/over-event-capacity"),
        ),
        Err(MembershipRefusal::EventCapacityExhausted)
    );
}

#[test]
fn revocation_removes_presence_without_granting_any_other_state() {
    let body = body();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part_id = part(&body.body_id, "subject/a", 1);
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part_id.clone(),
            proof("proof/a"),
            SignId::from("sign/admit"),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            observation("boot/a", 1, 1),
            SignId::from("sign/present"),
        )
        .unwrap();
    membership
        .revoke(
            &body.body_id,
            membership.revision,
            &part_id,
            SignId::from("sign/revoke"),
        )
        .unwrap();

    assert_eq!(membership.parts[0].state, MembershipState::Revoked);
    assert_eq!(membership.parts[0].current, None);
    assert_eq!(
        membership.observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            observation("boot/reappear", 1, 2),
            SignId::from("sign/reappear"),
        ),
        Err(MembershipRefusal::RevokedPart)
    );
}
