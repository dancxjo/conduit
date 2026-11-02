use super::*;
use conduit_body::{
    AdmissionManager, AdmissionSigns, BodyMembership, SpawnInvitationSecret, INVITATION_SCHEMA,
    SPAWN_ADMISSION_RECEIPT_SCHEMA,
};

fn write(value: &impl serde::Serialize) -> u32 {
    let bytes = serde_json::to_vec(value).unwrap();
    OUTPUT.with(|output| output.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
    bytes.len() as u32
}
fn read_output<T: serde::de::DeserializeOwned>() -> T {
    let length = OUTPUT_LEN.with(|length| *length.borrow());
    OUTPUT.with(|output| serde_json::from_slice(&output.borrow()[..length]).unwrap())
}
fn initialized() {
    clear_identity_and_output();
    let identity = BrowserAdmissionIdentity::from_csprng_seed(
        "browser/portable".into(),
        "boot/portable".into(),
        [7; 32],
    )
    .unwrap();
    IDENTITY.with(|slot| *slot.borrow_mut() = Some(identity));
}
fn prepare() -> (PortableSpawnAdmissionRequest, PortableAdmissionReceipt) {
    initialized();
    let mut manager = AdmissionManager::new("body/portable".into()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([9; 32]).unwrap(),
            [5; 32],
            100,
            200,
        )
        .unwrap();
    let portable = PortableInvitation {
        schema: INVITATION_SCHEMA.into(),
        claim: invitation.claim(),
        secret: [9; 32],
        rendezvous: None,
    };
    let length = write(&portable);
    assert_eq!(conduit_browser_seed_prepare(length, 101, 0, 1), 0);
    let request: PortableSpawnAdmissionRequest = read_output();
    let mut membership = BodyMembership::new(request.body_id.clone()).unwrap();
    let credential = manager
        .complete_spawn(
            &mut membership,
            &request.host_advertisement,
            &request.admission_proof().unwrap(),
            102,
            AdmissionSigns {
                part_admitted: "sign/part".into(),
                host_attached: "sign/host".into(),
                candidate_admitted: "sign/candidate".into(),
            },
        )
        .unwrap();
    let receipt = PortableAdmissionReceipt {
        schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential,
        host_advertisement: request.host_advertisement.clone(),
        membership_admitted: true,
        current_offers_available: !request.host_advertisement.capabilities.is_empty(),
        plan_created: false,
        play_created: false,
    };
    (request, receipt)
}

#[test]
fn actual_manager_receipt_requires_human_authority_and_is_consumed_once() {
    let (_, receipt) = prepare();
    let length = write(&receipt);
    assert_eq!(conduit_browser_seed_complete(length, 103, 0, 0), REFUSED);
    let length = write(&receipt);
    assert_eq!(conduit_browser_seed_complete(length, 103, 0, 1), 0);
    let credential: MembershipCredential = read_output();
    assert_eq!(credential, receipt.credential);
    let length = write(&receipt);
    assert_eq!(conduit_browser_seed_complete(length, 103, 0, 1), REFUSED);
    assert_eq!(conduit_browser_seed_credential(), 0);
    assert_eq!(read_output::<MembershipCredential>(), credential);
}

#[test]
fn wrong_body_host_boot_offer_and_effect_claims_refuse() {
    let (_, original) = prepare();
    for variant in 0..5 {
        let mut receipt = original.clone();
        match variant {
            0 => receipt.credential.body_id = "body/wrong".into(),
            1 => receipt.credential.host_id = "host/wrong".into(),
            2 => receipt.credential.boot_id = "boot/wrong".into(),
            3 => receipt.host_advertisement.offer_generation.0 += 1,
            _ => receipt.play_created = true,
        }
        let length = write(&receipt);
        assert_eq!(conduit_browser_seed_complete(length, 103, 0, 1), REFUSED);
    }
    let length = write(&original);
    assert_eq!(conduit_browser_seed_complete(length, 103, 0, 1), 0);
}

#[test]
fn expiry_cancel_and_new_identity_do_not_resurrect_pending_request() {
    let (_, receipt) = prepare();
    let length = write(&receipt);
    assert_eq!(conduit_browser_seed_complete(length, 200, 0, 1), REFUSED);
    assert_eq!(conduit_browser_seed_cancel(), 0);
    let length = write(&receipt);
    assert_eq!(conduit_browser_seed_complete(length, 103, 0, 1), REFUSED);
    assert_eq!(conduit_browser_seed_credential(), REFUSED);
    initialized();
    let length = write(&receipt);
    assert_eq!(conduit_browser_seed_complete(length, 103, 0, 1), REFUSED);
}

#[test]
fn expired_or_malformed_invitation_never_creates_request() {
    initialized();
    for document in [
        serde_json::json!({}),
        serde_json::json!({
            "schema": INVITATION_SCHEMA,
            "claim": {"invitation_id":"invite/expired", "body_id":"body/portable", "nonce":vec![5;32], "expires_at_millis":100},
            "secret":vec![9;32], "rendezvous":null,
        }),
    ] {
        let length = write(&document);
        assert_eq!(conduit_browser_seed_prepare(length, 101, 0, 1), REFUSED);
    }
    assert_eq!(conduit_browser_seed_credential(), REFUSED);
}

#[test]
fn cancel_cannot_be_followed_by_a_different_invitation_on_the_same_boot() {
    let _ = prepare();
    assert_eq!(conduit_browser_seed_cancel(), 0);
    let invitation = PortableInvitation {
        schema: INVITATION_SCHEMA.into(),
        claim: SpawnInvitationClaim {
            invitation_id: "invite/different".into(),
            body_id: "body/portable".into(),
            nonce: [6; 32],
            expires_at_millis: 200,
        },
        secret: [8; 32],
        rendezvous: None,
    };
    let length = write(&invitation);
    assert_eq!(conduit_browser_seed_prepare(length, 103, 0, 1), REFUSED);
    assert_eq!(conduit_browser_seed_credential(), REFUSED);
}
