//! Explicit stopped-Host recovery from original private construction evidence.
//! Restores signing custody only; an authenticated owner must still accept return.
use super::*;
use clap::Args as ClapArgs;
use conduit_body::{PortableAdmissionReceipt, PortableInvitation, PortableSpawnAdmissionRequest};
use conduit_host_make::DeploymentRealizationReceipt;
use std::{io::Read, path::PathBuf};

#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    #[arg(long)]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub original_seed: PathBuf,
    /// Independently retained byte-verification receipt for the original Seed.
    #[arg(long)]
    pub realization_receipt: PathBuf,
    #[arg(long)]
    pub admission_request: PathBuf,
    #[arg(long)]
    pub admission_receipt: PathBuf,
    #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
    pub authorize_recovery: bool,
}

pub(crate) fn run(args: Args) -> Result<(), String> {
    if !args.authorize_recovery {
        return Err("member key recovery requires --authorize-recovery".into());
    }
    let _host = super::super::owner_lock::acquire(&args.state_dir)?;
    let _body = super::super::owner_lock::body(&args.state_dir)?;
    // Refuse an unjoined/owning Host before opening a large private package.
    let installation = read_installation(&args.state_dir.join("installation.json"))?;
    let _current = load_joined_credential(&args.state_dir, &installation)?;
    let realization: DeploymentRealizationReceipt = serde_json::from_slice(&bounded_read(
        &args.realization_receipt,
        MAXIMUM_BODY_ADMISSION_BYTES,
    )?)
    .map_err(|_| "invalid original Seed realization receipt")?;
    let request: PortableSpawnAdmissionRequest = serde_json::from_slice(&bounded_read(
        &args.admission_request,
        MAXIMUM_BODY_ADMISSION_BYTES,
    )?)
    .map_err(|_| "invalid original admission request")?;
    let receipt: PortableAdmissionReceipt = serde_json::from_slice(&bounded_read(
        &args.admission_receipt,
        MAXIMUM_BODY_ADMISSION_BYTES,
    )?)
    .map_err(|_| "invalid original admission receipt")?;
    let mut bytes = capture_seed(&args.original_seed)?;
    // Expiry still forbids admission. Here historical invitation structure and
    // exact private bytes are checked solely to restore original key custody,
    // just as continuity_key::retain checks the consumed invitation at time 0.
    let verified = conduit_body_make::verify_joining_host_package(
        &bytes,
        &realization,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        0,
    );
    bytes.fill(0);
    let verified = verified?;
    restore_from_proof(&args.state_dir, verified.invitation(), request, &receipt)?;
    println!(
        "{}",
        serde_json::json!({
            "schema":"conduit.body/member-key-restoration@1",
            "body_id":receipt.credential.body_id,
            "part_id":receipt.credential.part_id,
            "host_id":receipt.credential.host_id,
            "private_key_retained":true,
            "membership_changed":false,
            "return_completed":false,
        })
    );
    Ok(())
}

pub(super) fn restore_from_proof(
    state: &Path,
    invitation: &PortableInvitation,
    request: PortableSpawnAdmissionRequest,
    receipt: &PortableAdmissionReceipt,
) -> Result<(), String> {
    let installation = read_installation(&state.join("installation.json"))?;
    let current = load_joined_credential(state, &installation)?;
    if receipt.credential.body_id != current.body_id
        || receipt.credential.part_id != current.part_id
        || receipt.credential.host_id != current.host_id
    {
        return Err("original admission differs from the installed Body/Part/Host".into());
    }
    receipt
        .validate_against(&request)
        .map_err(|_| "original admission evidence differs")?;
    let mut pending = PendingBodyJoin {
        schema: "conduit.body/pending-join@1".into(),
        invitation: invitation.clone(),
        request,
    };
    let result = continuity_key::retain(
        state,
        &pending,
        &receipt.credential,
        &receipt.host_advertisement,
    );
    pending.invitation.secret.fill(0);
    result
}

// One admitted package extent and overflow sentinel; no growing read or retry.
fn capture_seed(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = fs::File::open(path).map_err(|_| "open original private Seed failed")?;
    let metadata = file
        .metadata()
        .map_err(|_| "inspect original Seed failed")?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 64 * 1024 * 1024 {
        return Err("original Seed exceeds its finite regular-file bound".into());
    }
    let extent = metadata.len() as usize;
    let mut bytes = vec![0; extent + 1];
    let mut used = 0;
    while used < bytes.len() {
        match file.read(&mut bytes[used..]) {
            Ok(0) => break,
            Ok(count) => used += count,
            Err(_) => {
                bytes.fill(0);
                return Err("read original Seed failed".into());
            }
        }
    }
    if used != extent {
        bytes.fill(0);
        return Err("original Seed extent changed during capture".into());
    }
    bytes.truncate(used);
    Ok(bytes)
}
