use conduit_body::BodyPlan;
use sha2::{Digest, Sha256};

pub(super) fn execution_envelope(plan: &BodyPlan) -> u64 {
    digest_u64(&[plan.body_id.as_str(), plan.wake_id.as_str()])
}

pub(super) fn evidence_sign(sign: &conduit_core::SignId, sequence: u32) -> u64 {
    let sequence = sequence.to_le_bytes();
    let mut digest = Sha256::new();
    digest.update(sign.as_str().as_bytes());
    digest.update(sequence);
    let digest = digest.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

pub(super) fn digest_u64(parts: &[&str]) -> u64 {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part.as_bytes());
    }
    let digest = digest.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}
