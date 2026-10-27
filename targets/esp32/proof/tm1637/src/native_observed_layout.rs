//! Durable first-format provenance. Decoded receipts carry no authority until
//! verified against the independently pinned Body/Root and actual epoch owner.
use super::{Error, WholeEpoch, DATA, TOTAL};
use conduit_body::BodyId;
use conduit_environment_runtime::display_evidence_journal::{Journal, Storage};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
const MAGIC: &[u8; 8] = b"OLAYOUT1";
pub(crate) const BYTES: usize = 152;
#[derive(Clone)]
pub(crate) struct Receipt([u8; BYTES]);
pub(crate) struct Draft {
    receipt: Receipt,
    root: [u8; 32],
}
/// No Deserialize/Clone/caller constructor. This permits recognition of torn
/// first-format metadata, never erased data or reclamation on its own.
pub(crate) struct EstablishedLayout {
    region: [u8; 32],
}
impl EstablishedLayout {
    pub(super) fn matches(&self, storage: &impl WholeEpoch) -> bool {
        self.region == storage.identity()
    }
}
fn transcript(bytes: &[u8; BYTES], root: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"conduit.environment/observation-layout@1\0");
    h.update(root);
    h.update(&bytes[..88]);
    h.finalize().into()
}
impl Draft {
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn transcript(&self) -> [u8; 32] {
        transcript(&self.receipt.0, &self.root)
    }
    pub(crate) fn signed(mut self, signature: [u8; 64]) -> Receipt {
        self.receipt.0[88..].copy_from_slice(&signature);
        self.receipt
    }
}
impl Receipt {
    pub(crate) fn decode(bytes: [u8; BYTES]) -> Self {
        Self(bytes)
    }
    pub(crate) fn bytes(&self) -> &[u8; BYTES] {
        &self.0
    }
    /// Validate persisted data against Body/Root without minting physical
    /// custody. Region binding remains mandatory in verify below.
    pub(crate) fn validate(&self, body: &BodyId, root: &VerifyingKey) -> Result<(), Error> {
        if &self.0[..8] != MAGIC
            || self.0[8..12] != 1u32.to_le_bytes()
            || self.0[12..16] != (DATA as u32).to_le_bytes()
            || self.0[16..20] != (TOTAL as u32).to_le_bytes()
            || self.0[20..24] != [0; 4]
            || self.0[24..56] != Sha256::digest(body.as_str().as_bytes())[..]
        {
            return Err(Error::Region);
        }
        root.verify_strict(
            &transcript(&self.0, &root.to_bytes()),
            &Signature::from_bytes(self.0[88..].try_into().unwrap()),
        )
        .map_err(|_| Error::Authentication)?;
        Ok(())
    }
    pub(crate) fn verify(
        &self,
        body: &BodyId,
        root: &VerifyingKey,
        storage: &impl WholeEpoch,
    ) -> Result<EstablishedLayout, Error> {
        self.validate(body, root)?;
        if self.0[56..88] != storage.identity() {
            return Err(Error::Region);
        }
        Ok(EstablishedLayout {
            region: storage.identity(),
        })
    }
}
/// Consume the actual journal so its retained extent cannot be supplied by
/// incoming JSON or a caller integer. On refusal retain the affine epoch owner.
#[allow(clippy::result_large_err)]
pub(crate) fn preflight<S: Storage + WholeEpoch>(
    journal: Journal<'_, S>,
    body: &BodyId,
    root: &[u8; 32],
) -> Result<(S, Draft), (S, Error)> {
    let extent = journal.retained_extent() as usize;
    let mut storage = journal.into_storage();
    let result = (|| {
        let end = storage
            .data_offset()
            .checked_add(extent)
            .ok_or(Error::Region)?;
        if end > DATA {
            return Err(Error::Occupied);
        }
        let mut chunk = [0; 64];
        for at in (DATA..TOTAL).step_by(chunk.len()) {
            WholeEpoch::read(&mut storage, at, &mut chunk)?;
            if chunk.iter().any(|b| *b != 255) {
                return Err(Error::Occupied);
            }
        }
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&(DATA as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&(TOTAL as u32).to_le_bytes());
        bytes[24..56].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
        bytes[56..88].copy_from_slice(&storage.identity());
        Ok(Draft {
            receipt: Receipt(bytes),
            root: *root,
        })
    })();
    match result {
        Ok(draft) => Ok((storage, draft)),
        Err(error) => Err((storage, error)),
    }
}
#[cfg(test)]
#[path = "native_observed_layout_tests.rs"]
mod tests;
