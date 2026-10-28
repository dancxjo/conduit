//! Incremental verification of the exact native frozen owner. No wire value
//! constructs Verified; neither inspection nor its digest grants erase custody.
use super::Inspection;
use conduit_network_join_call::recovery_evidence::CAPACITY;
use sha2::{Digest, Sha256};

pub(crate) struct Verifier {
    owner: Inspection,
    hash: Sha256,
    next: u32,
    failed: bool,
}
impl Verifier {
    pub(crate) fn start(owner: Inspection) -> Self {
        Self {
            owner,
            hash: Sha256::new(),
            next: 0,
            failed: false,
        }
    }
    /// One bounded read per Root service turn. Failure is terminal even if a
    /// transport retries, so bytes cannot be skipped or hashed twice.
    pub(crate) fn step(&mut self, work: &mut [u8]) -> Result<bool, &'static str> {
        if self.failed {
            return Err("network-snapshot-failed");
        }
        if work.is_empty() || work.len() > 512 {
            self.failed = true;
            return Err("network-snapshot-workspace");
        }
        if self.next == CAPACITY {
            return Ok(true);
        }
        self.failed = true;
        let count = work.len().min((CAPACITY - self.next) as usize);
        self.owner.read_chunk(self.next, &mut work[..count])?;
        self.hash.update(&work[..count]);
        self.next += count as u32;
        self.failed = false;
        Ok(self.next == CAPACITY)
    }
    pub(crate) fn finish(self) -> Result<Verified, &'static str> {
        if self.failed || self.next != CAPACITY {
            return Err("network-snapshot-incomplete");
        }
        Ok(Verified {
            owner: self.owner,
            digest: self.hash.finalize().into(),
        })
    }
}
/// Keeps the physical owner pinned while an exact offer or acknowledgment is
/// checked. Dropping this value restores the same journal without programming.
pub(crate) struct Verified {
    owner: Inspection,
    digest: [u8; 32],
}
impl Verified {
    pub(crate) fn region_identity(&self) -> [u8; 32] {
        self.owner.region_identity()
    }
    pub(crate) fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub(crate) fn committed_count(&self) -> u8 {
        self.owner.committed_count()
    }
    pub(crate) fn read_chunk(&mut self, at: u32, out: &mut [u8]) -> Result<(), &'static str> {
        self.owner.read_chunk(at, out)
    }
    /// The immutable owner lives through the entire synchronous Root check.
    pub(crate) fn finish_with<T>(self, check: impl FnOnce(&Self) -> T) -> T {
        check(&self)
    }
}
