//! Installed, pipeable Body invitation issue and acceptance operations.

use super::{
    bounded_read, current_time_millis, digest, read_installation, restrict_directory,
    write_json_atomic, Installation, RuntimeStatus, RUNTIME_SCHEMA,
};
use conduit_body::{
    AdmissionManager, AdmissionSigns, BodyBiographyEvidence, MembershipCredential,
    SpawnAdmissionProof, SpawnInvitationSecret,
};
use conduit_core::{bind_sign, BootId, HostAdvertisement, HostId, OfferGeneration};
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PortableSpawnAdmissionRequest {
    pub(super) schema: String,
    pub(super) invitation_id: conduit_body::SpawnInvitationId,
    pub(super) body_id: conduit_body::BodyId,
    pub(super) host_advertisement: conduit_core::HostAdvertisement,
    pub(super) nonce: [u8; 32],
    pub(super) signature: Vec<u8>,
    pub(super) membership_admitted: bool,
    pub(super) plan_created: bool,
    pub(super) play_created: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct PortableAdmissionReceipt {
    schema: String,
    credential: MembershipCredential,
    host_advertisement: HostAdvertisement,
    membership_admitted: bool,
    current_offers_available: bool,
    plan_created: bool,
    play_created: bool,
}

#[derive(Serialize, Deserialize)]
struct AdmissionTransaction {
    schema: String,
    admission: AdmissionManager,
    biography: BodyBiographyEvidence,
    installation: Installation,
    receipt: PortableAdmissionReceipt,
}

#[derive(Serialize, Deserialize)]
pub(super) struct PendingBodyJoin {
    pub(super) schema: String,
    pub(super) invitation: PortableInvitation,
    pub(super) request: PortableSpawnAdmissionRequest,
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
    recover_admission_transaction(state_dir, Path::new(&body.biography_path))?;
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

pub(crate) fn admit_body_request(
    request_path: &Path,
    state_dir: &Path,
    authorize_admission: bool,
) -> Result<(), String> {
    if !authorize_admission {
        return Err("admitting a Host into this Body requires --authorize-admission".into());
    }
    let mut installation = read_installation(&state_dir.join("installation.json"))?;
    let body = installation
        .body_state
        .as_ref()
        .ok_or("this installed Host does not own a Body")?;
    let biography_path = std::path::PathBuf::from(&body.biography_path);
    recover_admission_transaction(state_dir, &biography_path)?;
    let request_bytes = if request_path == Path::new("-") {
        bounded_stdin(256 * 1024)?
    } else {
        bounded_read(request_path, 256 * 1024)?
    };
    let request: PortableSpawnAdmissionRequest = serde_json::from_slice(&request_bytes)
        .map_err(|error| format!("Body admission request: {error}"))?;
    if request.schema != "conduit.body/spawn-admission-request@1"
        || request.membership_admitted
        || request.plan_created
        || request.play_created
    {
        return Err("Body admission request has an unsupported schema or claims effects".into());
    }
    let biography_bytes = bounded_read(&biography_path, 2 * 1024 * 1024)?;
    if digest(&biography_bytes) != body.biography_sha256 {
        return Err("retained Body biography no longer matches its exact identity".into());
    }
    let mut biography: BodyBiographyEvidence = serde_json::from_slice(&biography_bytes)
        .map_err(|error| format!("retained Body biography: {error}"))?;
    biography
        .validate()
        .map_err(|error| format!("retained Body biography refused: {error:?}"))?;
    if biography.body_id != request.body_id || biography.body_id.as_str() != body.body_id {
        return Err("admission request belongs to another Body".into());
    }
    let admission_path = state_dir.join("body/admission.json");
    let mut admission: AdmissionManager =
        serde_json::from_slice(&bounded_read(&admission_path, 256 * 1024)?)
            .map_err(|error| format!("Body admission state: {error}"))?;
    let signature: [u8; conduit_body::ADMISSION_SIGNATURE_BYTES] = request
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| "Body admission request signature has the wrong bound")?;
    let proof = SpawnAdmissionProof {
        invitation_id: request.invitation_id,
        body_id: request.body_id,
        host_id: request.host_advertisement.host_id.clone(),
        boot_id: request.host_advertisement.boot_id.clone(),
        nonce: request.nonce,
        signature,
    };
    let first_sequence = biography
        .last_sequence()
        .checked_add(1)
        .ok_or("Body biography sequence exhausted")?;
    let second_sequence = first_sequence
        .checked_add(1)
        .ok_or("Body biography sequence exhausted")?;
    let authority_host = HostId::from(installation.host_id.as_str());
    let runtime: RuntimeStatus =
        serde_json::from_slice(&bounded_read(&state_dir.join("runtime.json"), 64 * 1024)?)
            .map_err(|error| format!("durable Host runtime status: {error}"))?;
    if runtime.schema != RUNTIME_SCHEMA || runtime.host_id != installation.host_id {
        return Err("durable Host runtime status is stale or belongs to another Host".into());
    }
    let authority_boot = BootId::from(runtime.boot_id.as_str());
    let prior_events = biography.membership.events.len();
    let credential = match admission.complete_spawn(
        &mut biography.membership,
        &request.host_advertisement,
        &proof,
        current_time_millis()?,
        AdmissionSigns {
            part_admitted: bind_sign(&authority_host, &authority_boot, None, first_sequence)
                .sign_id,
            host_attached: bind_sign(&authority_host, &authority_boot, None, second_sequence)
                .sign_id,
            candidate_admitted: bind_sign(&authority_host, &authority_boot, None, second_sequence)
                .sign_id,
        },
    ) {
        Ok(credential) => credential,
        Err(error) => {
            write_json_atomic(&admission_path, &admission)?;
            return Err(format!("Body admission refused: {error:?}"));
        }
    };
    let events = biography.membership.events[prior_events..]
        .iter()
        .zip([first_sequence, second_sequence])
        .map(|(event, sequence)| (event.change_id.clone(), sequence))
        .collect::<Vec<_>>();
    if events.len() != 2 {
        return Err("Body admission did not produce exact membership and presence events".into());
    }
    biography
        .append_membership_events(biography.membership.clone(), &events)
        .map_err(|error| format!("retain Body admission evidence: {error:?}"))?;
    let receipt = PortableAdmissionReceipt {
        schema: "conduit.body/spawn-admission-receipt@1".into(),
        credential,
        current_offers_available: !request.host_advertisement.capabilities.is_empty(),
        host_advertisement: request.host_advertisement,
        membership_admitted: true,
        plan_created: false,
        play_created: false,
    };
    let biography_bytes = serde_json::to_vec_pretty(&biography)
        .map_err(|error| format!("encode admitted Body biography: {error}"))?;
    installation
        .body_state
        .as_mut()
        .expect("owned Body checked above")
        .biography_sha256 = digest(&biography_bytes);
    let transaction_path = state_dir.join("body/admission-transaction.json");
    write_json_atomic(
        &transaction_path,
        &AdmissionTransaction {
            schema: "conduit.body/admission-transaction@1".into(),
            admission,
            biography,
            installation,
            receipt: receipt.clone(),
        },
    )?;
    recover_admission_transaction(state_dir, &biography_path)?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("encode Body admission receipt: {error}"))?
    );
    Ok(())
}

fn recover_admission_transaction(state_dir: &Path, biography_path: &Path) -> Result<(), String> {
    let transaction_path = state_dir.join("body/admission-transaction.json");
    if !transaction_path.exists() {
        return Ok(());
    }
    let transaction: AdmissionTransaction =
        serde_json::from_slice(&bounded_read(&transaction_path, 2 * 1024 * 1024)?)
            .map_err(|error| format!("Body admission transaction: {error}"))?;
    let biography_bytes = serde_json::to_vec_pretty(&transaction.biography)
        .map_err(|error| format!("encode Body admission transaction biography: {error}"))?;
    if transaction.schema != "conduit.body/admission-transaction@1"
        || transaction.admission.body_id != transaction.biography.body_id
        || transaction.receipt.credential.body_id != transaction.biography.body_id
        || transaction
            .installation
            .body_state
            .as_ref()
            .is_none_or(|body| {
                body.body_id != transaction.biography.body_id.as_str()
                    || body.biography_path != biography_path.display().to_string()
                    || body.biography_sha256 != digest(&biography_bytes)
            })
    {
        return Err("Body admission transaction lost its exact Body identity".into());
    }
    transaction
        .biography
        .validate()
        .map_err(|error| format!("Body admission transaction biography refused: {error:?}"))?;
    write_json_atomic(biography_path, &transaction.biography)?;
    write_json_atomic(
        &state_dir.join("body/admission.json"),
        &transaction.admission,
    )?;
    write_json_atomic(
        &state_dir.join("installation.json"),
        &transaction.installation,
    )?;
    fs::remove_file(&transaction_path)
        .map_err(|error| format!("finish Body admission transaction: {error}"))?;
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
        schema: "conduit.body/spawn-admission-request@1".into(),
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
            schema: "conduit.body/pending-join@1".into(),
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
