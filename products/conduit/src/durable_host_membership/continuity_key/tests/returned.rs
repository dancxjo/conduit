//! Existing returned credential custody and substitution.
use super::*;
#[test]
fn returned_credential_switch_is_recoverable_and_preserves_old_file_and_private_key() {
    let f = fixture();
    complete(&f).unwrap();
    let before_install =
        durable_host::read_installation(&f.root.join("installation.json")).unwrap();
    let old_path = before_install
        .joined_body_state
        .as_ref()
        .unwrap()
        .credential_path
        .clone();
    let old_bytes = fs::read(&old_path).unwrap();
    let key_path = f.root.join("body").join(PATH);
    let key_bytes = fs::read(&key_path).unwrap();
    let mut actual = f.receipt.host_advertisement.clone();
    actual.boot_id = "fixture/returned".into();
    actual.offer_generation = OfferGeneration(2);
    let mut returned = f.receipt.credential.clone();
    returned.credential_id = id("fixture/returned-credential");
    returned.boot_id = actual.boot_id.clone();
    // Interrupt precisely after immutable publication, before pointer commit.
    let obstruction = f.root.join("installation.tmp");
    fs::create_dir(&obstruction).unwrap();
    assert!(crate::durable_host::membership::retain_returned_membership(
        &f.root,
        &f.receipt.credential,
        &returned,
        &actual
    )
    .is_err());
    let interrupted = durable_host::read_installation(&f.root.join("installation.json")).unwrap();
    assert!(
        crate::durable_host::membership::load_joined_credential(&f.root, &interrupted).unwrap()
            == f.receipt.credential
    );
    fs::remove_dir(&obstruction).unwrap();
    crate::durable_host::membership::retain_returned_membership(
        &f.root,
        &f.receipt.credential,
        &returned,
        &actual,
    )
    .unwrap();
    let reopened = durable_host::read_installation(&f.root.join("installation.json")).unwrap();
    assert!(
        crate::durable_host::membership::load_joined_credential(&f.root, &reopened).unwrap()
            == returned
    );
    assert_ne!(
        reopened.joined_body_state.as_ref().unwrap().credential_path,
        old_path
    );
    assert_eq!(fs::read(old_path).unwrap(), old_bytes);
    assert_eq!(fs::read(key_path).unwrap(), key_bytes);
    // Stale prior cannot supersede an already committed replacement.
    assert!(crate::durable_host::membership::retain_returned_membership(
        &f.root,
        &f.receipt.credential,
        &returned,
        &actual
    )
    .is_err());
}

#[test]
fn returned_credential_substitution_refuses_without_touching_retained_binding() {
    let f = fixture();
    complete(&f).unwrap();
    let original = fs::read(f.root.join("installation.json")).unwrap();
    for variant in 0..5 {
        let mut actual = f.receipt.host_advertisement.clone();
        actual.boot_id = "fixture/returned".into();
        actual.offer_generation = OfferGeneration(2);
        let mut returned = f.receipt.credential.clone();
        returned.credential_id = id("fixture/returned-credential");
        returned.boot_id = actual.boot_id.clone();
        match variant {
            0 => returned.body_id = id("another/body"),
            1 => returned.part_id = id("another/part"),
            2 => returned.host_id = "another/host".into(),
            3 => returned.boot_id = "another/boot".into(),
            _ => actual.offer_generation.0 = 0,
        }
        assert!(crate::durable_host::membership::retain_returned_membership(
            &f.root,
            &f.receipt.credential,
            &returned,
            &actual
        )
        .is_err());
        assert_eq!(
            fs::read(f.root.join("installation.json")).unwrap(),
            original
        );
    }
}
