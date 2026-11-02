use super::member_leave::{admit_with_key, owner_fixture};
use super::*;
use crate::durable_host::owner::MemberPresentationBasis;

#[test]
fn member_presentation_rechecks_latest_credential_after_signed_same_boot_return() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, advertisement) = admit_with_key(&mut owner, &root, "host/presentation");
    let original = MemberPresentationBasis::new(credential.clone(), advertisement.clone()).unwrap();
    owner.require_current_native_member(&original).unwrap();
    owner.member_leave(&root, &credential).unwrap();
    assert!(owner.require_current_native_member(&original).is_err());
    // Actual Core proof, deliberately returning with identical Host/Boot/offer:
    // an older credential must still cease to authorize presentation.
    let mut manager = owner.admissions.clone().unwrap();
    let challenge = manager
        .begin_return(
            &owner.session.evidence().membership,
            &credential.part_id,
            &advertisement,
            [79; 32],
            100,
            200,
        )
        .unwrap();
    let proof = conduit_body::PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()),
    };
    let authority = owner.host.advertisement().clone();
    let current = owner
        .session
        .admit_returning_host(
            &mut manager,
            &advertisement,
            &proof,
            101,
            &authority.host_id,
            &authority.boot_id,
        )
        .unwrap();
    assert_ne!(current.credential_id, credential.credential_id);
    owner.admissions = Some(manager);
    owner.persist(&root).unwrap();
    let returned = MemberPresentationBasis::new(current, advertisement).unwrap();
    owner.require_current_native_member(&returned).unwrap();
    assert!(owner.require_current_native_member(&original).is_err());
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    for change in 0..3 {
        let mut forged = returned.clone();
        match change {
            0 => forged.host_advertisement.offer_generation.0 += 1,
            1 => forged.host_advertisement.boot_id = "boot/forged".into(),
            _ => forged.credential.issued_at_millis += 1,
        }
        assert!(owner.require_current_native_member(&forged).is_err());
        assert_eq!(owner.session.evidence(), &before);
        assert_eq!(owner.admissions, manager);
    }
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}
