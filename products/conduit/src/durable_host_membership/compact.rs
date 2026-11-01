//! Membership-only admission for the actual installed Host. No equipment offer,
//! resource grant, Plan or Play is synthesized by this exchange.
use super::*;
use conduit_body::{
    PortableInvitation, PortableMembershipAdmissionReceipt, PortableMembershipAdmissionRequest,
    SpawnInvitationSecret, MEMBERSHIP_ADMISSION_REQUEST_SCHEMA,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invitation {
    invitation: PortableInvitation,
    root_key: [u8; 32],
    producer_host: conduit_core::HostId,
    producer_boot: conduit_core::BootId,
}
impl Drop for Invitation {
    fn drop(&mut self) {
        self.invitation.secret.fill(0);
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    invitation: Invitation,
    request: PortableMembershipAdmissionRequest,
}
impl Pending {
    fn prepare(
        invitation: Invitation,
        root: [u8; 32],
        actual: &HostAdvertisement,
    ) -> Result<Self, String> {
        if invitation.root_key != root
            || root == [0; 32]
            || invitation.producer_host.as_str().is_empty()
            || invitation.producer_boot.as_str().is_empty()
        {
            return Err("membership invitation differs from independently pinned Root".into());
        }
        // The issuer owns the invitation's clock. Only it can check expiry;
        // never compare a remote Boot coordinate with this machine's wall time.
        invitation
            .invitation
            .validate(0)
            .map_err(|e| e.to_string())?;
        let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.invitation.secret)
            .map_err(|e| format!("membership invitation secret: {e:?}"))?;
        let claim = &invitation.invitation.claim;
        let request = PortableMembershipAdmissionRequest {
            schema: MEMBERSHIP_ADMISSION_REQUEST_SCHEMA.into(),
            protocol_version: conduit_core::PROTOCOL_VERSION,
            invitation_id: claim.invitation_id.clone(),
            body_id: claim.body_id.clone(),
            host_id: actual.host_id.clone(),
            boot_id: actual.boot_id.clone(),
            offer_generation: actual.offer_generation,
            nonce: claim.nonce,
            signature: secret.sign(&claim.signing_transcript(
                &actual.host_id,
                &actual.boot_id,
                actual.offer_generation,
            )),
        };
        request
            .validate()
            .map_err(|e| format!("membership request: {e:?}"))?;
        let encoded = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        PortableMembershipAdmissionRequest::decode(&encoded)
            .map_err(|e| format!("bounded membership request: {e:?}"))?;
        Ok(Self {
            invitation,
            request,
        })
    }
    fn validate_current(&self, actual: &HostAdvertisement) -> Result<(), String> {
        if self.request.host_id != actual.host_id
            || self.request.boot_id != actual.boot_id
            || self.request.offer_generation != actual.offer_generation
        {
            return Err(
                "membership attempt differs from current installed Host/Boot/generation".into(),
            );
        }
        Ok(())
    }
    fn verify(&self, receipt: &PortableMembershipAdmissionReceipt) -> Result<(), String> {
        receipt
            .verify(&self.invitation.root_key, &self.request)
            .map_err(|e| format!("membership Root receipt: {e:?}"))?;
        if receipt.producer_host != self.invitation.producer_host
            || receipt.producer_boot != self.invitation.producer_boot
        {
            return Err("membership receipt changed the invited producer incarnation".into());
        }
        Ok(())
    }
}
fn current(state: &Path, installation: &Installation) -> Result<HostAdvertisement, String> {
    let running: RuntimeStatus =
        serde_json::from_slice(&bounded_read(&state.join("runtime.json"), 64 * 1024)?)
            .map_err(|e| format!("membership runtime marker: {e}"))?;
    if running.schema != RUNTIME_SCHEMA {
        return Err("membership runtime schema".into());
    }
    let actual = crate::durable_host_control::current(state)?.advertisement;
    if actual.host_id.as_str() != installation.host_id
        || actual.host_id.as_str() != running.host_id
        || actual.boot_id.as_str() != running.boot_id
        || actual.offer_generation.0 < running.offer_generation
    {
        return Err("membership control truth differs from installed runtime".into());
    }
    Ok(actual)
}
fn unjoined(state: &Path) -> Result<Installation, String> {
    let installation = read_installation(&state.join("installation.json"))?;
    if installation.body_state.is_some() || installation.joined_body_state.is_some() {
        return Err("this installed Host already owns or has joined a Body".into());
    }
    Ok(installation)
}
fn pin(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Root pin must contain exactly 64 hexadecimal characters".into());
    }
    let mut root = [0; 32];
    for (out, pair) in root.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *out = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    }
    Ok(root)
}
pub(crate) fn accept(
    invitation_path: &Path,
    state: &Path,
    root_pin: &str,
    authorized: bool,
) -> Result<(), String> {
    if !authorized {
        return Err("membership admission requires --authorize-join".into());
    }
    let installation = unjoined(state)?;
    let actual = current(state, &installation)?;
    let invitation: Invitation = serde_json::from_slice(&bounded_read(invitation_path, 16_384)?)
        .map_err(|e| format!("membership invitation: {e}"))?;
    let pending = Pending::prepare(invitation, pin(root_pin)?, &actual)?;
    let directory = state.join("body");
    fs::create_dir_all(&directory).map_err(|e| format!("create membership directory: {e}"))?;
    restrict_directory(&directory)?;
    let path = directory.join("membership-join-private.json");
    if path.exists() {
        let retained: Pending = serde_json::from_slice(&bounded_read(&path, 16_384)?)
            .map_err(|e| format!("retained membership attempt: {e}"))?;
        retained.validate_current(&actual)?;
        if retained.request != pending.request
            || retained.invitation.root_key != pending.invitation.root_key
            || retained.invitation.producer_host != pending.invitation.producer_host
            || retained.invitation.producer_boot != pending.invitation.producer_boot
        {
            return Err("another retained membership attempt already owns this Host".into());
        }
    } else {
        let temporary = directory.join(format!(
            "membership-attempt-{}.json",
            crate::birth_identity::fresh_uuid()?
        ));
        write_json_atomic(&temporary, &pending)?;
        let published = fs::hard_link(&temporary, &path)
            .map_err(|e| format!("retain exclusive membership attempt: {e}"));
        let synced = published.and_then(|()| {
            fs::File::open(&directory)
                .and_then(|directory| directory.sync_all())
                .map_err(|e| e.to_string())
        });
        let _ = fs::remove_file(&temporary);
        synced?;
    }
    let encoded = serde_json::to_vec(&pending.request).map_err(|e| e.to_string())?;
    PortableMembershipAdmissionRequest::decode(&encoded)
        .map_err(|e| format!("bounded membership request: {e:?}"))?;
    println!(
        "{}",
        std::str::from_utf8(&encoded).map_err(|e| e.to_string())?
    );
    Ok(())
}
pub(crate) fn complete(receipt_path: &Path, state: &Path, authorized: bool) -> Result<(), String> {
    if !authorized {
        return Err("retaining membership requires --authorize-membership".into());
    }
    let mut installation = read_installation(&state.join("installation.json"))?;
    if installation.body_state.is_some() {
        return Err("this installed Host already owns a Body".into());
    }
    let actual = current(state, &installation)?;
    let pending: Pending = serde_json::from_slice(&bounded_read(
        &state.join("body/membership-join-private.json"),
        16_384,
    )?)
    .map_err(|e| format!("retained membership attempt: {e}"))?;
    pending.validate_current(&actual)?;
    let receipt = PortableMembershipAdmissionReceipt::decode(&bounded_read(
        receipt_path,
        conduit_body::MAX_MEMBERSHIP_ADMISSION_RECEIPT_BYTES as u64,
    )?)
    .map_err(|e| format!("bounded membership receipt: {e:?}"))?;
    pending.verify(&receipt)?;
    if let Some(binding) = &installation.joined_body_state {
        super::validate(binding, &installation)?;
        let retained = PortableMembershipAdmissionReceipt::decode(&bounded_read(
            &state.join("body/membership-admission-receipt.json"),
            conduit_body::MAX_MEMBERSHIP_ADMISSION_RECEIPT_BYTES as u64,
        )?)
        .map_err(|e| format!("retained membership receipt: {e:?}"))?;
        pending.verify(&retained)?;
        let credential =
            serde_json::to_vec_pretty(&receipt.credential).map_err(|e| e.to_string())?;
        if retained != receipt || binding.credential_sha256 != digest(&credential) {
            return Err("different membership is already retained by this installed Host".into());
        }
        return Ok(());
    }
    // Keep the original private attempt for reconciliation and future return;
    // transport delivery never substitutes for this verified Root receipt.
    let encoded = serde_json::to_vec(&receipt).map_err(|e| e.to_string())?;
    PortableMembershipAdmissionReceipt::decode(&encoded)
        .map_err(|e| format!("retained bounded membership receipt: {e:?}"))?;
    crate::durable_host::write_bytes_atomic(
        &state.join("body/membership-admission-receipt.json"),
        &encoded,
    )?;
    let retained_state =
        fs::canonicalize(state).map_err(|e| format!("membership state path: {e}"))?;
    super::persist_joined_membership(&retained_state, &mut installation, &receipt.credential)
}

#[cfg(test)]
#[path = "compact_tests.rs"]
mod tests;
