//! One bounded recoverable admission outcome, never transport-delivery custody.
use super::*;
use conduit_body::{
    AdmissionManager, BodyBiographyEvidence, MembershipCredential,
    PortableMembershipAdmissionReceipt, PortableMembershipAdmissionRequest, SpawnInvitationClaim,
    MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA,
};
use conduit_core::HostAdvertisement;
use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};
use std::io::Read;
const PATH: &str = "body/root-admission-receipt.json";
const MAXIMUM_DRAFT: usize = 4096;
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Draft {
    schema: String,
    request: PortableMembershipAdmissionRequest,
    claim: SpawnInvitationClaim,
    offered_sha256: String,
    browser_host_key: [u8; 32],
    receipt: PortableMembershipAdmissionReceipt,
}
fn offered_digest(advertisement: &HostAdvertisement) -> Result<String, String> {
    serde_json::to_vec(advertisement)
        .map(|v| crate::durable_host::digest(&v))
        .map_err(|e| e.to_string())
}
impl Draft {
    pub(crate) fn credential(&self) -> &MembershipCredential {
        &self.receipt.credential
    }
    pub(crate) fn matches(
        &self,
        request: &PortableMembershipAdmissionRequest,
        claim: &SpawnInvitationClaim,
        offered: &HostAdvertisement,
        key: &[u8; 32],
    ) -> Result<bool, String> {
        Ok(self.request == *request
            && self.claim == *claim
            && self.browser_host_key == *key
            && self.offered_sha256 == offered_digest(offered)?)
    }
    pub(crate) fn validate(&self, root: &Path, body: &str, host: &str) -> Result<(), String> {
        let encoded = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if encoded.len() > MAXIMUM_DRAFT
            || self.schema != "conduit.body/installed-admission-draft@1"
            || self.receipt.signature != [0; 64]
            || self.receipt.producer_host.as_str() != host
            || self.receipt.credential.body_id.as_str() != body
            || self.browser_host_key == [0; 32]
            || self.request.invitation_id != self.claim.invitation_id
            || self.request.body_id != self.claim.body_id
            || self.request.nonce != self.claim.nonce
            || !crate::durable_host::valid_digest(&self.offered_sha256)
        {
            return Err("invalid installed admission draft scope/bound".into());
        }
        self.claim
            .inspect(0)
            .map_err(|e| format!("retained admission claim: {e:?}"))?;
        PortableMembershipAdmissionRequest::decode(
            &serde_json::to_vec(&self.request).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("bounded retained admission request: {e:?}"))?;
        PortableMembershipAdmissionReceipt::decode(
            &serde_json::to_vec(&self.receipt).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("bounded retained admission receipt: {e:?}"))?;
        self.receipt
            .validate_against(&self.request)
            .map_err(|e| format!("retained admission receipt: {e:?}"))?;
        let binding = selected(root, body, host)?.ok_or("receipt has no selected Root signer")?;
        if binding.public_key != self.receipt.producer_key {
            return Err("receipt changed selected Root identity".into());
        }
        Ok(())
    }
    pub(crate) fn require_current(
        &self,
        biography: &BodyBiographyEvidence,
        manager: &AdmissionManager,
    ) -> Result<(), String> {
        let credential = &self.receipt.credential;
        if biography.body_id != credential.body_id
            || manager.body_id != credential.body_id
            || biography.membership.revision.0 < self.receipt.membership_revision
            || manager
                .receipts
                .iter()
                .rev()
                .find(|v| v.credential.part_id == credential.part_id.as_str())
                .is_none_or(|v| !v.credential.matches(credential))
            || !biography.membership.parts.iter().any(|part| {
                part.part_id == credential.part_id
                    && part.state == conduit_body::MembershipState::Admitted
                    && part.current.as_ref().is_none_or(|current| {
                        current.host_id == credential.host_id
                            && current.boot_id == credential.boot_id
                            && current.offer_generation == self.request.offer_generation
                    })
            })
        {
            return Err(
                "retained admission outcome is no longer the current Part incarnation".into(),
            );
        }
        self.claim
            .inspect(credential.issued_at_millis)
            .map_err(|e| format!("original admission expiry: {e:?}"))?;
        manager
            .verify_retained_custody_message(
                &biography.membership,
                credential,
                &self.claim.signing_transcript(
                    &self.request.host_id,
                    &self.request.boot_id,
                    self.request.offer_generation,
                ),
                &self.request.signature,
            )
            .map_err(|e| format!("canonical admitted request proof: {e:?}"))?;
        Ok(())
    }
}
pub(crate) fn load(root: &Path, body: &str, host: &str) -> Result<Option<Draft>, String> {
    let path = root.join(PATH);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(v) if !v.is_file() || v.file_type().is_symlink() => {
            return Err("invalid retained admission outcome file".into())
        }
        Ok(_) => {}
    }
    let draft: Draft = serde_json::from_slice(&crate::durable_host::bounded_read(
        &path,
        MAXIMUM_DRAFT as u64,
    )?)
    .map_err(|_| "invalid retained admission outcome")?;
    draft.validate(root, body, host)?;
    Ok(Some(draft))
}
/// A different outcome cannot evict this one before client durable custody exists.
/// Currently one admission slot; no transport-send acknowledgment retires it.
pub(crate) fn reserve(
    root: &Path,
    body: &str,
    host: &str,
    request: &PortableMembershipAdmissionRequest,
    claim: &SpawnInvitationClaim,
    offered: &HostAdvertisement,
    key: &[u8; 32],
) -> Result<Option<Draft>, String> {
    if let Some(draft) = load(root, body, host)? {
        if !draft.matches(request, claim, offered, key)? {
            return Err("finite committed admission receipt slot is occupied".into());
        }
        return Ok(Some(draft));
    }
    Ok(None)
}
pub(crate) fn prepare(
    root: &Path,
    biography: &BodyBiographyEvidence,
    manager: &AdmissionManager,
    authority: &HostAdvertisement,
    request: PortableMembershipAdmissionRequest,
    claim: SpawnInvitationClaim,
    offered: &HostAdvertisement,
    browser_host_key: [u8; 32],
    credential: MembershipCredential,
) -> Result<Draft, String> {
    let binding = selected(root, biography.body_id.as_str(), authority.host_id.as_str())?
        .ok_or("Root signer not established")?;
    let draft = Draft {
        schema: "conduit.body/installed-admission-draft@1".into(),
        receipt: PortableMembershipAdmissionReceipt {
            schema: MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA.into(),
            protocol_version: conduit_core::PROTOCOL_VERSION,
            credential,
            membership_revision: biography.membership.revision.0,
            request_digest: request.digest(),
            producer_host: authority.host_id.clone(),
            producer_boot: authority.boot_id.clone(),
            producer_key: binding.public_key,
            invitation_id: request.invitation_id.clone(),
            offer_generation: request.offer_generation,
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
            signature: [0; 64],
        },
        request,
        claim,
        offered_sha256: offered_digest(offered)?,
        browser_host_key,
    };
    draft.validate(root, biography.body_id.as_str(), authority.host_id.as_str())?;
    draft.require_current(biography, manager)?;
    Ok(draft)
}
pub(crate) fn commit(root: &Path, draft: &Draft, body: &str, host: &str) -> Result<(), String> {
    draft.validate(root, body, host)?;
    if load(root, body, host)?.as_ref().is_some_and(|v| v != draft) {
        return Err("committed admission receipt replacement refused".into());
    }
    let encoded = serde_json::to_vec(draft).map_err(|e| e.to_string())?;
    // The on-disk bound covers actual bytes, including encoding whitespace.
    crate::durable_host::write_bytes_atomic(&root.join(PATH), &encoded)
}
/// No arbitrary transcript/signing entrance. Only exact recovered committed draft.
pub(crate) fn sign_recovered(
    root: &Path,
    expected: &Draft,
    biography: &BodyBiographyEvidence,
    manager: &AdmissionManager,
) -> Result<PortableMembershipAdmissionReceipt, String> {
    if root
        .join("body/owner-transaction.json")
        .try_exists()
        .map_err(|e| e.to_string())?
    {
        return Err("admission outcome still has an unfinished owner transaction".into());
    }
    let installation = crate::durable_host::read_installation(&root.join("installation.json"))?;
    let bound = installation
        .body_state
        .as_ref()
        .ok_or("receipt signer has no authoritative installed Body")?;
    let expected_biography = serde_json::to_vec_pretty(biography).map_err(|e| e.to_string())?;
    let expected_manager = serde_json::to_vec_pretty(manager).map_err(|e| e.to_string())?;
    if installation.host_id != expected.receipt.producer_host.as_str()
        || bound.body_id != biography.body_id.as_str()
        || Path::new(&bound.biography_path) != root.join("body/biography.json")
        || bound.biography_sha256 != crate::durable_host::digest(&expected_biography)
        || file_digest(Path::new(&bound.biography_path))? != bound.biography_sha256
        || file_digest(&root.join("body/admission.json"))?
            != crate::durable_host::digest(&expected_manager)
    {
        return Err("receipt signing context differs from committed owner state".into());
    }
    let body = biography.body_id.as_str();
    let host = expected.receipt.producer_host.as_str();
    let draft = load(root, body, host)?.ok_or("admission receipt has not been committed")?;
    if &draft != expected {
        return Err("recovered admission receipt differs from requested outcome".into());
    }
    draft.require_current(biography, manager)?;
    let (_, key) = read_private(root, body, host)?;
    let mut receipt = draft.receipt.clone();
    receipt.signature = key.sign(&receipt.transcript()).to_bytes();
    receipt
        .verify(&key.verifying_key().to_bytes(), &draft.request)
        .map_err(|e| format!("signed Root receipt: {e:?}"))?;
    Ok(receipt)
}

// Recheck durable context without allocating another decoded biography/manager.
fn file_digest(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut total = 0usize;
    let mut buffer = [0u8; 4096];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count)
            .ok_or("owner context size overflow")?;
        if total > 2 * 1024 * 1024 {
            return Err("owner context exceeds existing finite state bound".into());
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}
