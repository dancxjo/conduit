use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, BodyMembershipRevision, HostPresenceClock,
    HostPresenceClockScale, HostPresenceEventKind, HostPresenceRefusal, HostPresenceState,
    HostPresenceTable, MembershipProofId, PartId, MAX_PRESENCE_EVENTS,
};

use conduit_core::{
    BootId, CheckedFormId, HostId, LinkBindingId, OfferGeneration, SignId, SourceDocumentId,
};

fn clock(label: &str) -> HostPresenceClock {
    HostPresenceClock::new(
        format!("clock/conformance/{label}"),
        HostPresenceClockScale::Milliseconds,
        1,
        0,
    )
    .unwrap()
}

fn admitted() -> (BodyMembership, PartId) {
    let body = Body::born(
        SourceDocumentId::from("source/presence"),
        CheckedFormId::from("checked/presence"),
        1,
        SignId::from("sign/body-born"),
    )
    .unwrap();
    let body_id = body.body_id;
    let part_id = PartId::bind(&body_id, "part/browser", 1).unwrap();
    let mut membership = BodyMembership::new(body_id.clone()).unwrap();
    membership
        .admit(
            &body_id,
            BodyMembershipRevision(0),
            part_id.clone(),
            MembershipProofId::bind("proof/admitted").unwrap(),
            SignId::from("sign/admitted"),
        )
        .unwrap();
    membership
        .observe_present(
            &body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: HostId::from("host/browser"),
                boot_id: BootId::from("boot/browser/1"),
                offer_generation: OfferGeneration(7),
                proof_id: MembershipProofId::bind("proof/attached").unwrap(),
                sequence: 1,
            },
            SignId::from("sign/attached"),
        )
        .unwrap();
    (membership, part_id)
}

#[test]
fn clock_truth_is_required_validated_and_preserved_by_serialization() {
    assert_eq!(
        HostPresenceClock::new(
            "clock/invalid-resolution".into(),
            HostPresenceClockScale::Milliseconds,
            0,
            0,
        ),
        Err(HostPresenceRefusal::InvalidClock)
    );
    assert_eq!(
        HostPresenceClock::new(String::new(), HostPresenceClockScale::Milliseconds, 1, 0,),
        Err(HostPresenceRefusal::EmptyIdentity)
    );

    let (membership, _) = admitted();
    let clock = HostPresenceClock::new(
        "clock/serialization/exact".into(),
        HostPresenceClockScale::Milliseconds,
        1,
        7,
    )
    .unwrap();
    let table = HostPresenceTable::new(membership.body_id, clock.clone(), 30_000).unwrap();
    let encoded = serde_json::to_value(&table).unwrap();
    let decoded: HostPresenceTable = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(decoded.clock, clock);

    let mut legacy = encoded;
    legacy.as_object_mut().unwrap().remove("clock");
    assert!(serde_json::from_value::<HostPresenceTable>(legacy).is_err());

    let mut malformed = table;
    malformed.clock.resolution_ticks = 0;
    assert_eq!(malformed.validate(), Err(HostPresenceRefusal::InvalidClock));
}

#[test]
fn epoch_and_process_relative_timelines_keep_equal_ticks_incomparable() {
    let (membership, part_id) = admitted();
    let epoch_clock = HostPresenceClock::new(
        "clock/unix-epoch/utc".into(),
        HostPresenceClockScale::Milliseconds,
        1,
        5,
    )
    .unwrap();
    let process_clock = HostPresenceClock::new(
        "clock/process-restart/conformance-1".into(),
        HostPresenceClockScale::Milliseconds,
        1,
        1,
    )
    .unwrap();
    let mut epoch =
        HostPresenceTable::new(membership.body_id.clone(), epoch_clock, 30_000).unwrap();
    let mut process =
        HostPresenceTable::new(membership.body_id.clone(), process_clock, 30_000).unwrap();

    for (table, label) in [(&mut epoch, "epoch"), (&mut process, "process")] {
        table
            .start(
                &membership,
                &part_id,
                LinkBindingId::from(format!("binding/{label}")),
                1,
                1_000,
                10_000,
                SignId::from(format!("sign/{label}")),
            )
            .unwrap();
    }

    assert_eq!(
        epoch.leases[0].observed_at_millis,
        process.leases[0].observed_at_millis
    );
    assert_ne!(epoch.clock.basis_id, process.clock.basis_id);
    let epoch_encoded = serde_json::to_value(&epoch).unwrap();
    let process_encoded = serde_json::to_value(&process).unwrap();
    assert_ne!(epoch_encoded["clock"], process_encoded["clock"]);
    assert_eq!(
        epoch_encoded["leases"][0]["observed_at_millis"],
        process_encoded["leases"][0]["observed_at_millis"]
    );
}

#[test]
fn renewal_advances_presence_without_mutating_membership_or_offer_truth() {
    let (membership, part_id) = admitted();
    let membership_before = membership.clone();
    let session = LinkBindingId::from("binding/browser/session-1");
    let mut table =
        HostPresenceTable::new(membership.body_id.clone(), clock("presence"), 30_000).unwrap();
    table
        .start(
            &membership,
            &part_id,
            session.clone(),
            1,
            1_000,
            20_000,
            SignId::from("sign/presence/1"),
        )
        .unwrap();
    table
        .renew(
            &membership,
            &part_id,
            &session,
            2,
            10_000,
            20_000,
            SignId::from("sign/presence/2"),
        )
        .unwrap();
    assert_eq!(membership, membership_before);
    assert_eq!(table.leases[0].offer_generation, OfferGeneration(7));
    assert_eq!(table.leases[0].sequence, 2);
    assert_eq!(table.leases[0].expires_at_millis, 30_000);
    assert_eq!(table.leases[0].state, HostPresenceState::Available);
}

#[test]
fn stale_replayed_wrong_session_and_clock_regression_refuse_distinctly() {
    let (membership, part_id) = admitted();
    let session = LinkBindingId::from("binding/browser/session-1");
    let mut table =
        HostPresenceTable::new(membership.body_id.clone(), clock("presence"), 30_000).unwrap();
    table
        .start(
            &membership,
            &part_id,
            session.clone(),
            4,
            1_000,
            20_000,
            SignId::from("sign/presence/start"),
        )
        .unwrap();
    assert_eq!(
        table.renew(
            &membership,
            &part_id,
            &session,
            4,
            2_000,
            20_000,
            SignId::from("sign/presence/replay")
        ),
        Err(HostPresenceRefusal::StaleSequence)
    );
    assert_eq!(
        table.renew(
            &membership,
            &part_id,
            &LinkBindingId::from("binding/browser/stale"),
            5,
            2_000,
            20_000,
            SignId::from("sign/presence/wrong-session")
        ),
        Err(HostPresenceRefusal::WrongSession)
    );
    assert_eq!(
        table.renew(
            &membership,
            &part_id,
            &session,
            5,
            999,
            20_000,
            SignId::from("sign/presence/clock")
        ),
        Err(HostPresenceRefusal::ClockRegressed)
    );
    assert_eq!(
        table.renew(
            &membership,
            &part_id,
            &session,
            5,
            2_000,
            0,
            SignId::from("sign/presence/zero")
        ),
        Err(HostPresenceRefusal::LeaseDurationZero)
    );
}

#[test]
fn host_boot_offer_and_body_drift_refuse_without_changing_the_current_lease() {
    let (membership, part_id) = admitted();
    let session = LinkBindingId::from("binding/browser/session-1");
    let mut table =
        HostPresenceTable::new(membership.body_id.clone(), clock("presence"), 30_000).unwrap();
    table
        .start(
            &membership,
            &part_id,
            session.clone(),
            1,
            1_000,
            5_000,
            SignId::from("sign/presence/start"),
        )
        .unwrap();
    let before = table.clone();
    let reattach = |host: &str, boot: &str, offer: u64, suffix: &str| {
        let mut changed = membership.clone();
        changed
            .observe_offline(
                &changed.body_id.clone(),
                changed.revision,
                &part_id,
                &BootId::from("boot/browser/1"),
                SignId::from(format!("sign/offline/{suffix}")),
            )
            .unwrap();
        changed
            .observe_present(
                &changed.body_id.clone(),
                changed.revision,
                &part_id,
                AuthenticatedHostObservation {
                    host_id: HostId::from(host),
                    boot_id: BootId::from(boot),
                    offer_generation: OfferGeneration(offer),
                    proof_id: MembershipProofId::bind(&format!("proof/{suffix}")).unwrap(),
                    sequence: 2,
                },
                SignId::from(format!("sign/reattached/{suffix}")),
            )
            .unwrap();
        changed
    };
    let changed = reattach("host/browser/other", "boot/browser/1", 7, "host");
    assert_eq!(
        table.renew(
            &changed,
            &part_id,
            &session,
            2,
            2_000,
            5_000,
            SignId::from("sign/presence/wrong-host")
        ),
        Err(HostPresenceRefusal::WrongHost)
    );
    let changed = reattach("host/browser", "boot/browser/other", 7, "boot");
    assert_eq!(
        table.renew(
            &changed,
            &part_id,
            &session,
            2,
            2_000,
            5_000,
            SignId::from("sign/presence/stale-boot")
        ),
        Err(HostPresenceRefusal::StaleBoot)
    );
    let changed = reattach("host/browser", "boot/browser/1", 8, "offer");
    assert_eq!(
        table.renew(
            &changed,
            &part_id,
            &session,
            2,
            2_000,
            5_000,
            SignId::from("sign/presence/stale-offer")
        ),
        Err(HostPresenceRefusal::StaleOfferGeneration)
    );
    let changed = reattach("host/browser", "boot/browser/1", 7, "proof");
    assert_eq!(
        table.renew(
            &changed,
            &part_id,
            &session,
            2,
            2_000,
            5_000,
            SignId::from("sign/presence/stale-proof")
        ),
        Err(HostPresenceRefusal::StaleMembershipProof)
    );
    assert_eq!(table, before);
}

#[test]
fn expiry_preserves_part_but_clears_current_boot_and_requires_fresh_continuity() {
    let (mut membership, part_id) = admitted();
    let old_session = LinkBindingId::from("binding/browser/session-1");
    let mut table =
        HostPresenceTable::new(membership.body_id.clone(), clock("presence"), 30_000).unwrap();
    table
        .start(
            &membership,
            &part_id,
            old_session.clone(),
            1,
            1_000,
            5_000,
            SignId::from("sign/presence/start"),
        )
        .unwrap();
    assert_eq!(
        table.expire(
            &mut membership,
            &part_id,
            5_999,
            SignId::from("sign/presence/early")
        ),
        Err(HostPresenceRefusal::LeaseStillCurrent)
    );
    table
        .expire(
            &mut membership,
            &part_id,
            6_000,
            SignId::from("sign/presence/expired"),
        )
        .unwrap();
    assert!(membership.parts[0].current.is_none());
    assert_eq!(
        membership.parts[0].state,
        conduit_body::MembershipState::Admitted
    );
    assert_eq!(
        table.renew(
            &membership,
            &part_id,
            &old_session,
            2,
            7_000,
            5_000,
            SignId::from("sign/presence/late")
        ),
        Err(HostPresenceRefusal::HostUnavailable)
    );
    let body_id = membership.body_id.clone();
    membership
        .observe_present(
            &body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: HostId::from("host/browser"),
                boot_id: BootId::from("boot/browser/2"),
                offer_generation: OfferGeneration(7),
                proof_id: MembershipProofId::bind("proof/continuity/2").unwrap(),
                sequence: 2,
            },
            SignId::from("sign/reattached"),
        )
        .unwrap();
    table
        .start(
            &membership,
            &part_id,
            LinkBindingId::from("binding/browser/session-2"),
            3,
            8_000,
            5_000,
            SignId::from("sign/presence/restarted"),
        )
        .unwrap();
    assert_eq!(table.leases[0].boot_id, BootId::from("boot/browser/2"));
}

#[test]
fn exact_session_loss_detaches_immediately_without_masquerading_as_expiry() {
    let (mut membership, part_id) = admitted();
    let session = LinkBindingId::from("binding/browser/session-1");
    let mut table =
        HostPresenceTable::new(membership.body_id.clone(), clock("presence"), 30_000).unwrap();
    table
        .start(
            &membership,
            &part_id,
            session.clone(),
            1,
            1_000,
            20_000,
            SignId::from("sign/presence/start"),
        )
        .unwrap();
    let membership_before = membership.clone();
    assert_eq!(
        table.lose_session(
            &mut membership,
            &part_id,
            &LinkBindingId::from("binding/browser/other"),
            2_000,
            SignId::from("sign/presence/wrong-session"),
        ),
        Err(HostPresenceRefusal::WrongSession)
    );
    assert_eq!(membership, membership_before);
    table
        .lose_session(
            &mut membership,
            &part_id,
            &session,
            2_000,
            SignId::from("sign/presence/session-lost"),
        )
        .unwrap();
    assert!(membership.parts[0].current.is_none());
    assert_eq!(table.leases[0].state, HostPresenceState::Unavailable);
    assert_eq!(
        table.events.last().unwrap().kind,
        HostPresenceEventKind::SessionLost
    );
    assert_eq!(table.events.last().unwrap().expires_at_millis, 21_000);
    table.validate().unwrap();
}

#[test]
fn renewal_history_is_finite_and_reports_the_exact_retention_gap() {
    let (membership, part_id) = admitted();
    let session = LinkBindingId::from("binding/browser/session-1");
    let mut table =
        HostPresenceTable::new(membership.body_id.clone(), clock("presence"), 30_000).unwrap();
    table
        .start(
            &membership,
            &part_id,
            session.clone(),
            1,
            0,
            10_000,
            SignId::from("sign/presence/1"),
        )
        .unwrap();
    for sequence in 2..=80 {
        table
            .renew(
                &membership,
                &part_id,
                &session,
                sequence,
                sequence,
                10_000,
                SignId::from(format!("sign/presence/{sequence}")),
            )
            .unwrap();
    }
    assert_eq!(table.events.len(), MAX_PRESENCE_EVENTS);
    assert_eq!(table.dropped_event_count, 16);
    assert_eq!(table.events[0].revision, 17);
    assert_eq!(table.events.last().unwrap().revision, 80);
    table.validate().unwrap();
}

#[test]
fn malformed_or_overflown_state_refuses_before_lease_or_membership_mutation() {
    let (mut membership, part_id) = admitted();
    let session = LinkBindingId::from("binding/browser/session-1");
    let mut table =
        HostPresenceTable::new(membership.body_id.clone(), clock("presence"), 30_000).unwrap();
    table
        .start(
            &membership,
            &part_id,
            session.clone(),
            1,
            1_000,
            5_000,
            SignId::from("sign/presence/start"),
        )
        .unwrap();
    let lease_before = table.leases[0].clone();
    let membership_before = membership.clone();
    table.revision = u64::MAX;
    table.dropped_event_count = u64::MAX - 1;
    table.events[0].revision = u64::MAX;
    assert_eq!(
        table.renew(
            &membership,
            &part_id,
            &session,
            2,
            2_000,
            5_000,
            SignId::from("sign/presence/overflow")
        ),
        Err(HostPresenceRefusal::RevisionOverflow)
    );
    assert_eq!(table.leases[0], lease_before);
    assert_eq!(membership, membership_before);
    table.revision = 1;
    table.dropped_event_count = 0;
    table.events[0].revision = 1;
    table.leases[0].sequence = 9;
    assert_eq!(
        table.expire(
            &mut membership,
            &part_id,
            6_000,
            SignId::from("sign/presence/malformed")
        ),
        Err(HostPresenceRefusal::MalformedState)
    );
    assert_eq!(membership, membership_before);
}
