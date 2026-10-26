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
