//! Shared descriptive fingerprints of original independently approved material.
//! These functions perform no validation, authorization, installation or IO.
use sha2::{Digest, Sha256};

/// Public finite installation bounds in the exact scope transcript. Buffered
/// bytes use a fixed u64 width so MCU and owner produce identical fingerprints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForeCarrierScopeBounds {
    pub maximum_lines: u8,
    pub maximum_frame_bytes: u32,
    pub maximum_payload_bytes: u32,
    pub maximum_buffered_bytes: u64,
    pub maximum_in_flight_items: u16,
    pub maximum_frames: u32,
}
/// SHA256 of `conduit.root/fore-carrier-scope@1\0`, child-document SHA256[32],
/// image and configuration (each u32-LE UTF-8 byte length then bytes), followed
/// by lines u8, frame u32-LE, payload u32-LE, backing u64-LE, in-flight u16-LE,
/// frames u32-LE. The historical domain is preserved byte for byte.
///
/// Callers must independently obtain the original approved child and installed
/// identities/bounds. Hashing received metadata does not make it authoritative.
pub fn fore_carrier_scope_commitment(
    child_sha256: &[u8; 32],
    image: &str,
    configuration: &str,
    bounds: ForeCarrierScopeBounds,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.root/fore-carrier-scope@1\0");
    hash.update(child_sha256);
    for text in [image, configuration] {
        hash.update((text.len() as u32).to_le_bytes());
        hash.update(text.as_bytes());
    }
    hash.update([bounds.maximum_lines]);
    hash.update(bounds.maximum_frame_bytes.to_le_bytes());
    hash.update(bounds.maximum_payload_bytes.to_le_bytes());
    hash.update(bounds.maximum_buffered_bytes.to_le_bytes());
    hash.update(bounds.maximum_in_flight_items.to_le_bytes());
    hash.update(bounds.maximum_frames.to_le_bytes());
    hash.finalize().into()
}
/// SHA256 of `conduit.root/external-fore-carrier@1\0` || full scope[32] ||
/// original public command custody transcript[32] || exact canonical Line/Fore
/// row[198]. All bytes are public; the fingerprint proves no custody or grant.
/// Both endpoints must independently pin these original inputs before comparing
/// a received scope. In particular a received scope is never its own authority.
pub fn external_fore_scope_commitment(
    full_scope: &[u8; 32],
    custody: &[u8; 32],
    line_payload: &[u8; conduit_core::ASSIGNED_LINE_FORE_BYTES],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.root/external-fore-carrier@1\0");
    hash.update(full_scope);
    hash.update(custody);
    hash.update(line_payload);
    hash.finalize().into()
}
#[cfg(test)]
mod tests;
