//! Owner-local cryptographic custody, not a network/Line or continuing Show proof.
use super::member_leave::{admit_with_key, owner_fixture};
use super::*;
use conduit_core::LineSecurity;
fn begin(
    owner: &mut Owner,
    credential: &conduit_body::MembershipCredential,
    ad: &conduit_core::HostAdvertisement,
) -> (conduit_body::FederationChallenge, [u8; 64]) {
    owner
        .begin_current_member_carrier(
            credential,
            ad,
            "fixture/line".into(),
            LineSecurity::AuthenticatedEncrypted,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
            128,
        )
        .unwrap()
}
fn signature(
    c: &conduit_body::FederationChallenge,
    key: &conduit_body::SpawnInvitationSecret,
) -> [u8; 64] {
    key.sign(&conduit_body::challenge_transcript(c).try_into().unwrap())
}
#[test]
fn current_member_carrier_authentication_and_retirement_do_not_readmit_or_withdraw() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = admit_with_key(&mut owner, &root, "host/carrier");
    let before = owner.session.evidence().clone();
    let admissions = owner.admissions.clone();
    for _ in 0..8 {
        let (challenge, receiver_signature) = begin(&mut owner, &credential, &ad);
        assert_eq!(challenge.initiator.verifying_key, key.verifying_key());
        assert!(owner
            .begin_current_member_carrier(
                &credential,
                &ad,
                "other/line".into(),
                LineSecurity::AuthenticatedEncrypted,
                crate::durable_host::current_time_millis().unwrap() + 60_000,
                128
            )
            .is_err());
        let proof = signature(&challenge, &key);
        let inspection = owner
            .complete_current_member_carrier(&challenge, proof)
            .unwrap();
        assert!(inspection.membership_current && inspection.credential_current);
        assert_eq!(inspection.session_id, challenge.session_id);
        assert!(owner
            .complete_current_member_carrier(&challenge, proof)
            .is_err());
        // A valid signature/session is not an initialized transport or effect grant.
        conduit_body::FederatedReceivingSession::establish(
            challenge.clone(),
            proof,
            receiver_signature,
            crate::durable_host::current_time_millis().unwrap(),
        )
        .unwrap();
        assert!(owner
            .begin_current_member_carrier(
                &credential,
                &ad,
                "other/line".into(),
                LineSecurity::AuthenticatedEncrypted,
                crate::durable_host::current_time_millis().unwrap() + 60_000,
                128
            )
            .is_err());
        assert!(owner
            .retire_current_member_carrier(&"foreign/session".into())
            .is_err());
        owner
            .retire_current_member_carrier(&challenge.session_id)
            .unwrap();
        assert_eq!(owner.session.evidence(), &before);
        assert_eq!(owner.admissions, admissions);
        assert_eq!(state::load(&root).unwrap().unwrap(), before);
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn bad_exact_proof_consumes_issuance_and_withdrawal_prevents_completion() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = admit_with_key(&mut owner, &root, "host/carrier-refusal");
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    let mut redirected = challenge.clone();
    redirected.responder.boot_id = "wrong/owner-boot".into();
    assert!(owner
        .complete_current_member_carrier(&redirected, signature(&redirected, &key))
        .is_err());
    assert!(owner
        .complete_current_member_carrier(&challenge, [0; 64])
        .is_err());
    assert!(owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .is_err());
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    owner.member_leave(&root, &credential).unwrap();
    let detached = owner.session.evidence().clone();
    assert!(owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .is_err());
    assert_eq!(owner.session.evidence(), &detached);
    assert!(owner
        .begin_current_member_carrier(
            &credential,
            &ad,
            "fixture/line".into(),
            LineSecurity::AuthenticatedEncrypted,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
            128
        )
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn same_incarnation_reattachment_retires_old_credential_session_without_waiting_for_expiry() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = admit_with_key(&mut owner, &root, "host/carrier-reattach");
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .unwrap();
    assert!(owner
        .inspect_current_member_carrier(&challenge.session_id)
        .is_ok());
    owner.member_leave(&root, &credential).unwrap();
    assert!(owner
        .inspect_current_member_carrier(&challenge.session_id)
        .is_err());
    let c = owner.begin_member_return(&credential, &ad).unwrap();
    let proof = conduit_body::PartReturnProof {
        admission_id: c.admission_id.clone(),
        body_id: c.body_id.clone(),
        part_id: c.part_id.clone(),
        host_id: c.host_id.clone(),
        boot_id: c.boot_id.clone(),
        nonce: c.nonce,
        signature: key.sign(&c.signing_transcript()),
    };
    let returned = owner.complete_member_return(&root, &proof).unwrap();
    assert_ne!(returned.credential_id, credential.credential_id);
    assert_eq!(returned.host_id, credential.host_id);
    assert_eq!(returned.boot_id, credential.boot_id);
    let attached = owner.session.evidence().clone();
    let admissions = owner.admissions.clone();
    assert!(owner
        .inspect_current_member_carrier(&challenge.session_id)
        .is_err());
    let (fresh, _) = begin(&mut owner, &returned, &ad);
    assert_ne!(fresh.session_id, challenge.session_id);
    owner
        .complete_current_member_carrier(&fresh, signature(&fresh, &key))
        .unwrap();
    assert!(owner
        .inspect_current_member_carrier(&fresh.session_id)
        .is_ok());
    assert_eq!(owner.session.evidence(), &attached);
    assert_eq!(owner.admissions, admissions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn member_presence_renews_but_cannot_resurrect_expired_carrier_lease() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = admit_with_key(&mut owner, &root, "host/presence");
    let before = owner.session.evidence().clone();
    let admissions = owner.admissions.clone();
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    assert!(owner
        .renew_current_member_presence(&challenge.session_id, 1)
        .is_err());
    owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .unwrap();
    assert!(owner
        .renew_current_member_presence(&challenge.session_id, 2)
        .is_err());
    assert!(owner
        .renew_current_member_presence(&"foreign/session".into(), 1)
        .is_err());
    for sequence in 1..=8 {
        let lease = owner
            .renew_current_member_presence(&challenge.session_id, sequence)
            .unwrap();
        assert_eq!(lease.sequence, sequence);
        assert_eq!(lease.session_binding_id, challenge.session_id);
        assert_eq!(lease.part_id, credential.part_id);
        assert_eq!(lease.expires_at_millis - lease.observed_at_millis, 2000);
        assert!(owner
            .renew_current_member_presence(&challenge.session_id, sequence)
            .is_err());
    }
    assert!(owner
        .renew_current_member_presence(&challenge.session_id, 129)
        .is_err());
    std::thread::sleep(std::time::Duration::from_millis(2010));
    assert!(owner
        .renew_current_member_presence(&challenge.session_id, 9)
        .is_err());
    // Overall authentication outlasts the shorter reachability lease.
    assert!(owner
        .inspect_current_member_carrier(&challenge.session_id)
        .is_ok());
    // Simulate a worker dying before exact cleanup: expired reachability must
    // not occupy the finite authentication slot until its longer deadline.
    let (fresh, _) = begin(&mut owner, &credential, &ad);
    assert!(owner
        .retire_current_member_carrier(&challenge.session_id)
        .is_err());
    assert!(owner
        .renew_current_member_presence(&challenge.session_id, 9)
        .is_err());
    owner
        .complete_current_member_carrier(&fresh, signature(&fresh, &key))
        .unwrap();
    assert!(owner
        .renew_current_member_presence(&fresh.session_id, 1)
        .is_ok());
    assert!(owner
        .renew_current_member_presence(&challenge.session_id, 9)
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, admissions);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[path = "member_carrier/presentation.rs"]
mod presentation;

#[test]
fn interrupted_challenge_retirement_releases_only_its_exact_pending_slot() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = admit_with_key(&mut owner, &root, "host/interrupted-carrier");
    let before = owner.session.evidence().clone();
    let admissions = owner.admissions.clone();
    let (interrupted, _) = begin(&mut owner, &credential, &ad);
    assert!(owner
        .retire_current_member_carrier(&"foreign/session".into())
        .is_err());
    assert!(owner
        .begin_current_member_carrier(
            &credential,
            &ad,
            "fixture/line".into(),
            LineSecurity::AuthenticatedEncrypted,
            crate::durable_host::current_time_millis().unwrap() + 60_000,
            128,
        )
        .is_err());
    owner
        .retire_current_member_carrier(&interrupted.session_id)
        .unwrap();
    let (replacement, _) = begin(&mut owner, &credential, &ad);
    assert_ne!(interrupted.session_id, replacement.session_id);
    assert!(owner
        .retire_current_member_carrier(&interrupted.session_id)
        .is_err());
    assert!(owner
        .complete_current_member_carrier(&interrupted, signature(&interrupted, &key),)
        .is_err());
    owner
        .complete_current_member_carrier(&replacement, signature(&replacement, &key))
        .unwrap();
    assert!(owner
        .inspect_current_member_carrier(&replacement.session_id)
        .is_ok());
    owner
        .retire_current_member_carrier(&replacement.session_id)
        .unwrap();
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, admissions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn receipt_attestation_requires_commit_exact_offer_and_pinned_owner() {
    let (mut owner, root) = owner_fixture();
    let invitation = owner.issue_invitation(&root, 60, None).unwrap();
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let mut ad = conduit_std_host::StdHost::new().advertisement().clone();
    // A minimal MCU identity/transport offer, not the much larger hosted workset.
    ad.bases.clear();
    ad.resources.clear();
    ad.capabilities.clear();
    ad.planner_capabilities.clear();
    let request = conduit_body::PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: ad.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &ad.host_id,
                &ad.boot_id,
                ad.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let receipt = owner
        .admit_invited(&root, request, ad.host_id.as_str())
        .unwrap();
    let peer = owner.prepare_current_member_carrier_receiver().unwrap();
    // Membership alone must not authorize arbitrary equipment supplied by a caller.
    owner.current_host_offers = Default::default();
    assert!(owner
        .attest_current_member_receipt(
            &crate::durable_host::owner::MembershipReceiptDocument::Spawn(receipt.clone()),
            &peer
        )
        .is_err());
    owner
        .current_host_offers
        .observe(owner.session.evidence(), ad.clone())
        .unwrap();
    let before = owner.session.evidence().clone();
    let signed = owner
        .attest_current_member_receipt(
            &crate::durable_host::owner::MembershipReceiptDocument::Spawn(receipt.clone()),
            &peer,
        )
        .unwrap();
    signed
        .verify(
            &peer,
            &key.verifying_key(),
            &receipt.credential.body_id,
            &ad,
        )
        .unwrap();
    assert_eq!(owner.session.evidence(), &before);
    let mut wrong_peer = peer.clone();
    wrong_peer.boot_id = "boot/foreign-owner".into();
    assert!(owner
        .attest_current_member_receipt(
            &crate::durable_host::owner::MembershipReceiptDocument::Spawn(receipt.clone()),
            &wrong_peer
        )
        .is_err());
    let mut changed = receipt.clone();
    changed.host_advertisement.profile = "profile/changed".into();
    assert!(owner
        .attest_current_member_receipt(
            &crate::durable_host::owner::MembershipReceiptDocument::Spawn(changed.clone()),
            &peer
        )
        .is_err());
    changed = receipt.clone();
    changed.credential.credential_id =
        serde_json::from_value(serde_json::json!("credential/not-committed")).unwrap();
    assert!(owner
        .attest_current_member_receipt(
            &crate::durable_host::owner::MembershipReceiptDocument::Spawn(changed.clone()),
            &peer
        )
        .is_err());
    changed = receipt;
    changed.play_created = true;
    assert!(owner
        .attest_current_member_receipt(
            &crate::durable_host::owner::MembershipReceiptDocument::Spawn(changed.clone()),
            &peer
        )
        .is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn carrier_preparation_preserves_selected_absolute_expiry() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = admit_with_key(&mut owner, &root, "host/deadline-carrier");
    let expiry = crate::durable_host::current_time_millis().unwrap() + 30_000;
    // Model actor scheduling after the route selected its deadline.
    std::thread::sleep(std::time::Duration::from_millis(5));
    let (challenge, _) = owner
        .begin_current_member_carrier(
            &credential,
            &ad,
            "fixture/deadline".into(),
            LineSecurity::AuthenticatedEncrypted,
            expiry,
            128,
        )
        .unwrap();
    assert_eq!(challenge.expires_at_millis, expiry);
    let before = owner.session.evidence().clone();
    // Invalid deadlines neither replace nor retire an existing valid issuance.
    for invalid in [
        0,
        crate::durable_host::current_time_millis().unwrap() - 1,
        crate::durable_host::current_time_millis().unwrap() + 3_660_000,
    ] {
        assert!(owner
            .begin_current_member_carrier(
                &credential,
                &ad,
                "fixture/invalid-deadline".into(),
                LineSecurity::AuthenticatedEncrypted,
                invalid,
                128
            )
            .is_err());
    }
    owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .unwrap();
    assert_eq!(owner.session.evidence(), &before);
    owner
        .retire_current_member_carrier(&challenge.session_id)
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
