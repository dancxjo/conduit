//! Synthetic local custody tests. Receipt correlation does not authenticate an
//! owner. Return tests exercise real Core proof verification; no live transport,
//! independently authenticated owner, Line or execution authority is claimed.
use super::*;
use crate::durable_host::{self, invitation::PortableAdmissionReceipt};
use conduit_body::{PortableInvitation, PortableSpawnAdmissionRequest, SpawnInvitationClaim};
use conduit_core::{HostProfileId, OfferGeneration, PROTOCOL_VERSION};
use std::path::PathBuf;

fn id<T: serde::de::DeserializeOwned>(text: &str) -> T {
    serde_json::from_value(serde_json::Value::String(text.into())).unwrap()
}

struct Fixture {
    root: PathBuf,
    pending: PendingBodyJoin,
    receipt: PortableAdmissionReceipt,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.root.parent().unwrap());
    }
}
fn fixture() -> Fixture {
    let (manifest, root) = durable_host::tests::fixture();
    let installation = durable_host::install_without_start(&manifest, &root).unwrap();
    fs::create_dir_all(root.join("body")).unwrap();
    durable_host::restrict_directory(&root.join("body")).unwrap();
    let advertisement = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: installation.host_id.as_str().into(),
        boot_id: "fixture/current-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("fixture/host"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let invitation = PortableInvitation {
        schema: conduit_body::INVITATION_SCHEMA.into(),
        claim: SpawnInvitationClaim {
            invitation_id: id("fixture/invitation"),
            body_id: id("fixture/body"),
            nonce: [3; 32],
            expires_at_millis: 1000,
        },
        secret: [7; 32],
        rendezvous: None,
    };
    let key = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let signature = key.sign(&invitation.claim.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    ));
    let pending = PendingBodyJoin {
        schema: "conduit.body/pending-join@1".into(),
        request: PortableSpawnAdmissionRequest {
            schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: invitation.claim.invitation_id.clone(),
            body_id: invitation.claim.body_id.clone(),
            host_advertisement: advertisement.clone(),
            nonce: invitation.claim.nonce,
            signature: signature.to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        },
        invitation,
    };
    durable_host::write_json_atomic(&root.join("body/pending-join.json"), &pending).unwrap();
    let receipt = PortableAdmissionReceipt {
        schema: conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential: MembershipCredential {
            credential_id: id("fixture/credential"),
            body_id: id("fixture/body"),
            part_id: id("fixture/part"),
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            issued_at_millis: 10,
        },
        host_advertisement: advertisement,
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    };
    Fixture {
        root,
        pending,
        receipt,
    }
}
fn complete(f: &Fixture) -> Result<(), String> {
    durable_host::membership::complete_body_join_document(f.receipt.clone(), &f.root, true)
}
#[test]
fn exact_key_scope_survives_pending_consumption_without_claiming_return_authority() {
    let f = fixture();
    complete(&f).unwrap();
    let record = read(&f.root.join("body").join(PATH)).unwrap();
    assert_eq!(record.body_id, f.receipt.credential.body_id.as_str());
    assert_eq!(record.part_id, f.receipt.credential.part_id.as_str());
    assert_eq!(record.host_id, f.receipt.credential.host_id.as_str());
    assert_eq!(
        record.invitation_id,
        f.pending.request.invitation_id.as_str()
    );
    assert!(record.secret == f.pending.invitation.secret);
    assert!(!f.root.join("body/pending-join.json").exists());
    // The same exact record can be retried independently of a completed local
    // membership transaction; no identity replacement is performed.
    retain(
        &f.root,
        &f.pending,
        &f.receipt.credential,
        &f.receipt.host_advertisement,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(f.root.join("body").join(PATH))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(f.root.join("body"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
#[test]
fn persistence_failure_leaves_membership_and_pending_unconsumed() {
    let f = fixture();
    fs::create_dir(f.root.join("body").join(PATH)).unwrap();
    assert!(complete(&f).is_err());
    assert!(f.root.join("body/pending-join.json").exists());
    assert!(
        durable_host::read_installation(&f.root.join("installation.json"))
            .unwrap()
            .joined_body_state
            .is_none()
    );
    assert!(!f.root.join("body/membership-credential.json").exists());
}
#[test]
fn different_receipt_scope_and_signed_key_cannot_overwrite_retained_identity() {
    let mut f = fixture();
    let mut wrong = f.receipt.clone();
    wrong.credential.body_id = id("fixture/other");
    assert!(durable_host::membership::complete_body_join_document(wrong, &f.root, true).is_err());
    retain(
        &f.root,
        &f.pending,
        &f.receipt.credential,
        &f.receipt.host_advertisement,
    )
    .unwrap();
    let original = fs::read(f.root.join("body").join(PATH)).unwrap();
    f.receipt.credential.part_id = id("fixture/another-part");
    assert!(complete(&f).is_err());
    f.receipt.credential.part_id = id("fixture/part");
    f.pending.invitation.secret = [8; 32];
    assert!(retain(
        &f.root,
        &f.pending,
        &f.receipt.credential,
        &f.receipt.host_advertisement
    )
    .is_err());
    assert!(fs::read(f.root.join("body").join(PATH)).unwrap() == original);
    assert!(f.root.join("body/pending-join.json").exists());
}
#[cfg(unix)]
#[test]
fn symlink_record_never_reads_or_overwrites_its_target() {
    use std::os::unix::fs::symlink;
    let f = fixture();
    let target = f.root.join("unrelated");
    fs::write(&target, b"unchanged").unwrap();
    symlink(&target, f.root.join("body").join(PATH)).unwrap();
    assert!(complete(&f).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"unchanged");
    assert!(f.root.join("body/pending-join.json").exists());
}
#[test]
fn rendezvous_without_original_pending_never_invents_private_identity() {
    let f = fixture();
    fs::remove_file(f.root.join("body/pending-join.json")).unwrap();
    durable_host::membership::retain_rendezvous_membership(
        &f.root,
        &f.receipt.credential,
        f.receipt.credential.body_id.as_str(),
        &f.receipt.host_advertisement,
    )
    .unwrap();
    assert!(!f.root.join("body").join(PATH).exists());
}

#[cfg(unix)]
#[test]
fn restart_retries_around_preserved_uncommitted_private_staging() {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let f = fixture();
    let body = f.root.join("body");
    // Simulate process death before publication, including the earlier fixed
    // staging spelling. Neither incomplete nor mismatched state is final custody.
    for name in [
        "member-continuity-key.tmp",
        "member-continuity-key-interrupted.tmp",
    ] {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(body.join(name))
            .unwrap();
        file.write_all(b"interrupted private staging fixture")
            .unwrap();
        file.sync_all().unwrap();
    }
    complete(&f).unwrap();
    assert!(read(&body.join(PATH)).is_ok());
    assert!(!body.join("pending-join.json").exists());
    let stages = fs::read_dir(&body)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "tmp")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        stages.len(),
        2,
        "only the interrupted fixture stages remain"
    );
    for stage in stages {
        assert_eq!(
            fs::read(stage.path()).unwrap(),
            b"interrupted private staging fixture"
        );
        assert_eq!(
            stage.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[cfg(unix)]
#[test]
fn restart_after_key_publication_finishes_membership_without_clobbering_stage() {
    let f = fixture();
    retain(
        &f.root,
        &f.pending,
        &f.receipt.credential,
        &f.receipt.host_advertisement,
    )
    .unwrap();
    let path = f.root.join("body").join(PATH);
    let stage = f
        .root
        .join("body/member-continuity-key-published-interruption.tmp");
    fs::hard_link(&path, &stage).unwrap();
    // A previous process published and synced its key but did not commit local
    // membership. The idempotent retry validates the exact retained key first.
    complete(&f).unwrap();
    assert!(stage.is_file());
    assert!(read(&path).unwrap() == read(&stage).unwrap());
    assert!(!f.root.join("body/pending-join.json").exists());
}

fn returning_fixture(
    fresh_boot: bool,
) -> (
    Fixture,
    conduit_body::AdmissionManager,
    conduit_body::BodyMembership,
    HostAdvertisement,
    conduit_body::PartReturnChallenge,
) {
    use conduit_body::{AdmissionManager, AdmissionSigns, BodyMembership, SpawnAdmissionProof};
    let mut f = fixture();
    let old = &f.receipt.host_advertisement;
    let mut manager = AdmissionManager::new(f.receipt.credential.body_id.clone()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap(),
            [3; 32],
            1,
            1000,
        )
        .unwrap();
    let signature = invitation.secret.sign(&invitation.signing_transcript(
        &old.host_id,
        &old.boot_id,
        old.offer_generation,
    ));
    let mut membership = BodyMembership::new(invitation.body_id.clone()).unwrap();
    f.receipt.credential = manager
        .complete_spawn(
            &mut membership,
            old,
            &SpawnAdmissionProof {
                invitation_id: invitation.invitation_id.clone(),
                body_id: invitation.body_id.clone(),
                host_id: old.host_id.clone(),
                boot_id: old.boot_id.clone(),
                nonce: invitation.nonce,
                signature,
            },
            2,
            AdmissionSigns {
                part_admitted: "fixture/part".into(),
                host_attached: "fixture/attached".into(),
                candidate_admitted: "fixture/candidate".into(),
            },
        )
        .unwrap();
    f.pending.invitation.claim = invitation.claim();
    f.pending.request.invitation_id = invitation.invitation_id;
    f.pending.request.signature = signature.to_vec();
    retain(&f.root, &f.pending, &f.receipt.credential, old).unwrap();
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &f.receipt.credential.part_id,
            &old.boot_id,
            "fixture/lost".into(),
        )
        .unwrap();
    let mut current = old.clone();
    if fresh_boot {
        current.boot_id = "fixture/returned-boot".into();
    }
    current.offer_generation = OfferGeneration(2);
    let challenge = manager
        .begin_return(
            &membership,
            &f.receipt.credential.part_id,
            &current,
            [9; 32],
            10,
            100,
        )
        .unwrap();
    (f, manager, membership, current, challenge)
}

#[test]
fn retained_private_signer_completes_real_core_return_without_invitation_reuse() {
    for fresh_boot in [false, true] {
        let (f, mut manager, mut membership, current, challenge) = returning_fixture(fresh_boot);
        let proof = sign_return_challenge(&f.root, &f.receipt.credential, &current, &challenge, 11)
            .unwrap();
        let returned = manager
            .complete_return(
                &mut membership,
                &current,
                &proof,
                12,
                "fixture/return".into(),
            )
            .unwrap();
        assert_eq!(returned.part_id, f.receipt.credential.part_id);
        assert_eq!(returned.host_id, f.receipt.credential.host_id);
        assert_eq!(returned.boot_id, current.boot_id);
        assert!(manager
            .complete_return(
                &mut membership,
                &current,
                &proof,
                13,
                "fixture/replay".into()
            )
            .is_err());
    }
}

#[test]
fn return_signer_refuses_mismatched_scope_and_invalid_time_without_changing_key() {
    let (f, _, _, current, challenge) = returning_fixture(true);
    let original = fs::read(f.root.join("body").join(PATH)).unwrap();
    for index in 0..10 {
        let mut wrong = challenge.clone();
        match index {
            0 => wrong.body_id = id("another/body"),
            1 => wrong.part_id = id("another/part"),
            2 => wrong.host_id = "another/host".into(),
            3 => wrong.boot_id = "another/boot".into(),
            4 => wrong.offer_generation = OfferGeneration(99),
            5 => wrong.nonce = [0; 32],
            6 => wrong.issued_at_millis = 12,
            7 => wrong.expires_at_millis = 11,
            8 => wrong.expires_at_millis = 10 + conduit_body::MAX_ADMISSION_TTL_MILLIS + 1,
            9 => wrong.admission_id = id(""),
            _ => unreachable!(),
        }
        assert!(
            sign_return_challenge(&f.root, &f.receipt.credential, &current, &wrong, 11).is_err()
        );
    }
    let mut credential = f.receipt.credential.clone();
    credential.part_id = id("another/part");
    assert!(sign_return_challenge(&f.root, &credential, &current, &challenge, 11).is_err());
    assert_eq!(fs::read(f.root.join("body").join(PATH)).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn return_signer_refuses_symlink_and_nonprivate_key_file() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let (f, _, _, current, challenge) = returning_fixture(true);
    let path = f.root.join("body").join(PATH);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        sign_return_challenge(&f.root, &f.receipt.credential, &current, &challenge, 11).is_err()
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let target = f.root.join("preserved-key");
    fs::rename(&path, &target).unwrap();
    let original = fs::read(&target).unwrap();
    symlink(&target, &path).unwrap();
    assert!(
        sign_return_challenge(&f.root, &f.receipt.credential, &current, &challenge, 11).is_err()
    );
    assert_eq!(fs::read(&target).unwrap(), original);
}
