//! Pinned original private bytes restore signing custody only, never membership.
use super::*;
use crate::durable_host::membership::recovery::original_invitation::{run, Args};

fn prepare(f: &Fixture) -> Args {
    let invitation = f.root.join("original-invitation.json");
    let receipt = f.root.join("original-receipt.json");
    fs::write(
        &invitation,
        serde_json::to_vec(&f.pending.invitation).unwrap(),
    )
    .unwrap();
    fs::write(&receipt, serde_json::to_vec(&f.receipt).unwrap()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&invitation, fs::Permissions::from_mode(0o600)).unwrap();
    }
    Args {
        state_dir: f.root.clone(),
        original_invitation_sha256: durable_host::digest(&fs::read(&invitation).unwrap()),
        original_invitation: invitation,
        admission_receipt: receipt,
        authorize_recovery: true,
    }
}
#[test]
fn pinned_original_invitation_restores_key_without_changing_credential_or_biography() {
    let f = fixture();
    complete(&f).unwrap();
    let args = prepare(&f);
    let installation = fs::read(f.root.join("installation.json")).unwrap();
    let credential = fs::read(f.root.join("body/membership-credential.json")).unwrap();
    let path = f.root.join("body").join(PATH);
    fs::remove_file(&path).unwrap();
    run(args).unwrap();
    assert!(read(&path).unwrap().secret == f.pending.invitation.secret);
    assert_eq!(
        fs::read(f.root.join("installation.json")).unwrap(),
        installation
    );
    assert_eq!(
        fs::read(f.root.join("body/membership-credential.json")).unwrap(),
        credential
    );
    let before = fs::read(&path).unwrap();
    run(prepare(&f)).unwrap();
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn changed_secret_other_body_receipt_and_missing_identity_never_write_a_key() {
    for variant in 0..6 {
        let f = fixture();
        complete(&f).unwrap();
        let mut args = prepare(&f);
        let key = f.root.join("body").join(PATH);
        fs::remove_file(&key).unwrap();
        let before = fs::read(f.root.join("installation.json")).unwrap();
        let mut invitation = f.pending.invitation.clone();
        let mut receipt = f.receipt.clone();
        match variant {
            0 => invitation.secret[0] ^= 1, // original independent digest MUST reject this.
            1 => invitation.claim.body_id = id("different/body"),
            2 => receipt.credential.part_id = id("different/part"),
            3 => {
                receipt.credential.boot_id = "".into();
                receipt.host_advertisement.boot_id = "".into();
            }
            4 => receipt.membership_admitted = false,
            _ => receipt.host_advertisement.offer_generation = OfferGeneration(0),
        }
        fs::write(
            &args.original_invitation,
            serde_json::to_vec(&invitation).unwrap(),
        )
        .unwrap();
        fs::write(
            &args.admission_receipt,
            serde_json::to_vec(&receipt).unwrap(),
        )
        .unwrap();
        // Other-scope inputs are independently selected, and still must refuse.
        if variant != 0 {
            args.original_invitation_sha256 =
                durable_host::digest(&fs::read(&args.original_invitation).unwrap());
        }
        assert!(run(args).is_err(), "variant {variant}");
        assert!(!key.exists());
        assert_eq!(fs::read(f.root.join("installation.json")).unwrap(), before);
    }
}
#[test]
fn recovery_cannot_overwrite_an_existing_other_private_key() {
    let f = fixture();
    complete(&f).unwrap();
    let mut args = prepare(&f);
    let path = f.root.join("body").join(PATH);
    let before = fs::read(&path).unwrap();
    let mut invitation = f.pending.invitation.clone();
    invitation.secret = [8; 32];
    fs::write(
        &args.original_invitation,
        serde_json::to_vec(&invitation).unwrap(),
    )
    .unwrap();
    args.original_invitation_sha256 =
        durable_host::digest(&fs::read(&args.original_invitation).unwrap());
    assert!(run(args).is_err());
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn authorization_is_required_before_opening_any_private_input() {
    let absent = PathBuf::from("/nonexistent/original-private-invitation");
    assert_eq!(
        run(Args {
            state_dir: absent.clone(),
            original_invitation: absent.clone(),
            original_invitation_sha256: "0".repeat(64),
            admission_receipt: absent,
            authorize_recovery: false
        })
        .unwrap_err(),
        "member key recovery requires --authorize-recovery"
    );
}
#[test]
fn seed_and_direct_invitation_cli_routes_are_mutually_exclusive() {
    use clap::Parser;
    let args = [
        "conduit",
        "body",
        "restore-member-key-from-invitation",
        "--state-dir",
        "state",
        "--original-invitation",
        "original.json",
        "--original-invitation-sha256",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "--admission-receipt",
        "receipt.json",
        "--authorize-recovery",
    ];
    assert!(crate::cli::Cli::try_parse_from(args).is_ok());
    let mut mixed = args.to_vec();
    mixed.extend(["--original-seed", "not-an-original-seed"]);
    assert!(crate::cli::Cli::try_parse_from(mixed).is_err());
    assert!(crate::cli::Cli::try_parse_from([
        "conduit",
        "body",
        "restore-member-key",
        "--state-dir",
        "state",
        "--original-seed",
        "seed",
        "--realization-receipt",
        "realized.json",
        "--admission-request",
        "request.json",
        "--admission-receipt",
        "receipt.json",
        "--authorize-recovery",
        "--original-invitation",
        "original.json"
    ])
    .is_err());
}

#[cfg(unix)]
#[test]
fn private_capture_rejects_public_bytes_symlinks_and_oversized_extent() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    for variant in 0..3 {
        let f = fixture();
        complete(&f).unwrap();
        let mut args = prepare(&f);
        let key = f.root.join("body").join(PATH);
        fs::remove_file(&key).unwrap();
        match variant {
            0 => fs::set_permissions(&args.original_invitation, fs::Permissions::from_mode(0o644))
                .unwrap(),
            1 => {
                let link = f.root.join("invitation.link");
                symlink(&args.original_invitation, &link).unwrap();
                args.original_invitation = link;
            }
            _ => {
                let bytes = vec![0; 16 * 1024 + 1];
                fs::write(&args.original_invitation, &bytes).unwrap();
                args.original_invitation_sha256 = durable_host::digest(&bytes);
            }
        }
        assert!(run(args).is_err(), "capture variant {variant}");
        assert!(!key.exists());
    }
}
