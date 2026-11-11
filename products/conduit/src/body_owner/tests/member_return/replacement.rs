//! Authenticated current-Boot replacement and durable refusal boundaries.
use super::*;

#[test]
fn member_return_replaces_current_boot_only_after_proof_and_durable_commit() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, old) = admit_with_key(&mut owner, &root, "host/restarting");
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    let offers = owner.current_offer_summary();
    let old_member =
        crate::durable_host::owner::MemberPresentationBasis::new(prior.clone(), old.clone())
            .unwrap();
    owner.require_current_native_member(&old_member).unwrap();
    assert!(owner.begin_member_return(&prior, &old).is_err());
    let mut replacement = old.clone();
    replacement.boot_id = "boot/restarting/2".into();
    replacement.offer_generation.0 += 1;
    let challenge = owner.begin_member_return(&prior, &replacement).unwrap();
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.current_offer_summary(), offers);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    owner.require_current_native_member(&old_member).unwrap();
    let proof = prove(&challenge, &key);
    let mut forged = proof.clone();
    forged.signature[0] ^= 1;
    assert!(owner.complete_member_return(&root, &forged).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.current_offer_summary(), offers);
    owner.require_current_native_member(&old_member).unwrap();
    let blocked = root.join("replacement-commit-blocked");
    std::fs::write(&blocked, b"occupied").unwrap();
    assert!(owner.complete_member_return(&blocked, &proof).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(owner.current_offer_summary(), offers);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    owner.require_current_native_member(&old_member).unwrap();
    let returned = owner.complete_member_return(&root, &proof).unwrap();
    assert_eq!(returned.part_id, prior.part_id);
    assert_eq!(returned.host_id, prior.host_id);
    assert_eq!(returned.boot_id, replacement.boot_id);
    assert!(owner.require_current_native_member(&old_member).is_err());
    assert!(owner.require_observed_member_offer(&old).is_err());
    assert!(owner.require_observed_member_offer(&replacement).is_ok());
    let committed = owner.session.evidence().clone();
    assert_eq!(state::load(&root).unwrap().unwrap(), committed);
    assert_eq!(
        owner.complete_member_return(&root, &proof).unwrap(),
        returned
    );
    assert_eq!(owner.session.evidence(), &committed);
    assert!(owner.complete_member_return(&root, &forged).is_err());

    // Once another authenticated Boot replaces this one, neither the old
    // credential nor the preceding exact proof can displace the latest Boot.
    owner.pending_member_return = None;
    replacement.boot_id = "boot/restarting/3".into();
    replacement.offer_generation.0 += 1;
    assert!(owner.begin_member_return(&prior, &replacement).is_err());
    let challenge = owner.begin_member_return(&returned, &replacement).unwrap();
    assert!(owner.complete_member_return(&root, &proof).is_err());
    let latest = owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .unwrap();
    assert_eq!(latest.part_id, prior.part_id);
    assert_eq!(latest.boot_id, replacement.boot_id);
    let committed = owner.session.evidence().clone();
    assert!(owner.complete_member_return(&root, &proof).is_err());
    assert_eq!(owner.session.evidence(), &committed);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn member_return_replacement_refuses_changed_attachment_and_foreign_host() {
    let (mut owner, root) = owner_fixture();
    let (prior, key, mut replacement) = admit_with_key(&mut owner, &root, "host/replacement-race");
    let before = owner.session.evidence().clone();
    replacement.boot_id = "boot/replacement-race/2".into();
    let mut foreign = replacement.clone();
    foreign.host_id = "host/foreign-replacement".into();
    assert!(owner.begin_member_return(&prior, &foreign).is_err());
    assert_eq!(owner.session.evidence(), &before);
    let challenge = owner.begin_member_return(&prior, &replacement).unwrap();
    // A distinct authorized withdrawal is not the condition accepted when
    // preparing this replacement. It needs a new finite return challenge.
    owner.member_leave(&root, &prior).unwrap();
    let withdrawn = owner.session.evidence().clone();
    assert!(owner
        .complete_member_return(&root, &prove(&challenge, &key))
        .is_err());
    assert_eq!(owner.session.evidence(), &withdrawn);
    assert_eq!(state::load(&root).unwrap().unwrap(), withdrawn);
    std::fs::remove_dir_all(root).unwrap();
}
