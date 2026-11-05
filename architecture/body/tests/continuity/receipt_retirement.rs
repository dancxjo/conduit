use super::*;
use conduit_body::{ReceiptRetirementRefusal, MAX_ADMISSION_RECEIPTS, MAX_MEMBERSHIP_EVENTS};

fn returned(
    manager: &mut AdmissionManager,
    membership: &mut BodyMembership,
    key: &SigningKey,
    index: u64,
) {
    let part = membership.parts[0].part_id.clone();
    let current = membership.parts[0]
        .current
        .as_ref()
        .unwrap()
        .boot_id
        .clone();
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &part,
            &current,
            format!("sign/retirement/offline/{index}").into(),
        )
        .unwrap();
    let host = advertisement(&format!("boot/retirement/{index}"), index + 1);
    let now = 10_000 + index * 1_000;
    let challenge = manager
        .begin_return(membership, &part, &host, [index as u8; 32], now, now + 500)
        .unwrap();
    manager
        .complete_return(
            membership,
            &host,
            &return_proof(&challenge, key),
            now + 1,
            format!("sign/retirement/returned/{index}").into(),
        )
        .unwrap();
}

#[test]
fn signed_returns_continue_with_exact_custody_and_serialized_reopen() {
    let (mut manager, mut membership, key) = admitted();
    let original = manager.receipts[0].clone();
    let mut cold = Vec::new();
    let mut events = Vec::new();
    let mut credential_ids = vec![original.credential.credential_id.clone()];
    for index in 1..=100 {
        if membership.events.len() + 2 > MAX_MEMBERSHIP_EVENTS {
            events.push(membership.checkpoint_events().unwrap());
        }
        if manager.receipts.len() == MAX_ADMISSION_RECEIPTS {
            let before = manager.clone();
            let selection = manager.prepare_receipt_retirement().unwrap().unwrap();
            assert_eq!(manager, before, "preparation must release nothing");
            assert_eq!(selection.body_id(), &manager.body_id);
            let latest = manager.receipts.last().unwrap().clone();
            assert!(!selection.receipts().contains(&original));
            assert!(!selection.receipts().contains(&latest));
            // In-memory custody model only: product file durability is separate.
            cold.extend_from_slice(selection.receipts());
            manager.acknowledge_receipt_custody(&selection).unwrap();
            assert_eq!(manager.receipts, vec![original.clone(), latest]);
            let before_replay = manager.clone();
            assert_eq!(
                manager.acknowledge_receipt_custody(&selection),
                Err(ReceiptRetirementRefusal::StaleSelection)
            );
            assert_eq!(manager, before_replay);
            manager = serde_json::from_slice(&serde_json::to_vec(&manager).unwrap()).unwrap();
        }
        returned(&mut manager, &mut membership, &key, index);
        let latest = &manager.receipts.last().unwrap().credential;
        assert!(!credential_ids.contains(&latest.credential_id));
        credential_ids.push(latest.credential_id.clone());
        assert_eq!(membership.parts.len(), 1);
        assert_eq!(manager.storage_usage().continuity_keys.len, 1);
        assert_eq!(manager.receipts[0], original);
        assert!(manager.receipts.len() <= MAX_ADMISSION_RECEIPTS);
    }
    assert!(!events.is_empty());
    assert_eq!(cold.len() + manager.receipts.len(), 101);
}

#[test]
fn changed_history_and_foreign_selection_cannot_release_receipts() {
    let (mut manager, mut membership, key) = admitted();
    returned(&mut manager, &mut membership, &key, 1);
    assert!(manager.prepare_receipt_retirement().unwrap().is_none());
    returned(&mut manager, &mut membership, &key, 2);
    let selection = manager.prepare_receipt_retirement().unwrap().unwrap();
    returned(&mut manager, &mut membership, &key, 3);
    let before = manager.clone();
    assert_eq!(
        manager.acknowledge_receipt_custody(&selection),
        Err(ReceiptRetirementRefusal::StaleSelection)
    );
    assert_eq!(manager, before);
    let mut foreign = manager.clone();
    foreign.body_id = Body::born(
        SourceDocumentId::from("source/another-body"),
        CheckedPlotId::from("checked/another-body"),
        1,
        SignId::from("sign/another-body-born"),
    )
    .unwrap()
    .body_id;
    assert_eq!(
        foreign.prepare_receipt_retirement(),
        Err(ReceiptRetirementRefusal::InvalidHistory)
    );
}

#[test]
fn legacy_json_keeps_ordinal_and_exhaustion_refuses_without_mutation() {
    let (manager, mut membership, key) = admitted();
    let mut value = serde_json::to_value(&manager).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("retired_receipt_count");
    let restored: AdmissionManager = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(restored, manager);
    value["retired_receipt_count"] = serde_json::json!(u64::MAX);
    let mut exhausted: AdmissionManager = serde_json::from_value(value).unwrap();
    let part = membership.parts[0].part_id.clone();
    let current = membership.parts[0]
        .current
        .as_ref()
        .unwrap()
        .boot_id
        .clone();
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &part,
            &current,
            "sign/exhausted-offline".into(),
        )
        .unwrap();
    let host = advertisement("boot/exhausted", 2);
    let challenge = exhausted
        .begin_return(
            &membership,
            &membership.parts[0].part_id,
            &host,
            [8; 32],
            10_000,
            10_500,
        )
        .unwrap();
    let before = exhausted.clone();
    let before_membership = membership.clone();
    assert_eq!(
        exhausted.complete_return(
            &mut membership,
            &host,
            &return_proof(&challenge, &key),
            10_001,
            "sign/exhausted".into(),
        ),
        Err(AdmissionRefusal::ReceiptSequenceExhausted)
    );
    assert_eq!(exhausted, before);
    assert_eq!(membership, before_membership);
}

#[test]
fn new_part_identity_uses_lifetime_issuance_after_retirement() {
    let (mut manager, mut membership, key) = admitted();
    returned(&mut manager, &mut membership, &key, 1);
    returned(&mut manager, &mut membership, &key, 2);
    let selection = manager.prepare_receipt_retirement().unwrap().unwrap();
    manager.acknowledge_receipt_custody(&selection).unwrap();
    assert_eq!(manager.receipts.len(), 2);
    let mut host = advertisement("boot/second-part", 1);
    host.host_id = HostId::from("host/second-part");
    let mut candidates = CandidateInventory::new(manager.body_id.clone()).unwrap();
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement: host,
            friendly_label: "second".into(),
            observed_binding_id: "line/second".into(),
            observation_sign_id: "sign/second-observed".into(),
            proof_id: DiscoveryProofId::bind("proof/second-discovery").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate,
            key.verifying_key().to_bytes(),
            [9; 32],
            20_000,
            20_500,
            "sign/second-requested".into(),
        )
        .unwrap();
    let expected =
        conduit_body::PartId::bind(&manager.body_id, challenge.admission_id.as_str(), 3).unwrap();
    let proof = AmbientAdmissionProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    };
    let credential = manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            20_001,
            AdmissionSigns {
                part_admitted: "sign/second-part".into(),
                host_attached: "sign/second-attached".into(),
                candidate_admitted: "sign/second-admitted".into(),
            },
        )
        .unwrap();
    assert_eq!(credential.part_id, expected);
}
