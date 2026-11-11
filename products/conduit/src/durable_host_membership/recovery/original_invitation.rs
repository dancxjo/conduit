//! Stopped-Host recovery from reviewed ORIGINAL private invitation bytes.
//! Reconstructs a historical request; never claims that request was retained or
//! authenticates the owner receipt. Fresh owner challenge remains mandatory.
use super::*;
use clap::Args as ClapArgs;
use conduit_body::{PortableInvitation, SpawnInvitationSecret};
use std::{fs, io::Read, path::Path};
const MAXIMUM_PRIVATE_INVITATION_BYTES: usize = 16 * 1024;

#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    #[arg(long)]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub original_invitation: PathBuf,
    /// Independently reviewed original private byte digest; not supplied by the file.
    #[arg(long)]
    pub original_invitation_sha256: String,
    #[arg(long)]
    pub admission_receipt: PathBuf,
    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
    pub authorize_recovery: bool,
}
pub(crate) fn run(args: Args) -> Result<(), String> {
    if !args.authorize_recovery {
        return Err("member key recovery requires --authorize-recovery".into());
    }
    let _host = super::super::super::owner_lock::acquire(&args.state_dir)?;
    let _body = super::super::super::owner_lock::body(&args.state_dir)?;
    let installation = read_installation(&args.state_dir.join("installation.json"))?;
    let _current = load_joined_credential(&args.state_dir, &installation)?;
    let receipt: PortableAdmissionReceipt = serde_json::from_slice(&bounded_read(
        &args.admission_receipt,
        MAXIMUM_BODY_ADMISSION_BYTES,
    )?)
    .map_err(|_| "invalid original admission receipt")?;
    let mut bytes = capture(&args.original_invitation, &args.original_invitation_sha256)?;
    let decoded = serde_json::from_slice::<PortableInvitation>(&bytes);
    bytes.fill(0);
    let mut invitation = decoded.map_err(|_| "invalid original private invitation")?;
    let result = reconstruct(&invitation, &receipt).and_then(|request| {
        super::restore_from_proof(&args.state_dir, &invitation, request, &receipt)
    });
    invitation.secret.fill(0);
    result?;
    println!(
        "{}",
        serde_json::json!({
            "schema":"conduit.body/member-key-restoration@1",
            "body_id":receipt.credential.body_id,"part_id":receipt.credential.part_id,
            "host_id":receipt.credential.host_id,"private_key_retained":true,
            "historical_request_reconstructed":true,"membership_changed":false,
            "return_completed":false,"authenticated_owner_challenge_required":true,
        })
    );
    Ok(())
}
fn reconstruct(
    invitation: &PortableInvitation,
    receipt: &PortableAdmissionReceipt,
) -> Result<PortableSpawnAdmissionRequest, String> {
    invitation
        .validate(0)
        .map_err(|_| "original invitation historical structure refused")?;
    let ad = &receipt.host_advertisement;
    let identities = [
        invitation.claim.invitation_id.as_str(),
        invitation.claim.body_id.as_str(),
        receipt.credential.credential_id.as_str(),
        receipt.credential.body_id.as_str(),
        receipt.credential.part_id.as_str(),
        receipt.credential.host_id.as_str(),
        receipt.credential.boot_id.as_str(),
        ad.host_id.as_str(),
        ad.boot_id.as_str(),
    ];
    if identities.iter().any(|id| id.is_empty() || id.len() > 256) || ad.offer_generation.0 == 0 {
        return Err("original invitation/admission identity is absent or unbounded".into());
    }
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret)
        .map_err(|_| "original private invitation key refused")?;
    let signature = secret.sign(&invitation.claim.signing_transcript(
        &ad.host_id,
        &ad.boot_id,
        ad.offer_generation,
    ));
    let request = PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: ad.clone(),
        nonce: invitation.claim.nonce,
        signature: signature.to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    request
        .validate()
        .map_err(|_| "reconstructed historical request refused")?;
    receipt
        .validate_against(&request)
        .map_err(|_| "original receipt differs from reconstructed historical request")?;
    Ok(request)
}
fn capture(path: &Path, expected: &str) -> Result<Vec<u8>, String> {
    let expected = expected.strip_prefix("sha256:").unwrap_or(expected);
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err("original invitation requires an exact independently reviewed SHA256".into());
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "inspect original private invitation failed")?;
    if !metadata.file_type().is_file()
        || metadata.len() == 0
        || metadata.len() > MAXIMUM_PRIVATE_INVITATION_BYTES as u64
    {
        return Err("original private invitation is not a bounded regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("original invitation is not private to its owner".into());
        }
    }
    let mut file = fs::File::open(path).map_err(|_| "open original private invitation failed")?;
    let opened = file
        .metadata()
        .map_err(|_| "inspect captured private invitation failed")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
            return Err("original private invitation changed before capture".into());
        }
    }
    if opened.len() != metadata.len() {
        return Err("original private invitation changed before capture".into());
    }
    let extent = metadata.len() as usize;
    let mut bytes = vec![0; extent + 1];
    let mut used = 0;
    while used < bytes.len() {
        match file.read(&mut bytes[used..]) {
            Ok(0) => break,
            Ok(n) => used += n,
            Err(_) => {
                bytes.fill(0);
                return Err("read original private invitation failed".into());
            }
        }
    }
    if used != extent || digest(&bytes[..used]) != format!("sha256:{expected}") {
        bytes.fill(0);
        return Err("original private invitation differs from independently reviewed bytes".into());
    }
    bytes.truncate(used);
    Ok(bytes)
}
