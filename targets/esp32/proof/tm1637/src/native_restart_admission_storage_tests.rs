//! Actual native Store re-decode and publication; CPU media only, no heap-fit claim.
use super::*;

fn another_invitation(manager: &mut AdmissionManager) -> conduit_body::SpawnInvitation {
    manager
        .issue_spawn_invitation(
            conduit_body::SpawnInvitationSecret::from_csprng_bytes([31; 32]).unwrap(),
            [32; 32],
            10,
            100,
        )
        .unwrap()
}
fn admit_with_original_invitation(
    manager: &mut AdmissionManager,
    membership: &mut conduit_body::BodyMembership,
    invitation: &conduit_body::SpawnInvitation,
) {
    let peer = conduit_core::HostAdvertisement {
        declared_name: None,
        host_id: "host/restored-invitation".into(),
        boot_id: "boot/restored-invitation".into(),
        protocol_version: conduit_core::PROTOCOL_VERSION,
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "fixture/restored-invitation".into(),
        bases: Vec::new(),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    };
    let proof = conduit_body::SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: peer.host_id.clone(),
        boot_id: peer.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &peer.host_id,
            &peer.boot_id,
            peer.offer_generation,
        )),
    };
    let signs = || conduit_body::AdmissionSigns {
        part_admitted: "sign/restored-part".into(),
        host_attached: "sign/restored-host".into(),
        candidate_admitted: "sign/restored-candidate".into(),
    };
    manager
        .complete_spawn(membership, &peer, &proof, 11, signs())
        .unwrap();
    assert_eq!(
        manager.complete_spawn(membership, &peer, &proof, 12, signs()),
        Err(conduit_body::AdmissionRefusal::Replay)
    );
}

#[test]
fn final_restore_compacts_redecoded_admissions_without_changing_body_or_authority() {
    let (mut store, mut owner, host, key) = fixture();
    let manager = store.admissions.as_mut().unwrap();
    let invitation = another_invitation(manager);
    compact_admission_storage(manager);
    let before = serde_json::to_vec(manager).unwrap();
    let initial = manager.storage_usage().invitations;
    assert_eq!(initial.len, 2);
    assert_eq!(initial.capacity, initial.len);
    let evidence = owner.as_ref().unwrap().evidence().clone();
    let mut token = park_store(&mut store, &mut owner, &key, &host.host_id, &host.boot_id).unwrap();
    let generation = store.journal.generation();
    let custody = store.custody.as_ref().unwrap();
    let decoded = store
        .journal
        .read_snapshot(&custody.source, |reader| {
            payload::decode_streamed(|out| reader.read(out).map_err(|_| ()), &evidence.body_id)
                .map_err(|_| Refusal::Corrupt)
        })
        .unwrap();
    let grown = decoded
        .admissions
        .as_ref()
        .unwrap()
        .storage_usage()
        .invitations;
    assert!(
        grown.capacity > grown.len,
        "actual streamed decoding must reproduce spare backing"
    );
    drop(decoded);
    let restored =
        restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &key).unwrap();
    assert_eq!(restored.evidence(), &evidence);
    assert_eq!(store.journal.generation(), generation);
    assert_eq!(store.seed, Some([7; 32]));
    assert!(store.ready && !store.parked && !store.failed);
    let manager = store.admissions.as_mut().unwrap();
    let final_usage = manager.storage_usage().invitations;
    assert_eq!(final_usage.len, initial.len);
    assert_eq!(final_usage.capacity, final_usage.len);
    assert_eq!(serde_json::to_vec(manager).unwrap(), before);
    admit_with_original_invitation(
        manager,
        &mut restored.evidence().membership.clone(),
        &invitation,
    );
}

#[test]
fn optional_compaction_null_refusal_keeps_exact_manager_usable() {
    let (mut store, owner, _, _) = fixture();
    let manager = store.admissions.as_mut().unwrap();
    let invitation = another_invitation(manager);
    let usage = manager.storage_usage();
    assert!(usage.invitations.capacity > usage.invitations.len);
    let before = serde_json::to_vec(manager).unwrap();
    let ((), memory) = crate::allocation_probe::observe(|| {
        let _refusal = crate::allocation_probe::refuse_next_allocation_size(
            usage.invitations.len * usage.invitations.element_bytes,
        );
        compact_admission_storage(manager);
    });
    assert_eq!(
        memory.allocations + memory.reallocations + memory.deallocations,
        0
    );
    assert_eq!(manager.storage_usage(), usage);
    assert_eq!(serde_json::to_vec(manager).unwrap(), before);
    admit_with_original_invitation(
        manager,
        &mut owner.unwrap().evidence().membership.clone(),
        &invitation,
    );
}
