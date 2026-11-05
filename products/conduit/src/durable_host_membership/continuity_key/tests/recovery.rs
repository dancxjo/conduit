//! Local recovery custody only. Owner acceptance still requires signed return.
use super::*;
use crate::durable_host::membership::recovery::restore_from_proof;

#[test]
fn recovery_restores_original_private_key_without_changing_installed_membership() {
    let f = fixture();
    complete(&f).unwrap();
    let installation = fs::read(f.root.join("installation.json")).unwrap();
    let credential = fs::read(f.root.join("body/membership-credential.json")).unwrap();
    let key = f.root.join("body").join(PATH);
    fs::remove_file(&key).unwrap();
    restore_from_proof(
        &f.root,
        &f.pending.invitation,
        f.pending.request.clone(),
        &f.receipt,
    )
    .unwrap();
    assert!(read(&key).unwrap().secret == [7; 32]);
    assert_eq!(
        fs::read(f.root.join("installation.json")).unwrap(),
        installation
    );
    assert_eq!(
        fs::read(f.root.join("body/membership-credential.json")).unwrap(),
        credential
    );
    // Repeat consumes no invitation, adds no Part and does not replace the key.
    let before = fs::read(&key).unwrap();
    restore_from_proof(
        &f.root,
        &f.pending.invitation,
        f.pending.request.clone(),
        &f.receipt,
    )
    .unwrap();
    assert_eq!(fs::read(key).unwrap(), before);
}

#[test]
fn recovery_refuses_another_part_or_an_unsigned_original_without_writing_a_key() {
    for altered in [0, 1, 2] {
        let f = fixture();
        complete(&f).unwrap();
        let path = f.root.join("body").join(PATH);
        fs::remove_file(&path).unwrap();
        let mut receipt = f.receipt.clone();
        let mut request = f.pending.request.clone();
        let mut invitation = f.pending.invitation.clone();
        match altered {
            0 => receipt.credential.part_id = id("other/part"),
            1 => request.signature[0] ^= 1,
            _ => invitation.secret = [8; 32],
        }
        assert!(restore_from_proof(&f.root, &invitation, request, &receipt).is_err());
        assert!(!path.exists());
    }
}

#[test]
fn recovery_requires_authorization_before_accessing_any_input() {
    use crate::durable_host::membership::recovery::{run, Args};
    let absent = PathBuf::from("/nonexistent/conduit-recovery-input");
    let args = Args {
        state_dir: absent.clone(),
        original_seed: absent.clone(),
        realization_receipt: absent.clone(),
        admission_request: absent.clone(),
        admission_receipt: absent,
        authorize_recovery: false,
    };
    assert_eq!(
        run(args).unwrap_err(),
        "member key recovery requires --authorize-recovery"
    );
}

#[test]
fn recovery_never_replaces_an_existing_different_private_key() {
    let f = fixture();
    complete(&f).unwrap();
    let path = f.root.join("body").join(PATH);
    let before = fs::read(&path).unwrap();
    let mut invitation = f.pending.invitation.clone();
    invitation.secret = [8; 32];
    let mut request = f.pending.request.clone();
    let key = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    request.signature = key
        .sign(&invitation.claim.signing_transcript(
            &request.host_advertisement.host_id,
            &request.host_advertisement.boot_id,
            request.host_advertisement.offer_generation,
        ))
        .to_vec();
    assert!(restore_from_proof(&f.root, &invitation, request, &f.receipt).is_err());
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
#[ignore = "requires original private Seed and independently retained physical receipts"]
fn original_seed_package_verifies_for_identity_recovery() {
    let root = PathBuf::from(
        std::env::var_os("CONDUIT_MEMBER_RECOVERY_EVIDENCE").expect("evidence directory"),
    );
    let mut bytes = fs::read(root.join("hamm-threebee-live.seed")).unwrap();
    let realization: conduit_host_make::DeploymentRealizationReceipt = serde_json::from_slice(
        &fs::read(root.join("threebee-native-install-receipt.json")).unwrap(),
    )
    .unwrap();
    let verified = conduit_body_make::verify_joining_host_package(
        &bytes,
        &realization,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        0,
    );
    bytes.fill(0);
    let package = verified.unwrap_or_else(|error| panic!("original Seed verification: {error}"));
    let request: PortableSpawnAdmissionRequest = serde_json::from_slice(
        &fs::read(root.join("threebee-live-admission-request.json")).unwrap(),
    )
    .unwrap();
    let receipt: PortableAdmissionReceipt = serde_json::from_slice(
        &fs::read(root.join("threebee-live-admission-receipt.json")).unwrap(),
    )
    .unwrap();
    receipt.validate_against(&request).unwrap();
    assert_eq!(
        package.invitation().claim.invitation_id,
        request.invitation_id
    );
    assert_eq!(
        package.invitation().claim.body_id,
        receipt.credential.body_id
    );
    let key = SpawnInvitationSecret::from_csprng_bytes(package.invitation().secret).unwrap();
    let signed = key.sign(&package.invitation().claim.signing_transcript(
        &request.host_advertisement.host_id,
        &request.host_advertisement.boot_id,
        request.host_advertisement.offer_generation,
    ));
    assert!(request.signature == signed);
    let owner = PathBuf::from(
        std::env::var_os("CONDUIT_MEMBER_RECOVERY_OWNER_STATE")
            .expect("independently retained owner state"),
    );
    let records: serde_json::Value =
        serde_json::from_slice(&fs::read(owner.join("body/admission.json")).unwrap()).unwrap();
    let record = records["continuity_keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| {
            record["part_id"].as_str() == Some(receipt.credential.part_id.as_str())
                && record["host_id"].as_str() == Some(receipt.credential.host_id.as_str())
        })
        .expect("actual owner's existing Part key");
    let expected: [u8; 32] = serde_json::from_value(record["verifying_key"].clone()).unwrap();
    assert!(key.verifying_key() == expected);
}

#[test]
fn historical_seed_recovers_same_part_after_a_verified_new_boot_return() {
    let (f, mut manager, mut membership, current, challenge) = returning_fixture(true);
    durable_host::write_json_atomic(&f.root.join("body/pending-join.json"), &f.pending).unwrap();
    complete(&f).unwrap();
    let proof =
        sign_return_challenge(&f.root, &f.receipt.credential, &current, &challenge, 11).unwrap();
    let returned = manager
        .complete_return(
            &mut membership,
            &current,
            &proof,
            12,
            "fixture/recovery-before-loss".into(),
        )
        .unwrap();
    durable_host::membership::retain_returned_membership(
        &f.root,
        &f.receipt.credential,
        &returned,
        &current,
    )
    .unwrap();
    let before = fs::read(f.root.join("body/membership-credential.json")).unwrap();
    fs::remove_file(f.root.join("body").join(PATH)).unwrap();
    restore_from_proof(
        &f.root,
        &f.pending.invitation,
        f.pending.request.clone(),
        &f.receipt,
    )
    .unwrap();
    assert_eq!(
        fs::read(f.root.join("body/membership-credential.json")).unwrap(),
        before
    );
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &returned.part_id,
            &current.boot_id,
            "fixture/recovery-detached".into(),
        )
        .unwrap();
    let next = manager
        .begin_return(&membership, &returned.part_id, &current, [10; 32], 20, 100)
        .unwrap();
    let signed = sign_return_challenge(&f.root, &returned, &current, &next, 21).unwrap();
    assert!(manager
        .complete_return(
            &mut membership,
            &current,
            &signed,
            22,
            "fixture/recovery-after-loss".into()
        )
        .is_ok());
}
