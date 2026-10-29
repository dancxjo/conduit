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

/// Root supplies its current authenticated scope; the physical fields below
/// are always derived from Verified, never accepted from a transport request.
pub(crate) struct Scope<'a> {
    pub body: &'a conduit_body::BodyId,
    pub boot: &'a conduit_core::BootId,
    pub epoch: u64,
    pub predecessor: [u8; 32],
}
pub(crate) struct Draft {
    _owner: Verified,
    root: [u8; 32],
    payload: [u8; 176],
}
impl Verified {
    pub(crate) fn offer(self, scope: Scope<'_>, root: [u8; 32]) -> Result<Draft, &'static str> {
        let count = self.committed_count();
        if !(1..=8).contains(&count) {
            return Err("network-snapshot-empty");
        }
        let mut payload = [0; 176];
        payload[..32].copy_from_slice(&Sha256::digest(scope.body.as_str().as_bytes()));
        payload[32..64].copy_from_slice(&self.region_identity());
        payload[64..72].copy_from_slice(&scope.epoch.to_le_bytes());
        payload[72..104].copy_from_slice(&Sha256::digest(scope.boot.as_str().as_bytes()));
        payload[104..108].copy_from_slice(&CAPACITY.to_le_bytes());
        payload[108] = count;
        payload[112..144].copy_from_slice(self.digest());
        payload[144..176].copy_from_slice(&scope.predecessor);
        Ok(Draft {
            _owner: self,
            root,
            payload,
        })
    }
}
impl Draft {
    /// Only the durable Pending successor may consume the old signed view.
    /// Physical authority is independently minted from the still-leased Store.
    #[cfg(all(feature = "body-persistence", target_os = "none"))]
    pub(crate) fn into_closed(
        self,
        receipt: &crate::native_restart::network_pending::CommittedCustody,
    ) -> Result<Inspection, Self> {
        if receipt.offer() != &self.transcript() {
            return Err(self);
        }
        Ok(self._owner.owner)
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn matches_scope(&self, scope: Scope<'_>) -> bool {
        self.payload[..32] == Sha256::digest(scope.body.as_str().as_bytes())[..]
            && self.payload[64..72] == scope.epoch.to_le_bytes()
            && self.payload[72..104] == Sha256::digest(scope.boot.as_str().as_bytes())[..]
            && self.payload[144..176] == scope.predecessor
    }
    pub(crate) fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/network-retirement-offer@1\0");
        hash.update(self.root);
        hash.update(self.payload);
        hash.finalize().into()
    }
    /// Signature presentation only; the Root signer separately checks actual
    /// current durable scope. The same owner stays pinned for subsequent ACK.
    pub(crate) fn signed(&self, signature: [u8; 64]) -> [u8; 240] {
        let mut bytes = [0; 240];
        bytes[..176].copy_from_slice(&self.payload);
        bytes[176..].copy_from_slice(&signature);
        bytes
    }
}

#[path = "native_network_snapshot_export.rs"]
pub(crate) mod export;

#[path = "native_network_ack.rs"]
pub(crate) mod ack;
