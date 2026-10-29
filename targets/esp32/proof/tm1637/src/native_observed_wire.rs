//! Versioned observation-outbox transcript. Verification additionally requires
//! the independently admitted producer key, exact Body/Boot/Play scope, Forms,
//! canonical records, sequence continuity and durable consumer acceptance.
use sha2::{Digest, Sha256};
pub fn transcript(
    scope: &[u8; 96],
    forms: &[u8; 32],
    after: u32,
    committed: u32,
    records: &[u8],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/observed-history@1\0");
    hash.update(scope);
    hash.update(forms);
    hash.update(after.to_le_bytes());
    hash.update(committed.to_le_bytes());
    hash.update((records.len() as u64).to_le_bytes());
    hash.update(records);
    hash.finalize().into()
}

/// Authentication of one canonical reading before durable journal commit.
/// The scope retains the original Body, Boot and sampler Play digests.
pub fn record_transcript(scope: &[u8; 96], forms: &[u8; 32], record: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/observed-record@1\0");
    hash.update(scope);
    hash.update(forms);
    hash.update((record.len() as u64).to_le_bytes());
    hash.update(record);
    hash.finalize().into()
}

pub const DIRECTORY_BYTES: usize = 33 + 5 * 133;
pub const DIRECTORY_CURSOR: u32 = u32::MAX;
pub fn directory_transcript(bytes: &[u8; DIRECTORY_BYTES]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/observed-epochs@1\0");
    hash.update(bytes);
    hash.finalize().into()
}
/// Slot indices are ephemeral read-only addresses, never identity or authority.
pub fn epoch_cursor(slot: u8, after: u32) -> Option<u32> {
    if slot > 5 || after > 960 {
        return None;
    }
    Some((u32::from(slot) << 24) | after)
}
pub fn split_cursor(cursor: u32) -> Option<(u8, u32)> {
    let slot = (cursor >> 24) as u8;
    let after = cursor & 0x00ff_ffff;
    epoch_cursor(slot, after).map(|_| (slot, after))
}
