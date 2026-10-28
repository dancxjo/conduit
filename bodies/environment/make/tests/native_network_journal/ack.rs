//! Actual canonical admission and pinned native snapshot; mocked NOR, no device proof.
use super::journal;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_network_custody_wire.rs"]
mod custody_wire;
use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, BodyMembership, CandidateInventory,
    CandidateObservation, DiscoveryProofId, PartReturnProof,
};
use conduit_core::{
    BootId, HostAdvertisement, HostId, HostProfileId, LinkBindingId, OfferGeneration, SignId,
    PROTOCOL_VERSION,
};
use ed25519_dalek::{Signer, SigningKey};

fn advertisement(boot: &str, generation: u64) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/durable-browser"),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("profile/browser"),
        bases: vec![],
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

fn admitted(body_id: conduit_body::BodyId) -> (AdmissionManager, BodyMembership, SigningKey) {
    let mut candidates = CandidateInventory::new(body_id.clone()).unwrap();
    let first = advertisement("boot/old", 1);
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement: first,
            friendly_label: "browser".into(),
            observed_binding_id: LinkBindingId::from("line/continuity"),
            observation_sign_id: SignId::from("sign/continuity-observed"),
            proof_id: DiscoveryProofId::bind("proof/continuity-discovery").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    let key = SigningKey::from_bytes(&[61; 32]);
    let mut manager = AdmissionManager::new(body_id.clone()).unwrap();
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate,
            key.verifying_key().to_bytes(),
            [62; 32],
            1_000,
            2_000,
            SignId::from("sign/continuity-requested"),
        )
        .unwrap();
    let proof = AmbientAdmissionProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    };
    let mut membership = BodyMembership::new(body_id).unwrap();
    manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            1_001,
            AdmissionSigns {
                part_admitted: SignId::from("sign/continuity-part"),
                host_attached: SignId::from("sign/continuity-host"),
                candidate_admitted: SignId::from("sign/continuity-candidate"),
            },
        )
        .unwrap();
    (manager, membership, key)
}

fn request_bytes(
    draft: &journal::snapshot::Draft,
    manager: &AdmissionManager,
    membership: &BodyMembership,
    key: &SigningKey,
) -> [u8; 628] {
    use journal::snapshot::ack::Request;
    let credential = manager
        .latest_completed_credential(&membership.parts[0].part_id)
        .unwrap();
    let unsigned = Request::for_credential(draft.transcript(), credential, [0; 64]).unwrap();
    let message = Request::decode(&unsigned)
        .unwrap()
        .transcript(&membership.body_id);
    Request::for_credential(
        draft.transcript(),
        credential,
        key.sign(&message).to_bytes(),
    )
    .unwrap()
}
pub(super) fn exercise(body: conduit_body::BodyId, root: &SigningKey) {
    use journal::snapshot::{
        ack::{self, Request},
        Scope, Verifier,
    };
    let boot = BootId::from("boot/current");
    let make_draft = |epoch| {
        let mut verifier = Verifier::start(journal::inspect().unwrap());
        while !verifier.step(&mut [0; 512]).unwrap() {}
        verifier
            .finish()
            .unwrap()
            .offer(
                Scope {
                    body: &body,
                    boot: &boot,
                    epoch,
                    predecessor: [0x42; 32],
                },
                root.verifying_key().to_bytes(),
            )
            .unwrap()
    };
    let draft = make_draft(7);
    let (manager, mut membership, member) = admitted(body.clone());
    let bytes = request_bytes(&draft, &manager, &membership, &member);
    let request = Request::decode(&bytes).unwrap();
    let pending = ack::verify(&draft, &manager, &membership, &request).unwrap();
    assert_eq!(pending.membership_revision(), membership.revision.0);
    assert_eq!(pending.root(), &root.verifying_key().to_bytes());
    let signature = root.sign(&pending.transcript()).to_bytes();
    let signed = pending.signed(signature);
    assert!(request.matches_pending(&draft, &body, signed.bytes()));
    assert!(request.matches_receipt(&body, signed.bytes()));
    for index in [24, 168, 200, 264, 328] {
        let mut changed = *signed.bytes();
        changed[index] ^= 1;
        assert!(!request.matches_receipt(&body, &changed));
    }

    let mut other = *signed.bytes();
    other[264] ^= 1;
    assert!(!request.matches_pending(&draft, &body, &other));
    let cursor = custody_wire::Cursor::decode(*signed.bytes());
    cursor.validate(&body, &root.verifying_key()).unwrap();
    assert!(cursor.offer_chain(&body, &root.verifying_key()).is_err());
    assert_eq!(&cursor.bytes()[232..264], member.verifying_key().as_bytes());
    assert_eq!(signed.offer(), draft.transcript());
    assert_eq!(signed.membership_revision(), membership.revision.0);
    assert!(journal::inspect().is_err());
    drop(cursor);
    assert!(journal::inspect().is_err());
    drop(signed);
    // Alter every signed byte: malformed encoding or canonical verification must refuse.
    for index in 8..628 {
        let mut altered = bytes;
        altered[index] ^= 1;
        if let Ok(request) = Request::decode(&altered) {
            assert!(
                ack::verify(&draft, &manager, &membership, &request).is_err(),
                "byte {index}"
            );
        }
    }
    assert!(Request::decode(&bytes[..627]).is_err());
    let mut altered = bytes;
    altered[0] ^= 1;
    assert!(Request::decode(&altered).is_err());
    for index in 48..52 {
        let mut altered = bytes;
        altered[index] = 0;
        assert!(Request::decode(&altered).is_err());
    }
    for malformed in [0, 0xff] {
        let mut altered = bytes;
        altered[52] = malformed;
        assert!(Request::decode(&altered).is_err());
    }
    let empty = AdmissionManager::new(body.clone()).unwrap();
    assert!(ack::verify(&draft, &empty, &membership, &request).is_err());
    let mut missing_key = empty;
    missing_key.receipts = manager.receipts.clone();
    assert!(ack::verify(&draft, &missing_key, &membership, &request).is_err());
    let mut revoked = membership.clone();
    revoked
        .revoke(
            &body,
            revoked.revision,
            &membership.parts[0].part_id,
            SignId::from("sign/custody-revoked"),
        )
        .unwrap();
    assert!(ack::verify(&draft, &manager, &revoked, &request).is_err());
    let foreign: conduit_body::BodyId = serde_json::from_str("\"body/foreign-custody\"").unwrap();
    let (foreign_manager, foreign_membership, _) = admitted(foreign);
    assert!(ack::verify(&draft, &foreign_manager, &foreign_membership, &request).is_err());
    let pending = ack::verify(&draft, &manager, &membership, &request).unwrap();
    let invalid = pending.signed([0; 64]);
    assert!(custody_wire::Cursor::decode(*invalid.bytes())
        .validate(&body, &root.verifying_key())
        .is_err());
    drop(invalid);
    let part = membership.parts[0].part_id.clone();
    let before = membership.revision;
    membership
        .observe_offline(
            &body,
            before,
            &part,
            &BootId::from("boot/old"),
            SignId::from("sign/member-offline"),
        )
        .unwrap();
    assert!(!membership.parts[0].is_present());
    // Same admitted credential remains valid while detached, without restoring presence.
    let pending = ack::verify(&draft, &manager, &membership, &request).unwrap();
    assert_eq!(pending.membership_revision(), membership.revision.0);
    assert_ne!(pending.membership_revision(), before.0);
    drop(pending);
    assert!(!membership.parts[0].is_present());
    let mut manager: AdmissionManager =
        serde_json::from_slice(&serde_json::to_vec(&manager).unwrap()).unwrap();
    let mut membership: BodyMembership =
        serde_json::from_slice(&serde_json::to_vec(&membership).unwrap()).unwrap();
    assert!(ack::verify(&draft, &manager, &membership, &request).is_ok());
    let returned = advertisement("boot/fresh", 1);
    let challenge = manager
        .begin_return(&membership, &part, &returned, [63; 32], 2100, 3100)
        .unwrap();
    let proof = PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: member.sign(&challenge.signing_transcript()).to_bytes(),
    };
    manager
        .complete_return(
            &mut membership,
            &returned,
            &proof,
            2101,
            SignId::from("sign/member-returned"),
        )
        .unwrap();
    assert!(ack::verify(&draft, &manager, &membership, &request).is_err());
    let fresh = request_bytes(&draft, &manager, &membership, &member);
    assert!(ack::verify(
        &draft,
        &manager,
        &membership,
        &Request::decode(&fresh).unwrap()
    )
    .is_ok());
    drop(draft);
    assert!(journal::inspect().is_ok());
    let draft = make_draft(u64::MAX);
    let bytes = request_bytes(&draft, &manager, &membership, &member);
    assert!(ack::verify(
        &draft,
        &manager,
        &membership,
        &Request::decode(&bytes).unwrap()
    )
    .is_err());
    drop(draft);
    assert!(journal::inspect().is_ok());
}
