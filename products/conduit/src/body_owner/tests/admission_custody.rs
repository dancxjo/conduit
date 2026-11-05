use super::member_leave::{admit_with_key, owner_fixture};
use super::*;
use conduit_body::{MembershipCredential, PartReturnProof, SpawnInvitationSecret};
use std::path::Path;

fn cycle(
    owner: &mut Owner,
    root: &Path,
    credential: &MembershipCredential,
    key: &SpawnInvitationSecret,
    offer: &mut conduit_core::HostAdvertisement,
    index: u64,
) -> MembershipCredential {
    if owner
        .session
        .evidence()
        .membership
        .parts
        .iter()
        .any(|part| part.part_id == credential.part_id && part.current.is_some())
    {
        owner.member_leave(root, credential).unwrap();
    }
    offer.boot_id = format!("boot/custody/{index}").into();
    offer.offer_generation.0 += 1;
    let challenge = owner.begin_member_return(credential, offer).unwrap();
    let proof = PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()),
    };
    let current = owner.complete_member_return(root, &proof).unwrap();
    // Lost delivery: exact retransmission returns the durable credential once.
    assert_eq!(owner.complete_member_return(root, &proof).unwrap(), current);
    assert_eq!(
        owner.begin_member_return(credential, offer).unwrap(),
        challenge
    );
    current
}

fn reopen(root: &Path) -> Owner {
    let retained = state::load(root).unwrap().unwrap();
    let manager = state::admissions(root, &retained.body_id).unwrap();
    let boot = crate::durable_host::fresh_identity("boot/custody-owner", "reopen");
    let mut owner = Owner::resume(host(&boot), retained).unwrap();
    owner.admissions = manager;
    owner.persist(root).unwrap();
    owner
}

#[test]
fn one_member_returns_one_hundred_times_through_durable_owner_custody() {
    let (mut owner, root) = owner_fixture();
    let (mut current, key, mut offer) = admit_with_key(&mut owner, &root, "host/custody");
    let original = current.clone();
    let mut ids = vec![current.credential_id.clone()];
    for index in 1..=100 {
        current = cycle(&mut owner, &root, &current, &key, &mut offer, index);
        assert_eq!(current.part_id, original.part_id);
        assert!(!ids.contains(&current.credential_id));
        ids.push(current.credential_id.clone());
        let manager = owner.admissions.as_ref().unwrap();
        assert!(manager.receipts.len() <= conduit_body::MAX_ADMISSION_RECEIPTS);
        assert_eq!(manager.receipts[0].credential, original);
        assert_eq!(manager.storage_usage().continuity_keys.len, 1);
        assert_eq!(owner.session.evidence().membership.parts.len(), 2);
        owner = reopen(&root);
        assert!(owner.begin_member_return(&original, &offer).is_err());
        assert_eq!(
            owner
                .admissions
                .as_ref()
                .unwrap()
                .receipts
                .last()
                .unwrap()
                .credential,
            current
        );
    }
    let cold: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("body/admission-custody.json")).unwrap())
            .unwrap();
    assert_eq!(cold["receipts"].as_array().unwrap().len(), 90);
    assert_eq!(owner.admissions.as_ref().unwrap().receipts.len(), 11);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_or_corrupt_cold_evidence_refuses_reopen_and_new_publication() {
    let (mut owner, root) = owner_fixture();
    let (mut current, key, mut offer) = admit_with_key(&mut owner, &root, "host/cold-fault");
    for index in 1..=32 {
        current = cycle(&mut owner, &root, &current, &key, &mut offer, index);
        owner = reopen(&root);
    }
    let file = root.join("body/admission-custody.json");
    let original = std::fs::read(&file).unwrap();
    let manager_bytes = std::fs::read(root.join("body/admission.json")).unwrap();
    std::fs::remove_file(&file).unwrap();
    assert!(state::load(&root).is_err());
    assert!(owner.persist(&root).is_err());
    assert_eq!(
        std::fs::read(root.join("body/admission.json")).unwrap(),
        manager_bytes
    );
    std::fs::write(&file, b"corrupt").unwrap();
    assert!(state::load(&root).is_err());
    assert!(owner.persist(&root).is_err());
    std::fs::write(&file, original).unwrap();
    state::load(&root).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn full_nonretirable_hot_profile_reports_public_pressure_without_mutation() {
    let (mut owner, root) = owner_fixture();
    let (credential, _, _) = admit_with_key(&mut owner, &root, "host/pressure");
    let manager = owner.admissions.as_mut().unwrap();
    let template = manager.receipts[0].clone();
    manager.receipts.clear();
    for index in 0..conduit_body::MAX_ADMISSION_RECEIPTS {
        // Storage-pressure fixture: two protected endpoints per Part.
        // These synthesized rows do not prove member authentication.
        let mut row = template.clone();
        row.admission_id =
            serde_json::from_value(format!("admission/pressure/{index}").into()).unwrap();
        row.credential.credential_id =
            serde_json::from_value(format!("credential/pressure/{index}").into()).unwrap();
        row.credential.part_id = conduit_body::PartId::bind(
            &credential.body_id,
            &format!("host/pressure/{}", index / 2),
            0,
        )
        .unwrap();
        manager.receipts.push(row);
    }
    let before = owner.admissions.clone();
    owner.persist(&root).unwrap();
    let file = root.join("body/admission.json");
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(
        owner.retain_admission_capacity(&root).unwrap_err(),
        "admission-receipt-capacity-exhausted"
    );
    assert_eq!(owner.admissions, before);
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    assert!(!root.join("body/admission-custody.json").exists());
    std::fs::remove_dir_all(root).unwrap();
}
