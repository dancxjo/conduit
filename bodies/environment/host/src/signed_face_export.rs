//! Reusable pinned-Root Face verification, with no transport or authority grant.
//! A verified offer binds one nonce, Body, producer Boot and exact document.
//! Callers still enforce elapsed local time and selected execution admission;
//! this is neither a durable recording receipt nor proof of MCU membership.
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt::Debug};
pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn checked<T, E: Debug>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|e| format!("{e:?}").into())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Offer {
    profile: String,
    producer_boot: String,
    body_id: conduit_body::BodyId,
    offer: String,
}
pub struct VerifiedSnapshot {
    body: conduit_body::BodyId,
    producer_boot: conduit_core::BootId,
    digest: [u8; 32],
    revision: u64,
    bytes: u32,
    lifetime: u64,
}
impl VerifiedSnapshot {
    pub fn producer_boot(&self) -> &conduit_core::BootId {
        &self.producer_boot
    }
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn bytes(&self) -> u32 {
        self.bytes
    }
    /// Original signed lifetime, never a new lease beginning on arrival.
    pub fn lifetime(&self) -> u64 {
        self.lifetime
    }
    /// Verify the complete original bytes before interpreting any quantities.
    pub fn decode_document(&self, document: &[u8]) -> Result<conduit_face::Face> {
        if document.len() != self.bytes as usize || Sha256::digest(document)[..] != self.digest {
            return Err("Face export document digest/extent".into());
        }
        let face: conduit_face::Face = serde_json::from_slice(document)?;
        checked(face.validate())?;
        if face.revision != self.revision
            || face.basis.body_id.as_ref() != Some(&self.body)
            || face.basis.wake_id.is_none()
            || face.basis.source_document_id.is_none()
            || face.basis.checked_plot_id.is_none()
            || face.basis.expanded_plot_id.is_none()
            || face.basis.plan_id.is_none()
            || face.basis.active_play_id.is_none()
        {
            return Err("Face export execution basis".into());
        }
        Ok(face)
    }
}
fn unhex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2)
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Face export hex".into());
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| Ok(u8::from_str_radix(std::str::from_utf8(p)?, 16)?))
        .collect()
}
pub fn verify_snapshot(
    root: &[u8; 32],
    body: &conduit_body::BodyId,
    nonce: &[u8; 32],
    bytes: &[u8],
) -> Result<VerifiedSnapshot> {
    if bytes.len() > 1408 {
        return Err("Face export offer capacity".into());
    }
    let reply: Offer = serde_json::from_slice(bytes)?;
    let offer = unhex(&reply.offer)?;
    if reply.profile != "conduit.face/export@1"
        || reply.body_id != *body
        || reply.producer_boot.is_empty()
        || reply.producer_boot.len() > 256
        || offer.len() != 220
    {
        return Err("Face export scope/extent".into());
    }
    let mut hash = Sha256::new();
    hash.update(b"conduit.face/export@1\0");
    hash.update(root);
    hash.update(&offer[..156]);
    checked(checked(VerifyingKey::from_bytes(root))?.verify_strict(
        &hash.finalize(),
        &Signature::from_bytes(offer[156..].try_into()?),
    ))?;
    if offer[..32] != *nonce
        || offer[32..64] != Sha256::digest(body.as_str().as_bytes())[..]
        || offer[64..96] != Sha256::digest(reply.producer_boot.as_bytes())[..]
    {
        return Err("Face export signed scope".into());
    }
    let revision = u64::from_le_bytes(offer[128..136].try_into()?);
    let length = u32::from_le_bytes(offer[136..140].try_into()?);
    let issued = u64::from_le_bytes(offer[140..148].try_into()?);
    let until = u64::from_le_bytes(offer[148..156].try_into()?);
    let lifetime = until
        .checked_sub(issued)
        .filter(|n| (1..=5000).contains(n))
        .ok_or("Face export lifetime")?;
    if !(1..=8192).contains(&length) {
        return Err("Face export document capacity".into());
    }
    Ok(VerifiedSnapshot {
        body: body.clone(),
        producer_boot: reply.producer_boot.into(),
        digest: offer[96..128].try_into()?,
        revision,
        bytes: length,
        lifetime,
    })
}

/// One complete authenticated document, retaining its original local deadline.
pub struct VerifiedFaceRead {
    pub face: conduit_face::Face,
    pub producer_boot: conduit_core::BootId,
    pub observed_at_boot_ms: u64,
    pub deadline_boot_ms: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chunk {
    profile: String,
    digest: String,
    offset: u32,
    bytes: String,
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
/// Retrieve through the caller's selected transport. The callback must enforce
/// the supplied absolute start plus 5000ms budget during blocking operations.
/// The clock must be the same local suspend-aware monotonic clock throughout.
/// This function grants no execution, membership or durable recording authority.
pub fn read_snapshot_with(
    root: &[u8; 32],
    body: &conduit_body::BodyId,
    nonce: &[u8; 32],
    mut get: impl FnMut(&str, u64) -> Result<Vec<u8>>,
    mut now: impl FnMut() -> Result<u64>,
) -> Result<VerifiedFaceRead> {
    let started = now()?;
    let offer = get(
        &format!("/face/export/snapshot?nonce={}", hex(nonce)),
        started,
    )?;
    let verified = verify_snapshot(root, body, nonce, &offer)?;
    let deadline = started
        .checked_add(verified.lifetime())
        .ok_or("Face export deadline")?;
    let mut document = Vec::with_capacity(verified.bytes() as usize);
    for offset in (0..verified.bytes()).step_by(512) {
        if now()? >= deadline {
            return Err("Face export expired in transit".into());
        }
        let path = format!(
            "/face/export/chunk?digest={}&revision={}&bytes={}&offset={offset}",
            hex(verified.digest()),
            verified.revision(),
            verified.bytes()
        );
        let reply = get(&path, started)?;
        if reply.len() > 1408 {
            return Err("Face export chunk capacity".into());
        }
        let chunk: Chunk = serde_json::from_slice(&reply)?;
        let bytes = unhex(&chunk.bytes)?;
        if chunk.profile != "conduit.face/chunk@1"
            || chunk.digest != hex(verified.digest())
            || chunk.offset != offset
            || bytes.len() != 512.min((verified.bytes() - offset) as usize)
        {
            return Err("Face export chunk scope".into());
        }
        document.extend_from_slice(&bytes);
    }
    let face = verified.decode_document(&document)?;
    let observed = now()?;
    if observed < started || observed.saturating_add(1) >= deadline {
        return Err("Face export expired in transit".into());
    }
    Ok(VerifiedFaceRead {
        face,
        producer_boot: verified.producer_boot().clone(),
        observed_at_boot_ms: observed,
        deadline_boot_ms: deadline,
    })
}
