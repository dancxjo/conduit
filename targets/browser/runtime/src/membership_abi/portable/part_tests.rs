use super::*;
use conduit_body::{
    AdmissionManager, AdmissionSigns, Body, BodyMembership, PartReturnProof, SpawnAdmissionProof,
};
use conduit_core::{CheckedPlotId, HostAdvertisement, SignId, SourceDocumentId};
use ed25519_dalek::{Signer, SigningKey};
fn input(value: &impl serde::Serialize) -> usize {
    let mut bytes = serde_json::to_vec(value).unwrap();
    assert!(bytes.len() <= INPUT_CAPACITY);
    INPUT.with(|slot| slot.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
    let length = bytes.len();
    bytes.fill(0);
    length
}
fn output() -> Vec<u8> {
    OUTPUT.with(|slot| slot.borrow()[..OUTPUT_LEN.with(|length| *length.borrow())].to_vec())
}
fn initialize(host: &str, boot: &str) -> [u8; 32] {
    let mut bytes = [host.as_bytes(), boot.as_bytes(), &[7; 32]].concat();
    INPUT.with(|slot| slot.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
    assert_eq!(
        conduit_browser_membership_initialize(host.len() as u32, boot.len() as u32),
        STATUS_READY
    );
    bytes.fill(0);
    output().try_into().unwrap()
}
fn advertisement() -> HostAdvertisement {
    assert_eq!(conduit_browser_membership_advertisement(), STATUS_READY);
    serde_json::from_slice(&output()).unwrap()
}
struct Root {
    manager: AdmissionManager,
    membership: BodyMembership,
    claim: SpawnInvitationClaim,
    root: SigningKey,
    request: PortableMembershipAdmissionRequest,
}
fn prepare() -> Root {
    initialize("browser/own-host", "boot/browser/first");
    let body = Body::born(
        SourceDocumentId::from("source/part-return"),
        CheckedPlotId::from("checked/part-return"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([9; 32]).unwrap(),
            [5; 32],
            100,
            90_000,
        )
        .unwrap();
    let claim = invitation.claim();
    let root = SigningKey::from_bytes(&[11; 32]);
    let envelope = serde_json::json!({ "claim":claim,"secret":vec![9u8;32],
        "expected_body_id":body.body_id, "expected_root":root.verifying_key().to_bytes(),
        "producer_host":"esp32/actual-root", "producer_boot":"boot/actual-root" });
    let length = input(&envelope);
    assert_eq!(
        conduit_browser_membership_prepare_scope(length as u32),
        STATUS_READY
    );
    let request = PortableMembershipAdmissionRequest::decode(&output()).unwrap();
    Root {
        manager,
        membership: BodyMembership::new(body.body_id).unwrap(),
        claim,
        root,
        request,
    }
}
fn adopt(root: &mut Root) -> PortableMembershipAdmissionReceipt {
    let request = &root.request;
    let credential = root
        .manager
        .complete_spawn(
            &mut root.membership,
            &advertisement(),
            &SpawnAdmissionProof {
                invitation_id: request.invitation_id.clone(),
                body_id: request.body_id.clone(),
                host_id: request.host_id.clone(),
                boot_id: request.boot_id.clone(),
                nonce: request.nonce,
                signature: request.signature,
            },
            150,
            AdmissionSigns {
                part_admitted: "sign/admit".into(),
                host_attached: "sign/attach".into(),
                candidate_admitted: "sign/consumed".into(),
            },
        )
        .unwrap();
    // The fixture signs only after the canonical authority accepted the exact
    // request. Serialization simulates retention, not physical flash custody.
    let persisted = serde_json::to_vec(&root.manager).unwrap();
    root.manager = serde_json::from_slice(&persisted).unwrap();
    let mut receipt = PortableMembershipAdmissionReceipt {
        schema: conduit_body::MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA.into(),
        protocol_version: conduit_core::PROTOCOL_VERSION,
        credential,
        membership_revision: root.membership.revision.0,
        request_digest: request.digest(),
        producer_host: "esp32/actual-root".into(),
        producer_boot: "boot/actual-root".into(),
        producer_key: root.root.verifying_key().to_bytes(),
        invitation_id: request.invitation_id.clone(),
        offer_generation: request.offer_generation,
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
        signature: [0; 64],
    };
    receipt.signature = root.root.sign(&receipt.transcript()).to_bytes();
    let length = input(&receipt);
    assert_eq!(
        conduit_browser_membership_adopt_scope(length as u32),
        STATUS_READY
    );
    receipt
}
fn export(root: &Root) -> serde_json::Value {
    let length = input(&serde_json::json!({ "expected_body_id":root.claim.body_id,
        "expected_root":root.root.verifying_key().to_bytes() }));
    assert_eq!(
        part::conduit_browser_membership_export_part(length as u32),
        STATUS_READY
    );
    let mut bytes = output();
    assert!(bytes.len() <= 3072);
    let record = serde_json::from_slice(&bytes).unwrap();
    bytes.fill(0);
    record
}
fn restore(root: &Root, record: &serde_json::Value) -> i32 {
    let length = input(
        &serde_json::json!({ "record":record, "expected_body_id":root.claim.body_id,
        "expected_root":root.root.verifying_key().to_bytes() }),
    );
    let result = part::conduit_browser_membership_import_part(length as u32);
    assert!(INPUT.with(|slot| slot.borrow()[..length].iter().all(|byte| *byte == 0)));
    result
}
#[test]
fn canonical_root_accepts_retained_part_key_return_from_fresh_boot_without_replacing_host_key() {
    let mut root = prepare();
    let host_key = IDENTITY.with(|slot| slot.borrow().as_ref().unwrap().verifying_key());
    let receipt = adopt(&mut root);
    let record = export(&root);
    let fresh_host_key = initialize("browser/own-host", "boot/browser/second");
    assert_eq!(fresh_host_key, host_key);
    assert_eq!(restore(&root, &record), STATUS_READY);
    assert_eq!(
        IDENTITY.with(|slot| slot.borrow().as_ref().unwrap().verifying_key()),
        host_key
    );
    root.membership
        .observe_offline(
            &root.claim.body_id,
            root.membership.revision,
            &receipt.credential.part_id,
            &receipt.credential.boot_id,
            "sign/lost".into(),
        )
        .unwrap();
    let fresh = advertisement();
    let challenge = root
        .manager
        .begin_return(
            &root.membership,
            &receipt.credential.part_id,
            &fresh,
            [6; 32],
            100_000,
            100_100,
        )
        .unwrap(); // original invitation has expired
    let length = input(&challenge);
    assert_eq!(
        conduit_browser_membership_prove_return(length as u32),
        STATUS_READY
    );
    let signature = output().try_into().unwrap();
    let proof = PartReturnProof {
        admission_id: challenge.admission_id,
        body_id: challenge.body_id,
        part_id: challenge.part_id,
        host_id: challenge.host_id,
        boot_id: challenge.boot_id,
        nonce: challenge.nonce,
        signature,
    };
    let returned = root
        .manager
        .complete_return(
            &mut root.membership,
            &fresh,
            &proof,
            100_050,
            "sign/return".into(),
        )
        .unwrap();
    assert_eq!(returned.part_id, receipt.credential.part_id);
    assert_ne!(returned.credential_id, receipt.credential.credential_id);
    assert_eq!(returned.boot_id.as_str(), "boot/browser/second");
}
#[test]
fn wrong_seed_claim_receipt_body_root_or_host_releases_no_part_signer() {
    let mut root = prepare();
    adopt(&mut root);
    let record = export(&root);
    for mode in 0..6 {
        initialize(
            if mode == 5 {
                "browser/foreign"
            } else {
                "browser/own-host"
            },
            "boot/browser/fresh",
        );
        let mut envelope = serde_json::json!({ "record":record, "expected_body_id":root.claim.body_id,
            "expected_root":root.root.verifying_key().to_bytes() });
        match mode {
            0 => envelope["record"]["secret"][0] = 10.into(),
            1 => envelope["record"]["claim"]["nonce"][0] = 7.into(),
            2 => envelope["record"]["receipt"]["credential"]["part_id"] = "part/foreign".into(),
            3 => envelope["expected_body_id"] = "body/foreign".into(),
            4 => {
                envelope["expected_root"] = serde_json::to_value(
                    SigningKey::from_bytes(&[12; 32]).verifying_key().to_bytes(),
                )
                .unwrap()
            }
            _ => {}
        }
        let length = input(&envelope);
        assert_ne!(
            part::conduit_browser_membership_import_part(length as u32),
            STATUS_READY
        );
        assert!(PENDING.with(|slot| slot.borrow().is_none()));
        assert_eq!(OUTPUT_LEN.with(|length| *length.borrow()), 0);
    }
}
#[test]
fn unendorsed_export_and_wrong_scope_retirement_refuse_and_exact_retirement_erases_authority() {
    let mut root = prepare();
    let length = input(&serde_json::json!({ "expected_body_id":root.claim.body_id,
        "expected_root":root.root.verifying_key().to_bytes() }));
    assert_eq!(
        part::conduit_browser_membership_export_part(length as u32),
        ERROR_MEMBERSHIP
    );
    let receipt = adopt(&mut root);
    let wrong =
        input(&serde_json::json!({ "body_id":root.claim.body_id, "part_id":"part/foreign" }));
    assert_ne!(
        part::conduit_browser_membership_retire_scope(wrong as u32),
        STATUS_READY
    );
    assert!(PENDING.with(|slot| slot.borrow().as_ref().unwrap().receipt.is_some()));
    let length = input(
        &serde_json::json!({ "body_id":root.claim.body_id, "part_id":receipt.credential.part_id }),
    );
    assert_eq!(
        part::conduit_browser_membership_retire_scope(length as u32),
        STATUS_READY
    );
    assert!(PENDING.with(|slot| slot.borrow().is_none()));
    assert_eq!(OUTPUT_LEN.with(|length| *length.borrow()), 0);
}

#[test]
fn selected_part_refuses_cross_scope_and_retired_key_cannot_pass_root_return_verification() {
    let mut root = prepare();
    let receipt = adopt(&mut root);
    let record = export(&root);
    initialize("browser/own-host", "boot/browser/fresh");
    assert_eq!(restore(&root, &record), STATUS_READY);
    root.membership
        .observe_offline(
            &root.claim.body_id,
            root.membership.revision,
            &receipt.credential.part_id,
            &receipt.credential.boot_id,
            "sign/lost-scope".into(),
        )
        .unwrap();
    let fresh = advertisement();
    let challenge = root
        .manager
        .begin_return(
            &root.membership,
            &receipt.credential.part_id,
            &fresh,
            [6; 32],
            200,
            1000,
        )
        .unwrap();
    for mode in 0..6 {
        let mut wrong = challenge.clone();
        match mode {
            0 => wrong.body_id = serde_json::from_str("\"body/foreign\"").unwrap(),
            1 => wrong.part_id = serde_json::from_str("\"part/foreign\"").unwrap(),
            2 => wrong.host_id = "browser/foreign".into(),
            3 => wrong.boot_id = "boot/foreign".into(),
            4 => wrong.offer_generation = OfferGeneration(2),
            _ => wrong.nonce = [0; 32],
        }
        let length = input(&wrong);
        assert_eq!(
            conduit_browser_membership_prove_return(length as u32),
            ERROR_CHALLENGE
        );
        assert_eq!(OUTPUT_LEN.with(|length| *length.borrow()), 0);
    }
    let length = input(
        &serde_json::json!({ "body_id":root.claim.body_id, "part_id":receipt.credential.part_id }),
    );
    assert_eq!(
        part::conduit_browser_membership_retire_scope(length as u32),
        STATUS_READY
    );
    let length = input(&challenge);
    // Ambient Host behavior remains available after local Part retirement;
    // its different Host key cannot impersonate the retired Part at the Root.
    assert_eq!(
        conduit_browser_membership_prove_return(length as u32),
        STATUS_READY
    );
    let proof = PartReturnProof {
        admission_id: challenge.admission_id,
        body_id: challenge.body_id,
        part_id: challenge.part_id,
        host_id: challenge.host_id,
        boot_id: challenge.boot_id,
        nonce: challenge.nonce,
        signature: output().try_into().unwrap(),
    };
    assert!(root
        .manager
        .complete_return(
            &mut root.membership,
            &fresh,
            &proof,
            250,
            "sign/rejected".into()
        )
        .is_err());
}
