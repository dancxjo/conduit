//! Durable completion and observation of an admitted Body membership.

use super::invitation::{PendingBodyJoin, PortableAdmissionReceipt};
use super::{
    bounded_read, digest, observe_current_runtime, read_installation, write_json_atomic,
    Installation, RuntimeStatus, RUNTIME_SCHEMA,
};
use conduit_body::MembershipCredential;
use conduit_core::HostAdvertisement;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct JoinedBodyBinding {
    pub(super) body_id: String,
    pub(super) part_id: String,
    pub(super) credential_sha256: String,
    pub(super) credential_path: String,
}

pub(crate) fn complete_body_join(
    receipt_path: &Path,
    state_dir: &Path,
    authorize_membership: bool,
) -> Result<(), String> {
    if !authorize_membership {
        return Err("retaining admitted Body membership requires --authorize-membership".into());
    }
    let install_path = state_dir.join("installation.json");
    let mut installation = read_installation(&install_path)?;
    if installation.body_state.is_some() || installation.joined_body_state.is_some() {
        return Err("this installed Host already has current Body state".into());
    }
    let pending_path = state_dir.join("body/pending-join.json");
    let pending: PendingBodyJoin =
        serde_json::from_slice(&bounded_read(&pending_path, 256 * 1024)?)
            .map_err(|error| format!("pending Body join: {error}"))?;
    if pending.schema != "conduit.body/pending-join@1" {
        return Err("pending Body join has an unsupported schema".into());
    }
    let receipt: PortableAdmissionReceipt =
        serde_json::from_slice(&bounded_read(receipt_path, 256 * 1024)?)
            .map_err(|error| format!("Body admission receipt: {error}"))?;
    let request = &pending.request;
    if receipt.schema != "conduit.body/spawn-admission-receipt@1"
        || !receipt.membership_admitted
        || receipt.plan_created
        || receipt.play_created
        || receipt.current_offers_available != !request.host_advertisement.capabilities.is_empty()
        || receipt.host_advertisement != request.host_advertisement
        || receipt.credential.body_id != request.body_id
        || receipt.credential.host_id != request.host_advertisement.host_id
        || receipt.credential.boot_id != request.host_advertisement.boot_id
        || receipt.credential.host_id.as_str() != installation.host_id
    {
        return Err(
            "Body admission receipt lost its exact pending join identity or claims effects".into(),
        );
    }
    persist_joined_membership(state_dir, &mut installation, &receipt.credential)?;
    fs::remove_file(&pending_path)
        .map_err(|error| format!("consume pending Body join: {error}"))?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("encode retained admission receipt: {error}"))?
    );
    Ok(())
}

pub(crate) fn retain_rendezvous_membership(
    state_dir: &Path,
    credential: &MembershipCredential,
    expected_body_id: &str,
    expected_advertisement: &HostAdvertisement,
) -> Result<(), String> {
    let mut installation = read_installation(&state_dir.join("installation.json"))?;
    if installation.body_state.is_some() || installation.joined_body_state.is_some() {
        return Err("this installed Host already has current Body state".into());
    }
    if credential.body_id.as_str() != expected_body_id
        || credential.host_id != expected_advertisement.host_id
        || credential.boot_id != expected_advertisement.boot_id
        || credential.host_id.as_str() != installation.host_id
    {
        return Err("rendezvous admission receipt lost its exact Body, Host, or Boot identity".into());
    }
    persist_joined_membership(state_dir, &mut installation, credential)
}

fn persist_joined_membership(
    state_dir: &Path,
    installation: &mut Installation,
    credential: &MembershipCredential,
) -> Result<(), String> {
    let credential_path = state_dir.join("body/membership-credential.json");
    let credential_bytes = serde_json::to_vec_pretty(credential)
        .map_err(|error| format!("encode membership credential: {error}"))?;
    write_json_atomic(&credential_path, credential)?;
    installation.joined_body_state = Some(JoinedBodyBinding {
        body_id: credential.body_id.as_str().into(),
        part_id: credential.part_id.as_str().into(),
        credential_sha256: digest(&credential_bytes),
        credential_path: credential_path.display().to_string(),
    });
    let runtime_path = state_dir.join("runtime.json");
    let current_runtime = if runtime_path.exists() {
        let mut runtime: RuntimeStatus =
            serde_json::from_slice(&bounded_read(&runtime_path, 64 * 1024)?)
                .map_err(|error| format!("durable Host runtime status: {error}"))?;
        if runtime.schema != RUNTIME_SCHEMA
            || runtime.host_id != installation.host_id
            || runtime.boot_id != credential.boot_id.as_str()
            || runtime.body_id.is_some()
        {
            return Err("current runtime differs from the Boot that was admitted".into());
        }
        runtime.body_id = Some(credential.body_id.as_str().into());
        Some(runtime)
    } else {
        None
    };
    write_json_atomic(&state_dir.join("installation.json"), installation)?;
    if let Some(runtime) = current_runtime {
        write_json_atomic(&runtime_path, &runtime)?;
    }
    Ok(())
}

pub(super) fn status(
    state_dir: &Path,
    installation: &Installation,
    binding: &JoinedBodyBinding,
    json: bool,
) -> Result<(), String> {
    validate(binding, installation)?;
    let runtime = observe_current_runtime(state_dir, installation)?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "schema": "conduit.body/installed-status@1",
                "body_id": binding.body_id,
                "part_id": binding.part_id,
                "membership_credential_sha256": binding.credential_sha256,
                "host_id": installation.host_id,
                "boot_id": runtime.as_ref().map(|status| status.boot_id.as_str()),
                "offer_generation": runtime.as_ref().map(|status| status.offer_generation),
                "presence": if runtime.is_some() { "current" } else { "admitted-offline" },
                "membership": "admitted",
                "plan_created": false,
                "play_created": false,
            })
        );
    } else {
        println!("Body {}", binding.body_id);
        println!("admitted Part {}", binding.part_id);
        match runtime {
            Some(status) => println!(
                "current Host {} boot {} offers generation {}",
                status.host_id, status.boot_id, status.offer_generation
            ),
            None => println!("Host {} is admitted but offline", installation.host_id),
        }
    }
    Ok(())
}

pub(super) fn validate(
    binding: &JoinedBodyBinding,
    installation: &Installation,
) -> Result<(), String> {
    let credential_bytes = bounded_read(Path::new(&binding.credential_path), 64 * 1024)?;
    if binding.body_id.is_empty()
        || binding.part_id.is_empty()
        || digest(&credential_bytes) != binding.credential_sha256
    {
        return Err("retained Body membership credential is invalid or stale".into());
    }
    let credential: MembershipCredential = serde_json::from_slice(&credential_bytes)
        .map_err(|error| format!("retained membership credential: {error}"))?;
    if credential.body_id.as_str() != binding.body_id
        || credential.part_id.as_str() != binding.part_id
        || credential.host_id.as_str() != installation.host_id
    {
        return Err("retained membership credential belongs to another Body or Host".into());
    }
    Ok(())
}
