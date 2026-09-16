//! Installed, pipeable Body invitation issue and acceptance operations.

use super::{
    bounded_read, current_time_millis, digest, read_installation, restrict_directory,
    write_json_atomic, RuntimeStatus, RUNTIME_SCHEMA,
};
use conduit_body::{AdmissionManager, SpawnAdmissionProof, SpawnInvitationSecret};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostConfig};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path};

pub(super) const INVITATION_SCHEMA: &str = "conduit.body/spawn-invitation@1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PortableInvitation {
    pub(super) schema: String,
    pub(super) claim: conduit_body::SpawnInvitationClaim,
    pub(super) secret: [u8; 32],
}

#[derive(Serialize)]
struct PortableSpawnAdmissionRequest {
    schema: &'static str,
    invitation_id: conduit_body::SpawnInvitationId,
    body_id: conduit_body::BodyId,
    host_advertisement: conduit_core::HostAdvertisement,
    nonce: [u8; 32],
    signature: Vec<u8>,
    membership_admitted: bool,
    plan_created: bool,
    play_created: bool,
}

#[derive(Serialize)]
struct PendingBodyJoin {
    schema: &'static str,
    invitation: PortableInvitation,
    request: PortableSpawnAdmissionRequest,
}

pub(crate) fn issue_body_invitation(state_dir: &Path, ttl_seconds: u64) -> Result<(), String> {
    if !(1..=600).contains(&ttl_seconds) {
        return Err("invitation lifetime must be between 1 and 600 seconds".into());
    }
    let installation = read_installation(&state_dir.join("installation.json"))?;
    let body = installation
        .body_state
        .as_ref()
        .ok_or("this installed Host does not own a Body")?;
    let biography_bytes = bounded_read(Path::new(&body.biography_path), 2 * 1024 * 1024)?;
    if digest(&biography_bytes) != body.biography_sha256 {
        return Err("retained Body biography no longer matches its exact identity".into());
    }
    let biography: conduit_body::BodyBiographyEvidence =
        serde_json::from_slice(&biography_bytes)
            .map_err(|error| format!("retained Body biography: {error}"))?;
    biography
        .validate()
        .map_err(|error| format!("retained Body biography refused: {error:?}"))?;
    if biography.body_id.as_str() != body.body_id {
        return Err("retained Body biography belongs to another Body".into());
    }
    let body_id = biography.body_id;
    let admission_path = state_dir.join("body").join("admission.json");
    let mut manager = if admission_path.exists() {
        let bytes = bounded_read(&admission_path, 256 * 1024)?;
        let manager: AdmissionManager = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Body admission state: {error}"))?;
        if manager.body_id != body_id {
            return Err("Body admission state belongs to another Body".into());
        }
        manager
    } else {
        AdmissionManager::new(body_id)
            .map_err(|error| format!("initialize Body admission: {error:?}"))?
    };
    let now_millis = current_time_millis()?;
    let expires_at_millis = now_millis
        .checked_add(ttl_seconds.saturating_mul(1_000))
        .ok_or("invitation expiry overflow")?;
    let mut secret_bytes = [0_u8; 32];
    let mut nonce = [0_u8; 32];
    getrandom::fill(&mut secret_bytes)
        .map_err(|error| format!("create invitation secret: {error}"))?;
    getrandom::fill(&mut nonce).map_err(|error| format!("create invitation nonce: {error}"))?;
    let secret = SpawnInvitationSecret::from_csprng_bytes(secret_bytes)
        .map_err(|error| format!("create invitation secret: {error:?}"))?;
    let invitation = manager
        .issue_spawn_invitation(secret, nonce, now_millis, expires_at_millis)
        .map_err(|error| format!("issue Body invitation: {error:?}"))?;
    write_json_atomic(&admission_path, &manager)?;
    let portable = PortableInvitation {
        schema: INVITATION_SCHEMA.into(),
        claim: invitation.claim(),
        secret: invitation.secret.copy_for_target_provisioning(),
    };
    let encoded = serde_json::to_string(&portable)
        .map_err(|error| format!("encode Body invitation: {error}"))?;
    println!("{encoded}");
    secret_bytes.fill(0);
    Ok(())
}

pub(crate) fn accept_body_invitation(
    invitation_path: &Path,
    state_dir: &Path,
    authorize_join: bool,
) -> Result<(), String> {
    if !authorize_join {
        return Err("accepting a Body invitation requires --authorize-join".into());
    }
    let installation = read_installation(&state_dir.join("installation.json"))?;
    if installation.body_state.is_some() {
        return Err("this installed Host already owns a Body".into());
    }
    let bytes = if invitation_path == Path::new("-") {
        bounded_stdin(64 * 1024)?
    } else {
        bounded_read(invitation_path, 64 * 1024)?
    };
    let invitation: PortableInvitation =
        serde_json::from_slice(&bytes).map_err(|error| format!("Body invitation: {error}"))?;
    if invitation.schema != INVITATION_SCHEMA {
        return Err("Body invitation has an unsupported schema".into());
    }
    let now_millis = current_time_millis()?;
    invitation
        .claim
        .inspect(now_millis)
        .map_err(|error| format!("Body invitation refused: {error:?}"))?;
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret)
        .map_err(|error| format!("Body invitation secret refused: {error:?}"))?;
    let runtime_bytes = bounded_read(&state_dir.join("runtime.json"), 64 * 1024)?;
    let runtime: RuntimeStatus = serde_json::from_slice(&runtime_bytes)
        .map_err(|error| format!("durable Host runtime status: {error}"))?;
    if runtime.schema != RUNTIME_SCHEMA || runtime.host_id != installation.host_id {
        return Err("durable Host runtime status is stale or belongs to another Host".into());
    }
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(runtime.host_id.as_str()),
        boot_id: BootId::from(runtime.boot_id.as_str()),
        offer_generation: OfferGeneration(runtime.offer_generation),
    });
    let advertisement = host.advertisement().clone();
    let transcript = invitation.claim.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    );
    let proof = SpawnAdmissionProof {
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        nonce: invitation.claim.nonce,
        signature: secret.sign(&transcript),
    };
    let request = PortableSpawnAdmissionRequest {
        schema: "conduit.body/spawn-admission-request@1",
        invitation_id: proof.invitation_id,
        body_id: proof.body_id,
        host_advertisement: advertisement,
        nonce: proof.nonce,
        signature: proof.signature.to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let body_dir = state_dir.join("body");
    fs::create_dir_all(&body_dir)
        .map_err(|error| format!("create Body state directory: {error}"))?;
    restrict_directory(&body_dir)?;
    let encoded = serde_json::to_string(&request)
        .map_err(|error| format!("encode Body admission request: {error}"))?;
    write_json_atomic(
        &body_dir.join("pending-join.json"),
        &PendingBodyJoin {
            schema: "conduit.body/pending-join@1",
            invitation,
            request,
        },
    )?;
    println!("{encoded}");
    Ok(())
}

fn bounded_stdin(maximum: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read Body invitation from standard input: {error}"))?;
    if bytes.is_empty() || bytes.len() as u64 > maximum {
        return Err("standard input violates the finite invitation bound".into());
    }
    Ok(bytes)
}
