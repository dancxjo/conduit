//! Canonical wire representation of Core's signed member-return proof.
//! Callers must bound received bytes before deserializing. Decoding checks shape,
//! never the challenge signature, current membership or execution authority.
use crate::{AdmissionId, BodyId, PartId, PartReturnProof};
use alloc::vec::Vec;
use conduit_core::{BootId, HostId};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberReturnProofDocumentRefusal {
    IdentifierBounds,
    SignatureExtent,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberReturnProofDocument {
    admission_id: AdmissionId,
    body_id: BodyId,
    part_id: PartId,
    host_id: HostId,
    boot_id: BootId,
    nonce: [u8; 32],
    signature: Vec<u8>,
}
impl MemberReturnProofDocument {
    pub fn encode(proof: PartReturnProof) -> Self {
        Self {
            admission_id: proof.admission_id,
            body_id: proof.body_id,
            part_id: proof.part_id,
            host_id: proof.host_id,
            boot_id: proof.boot_id,
            nonce: proof.nonce,
            signature: proof.signature.to_vec(),
        }
    }
    pub fn decode(self) -> Result<PartReturnProof, MemberReturnProofDocumentRefusal> {
        if [
            self.admission_id.as_str(),
            self.body_id.as_str(),
            self.part_id.as_str(),
            self.host_id.as_str(),
            self.boot_id.as_str(),
        ]
        .into_iter()
        .any(|id| id.is_empty() || id.len() > crate::MAX_LIFECYCLE_ID_BYTES)
        {
            return Err(MemberReturnProofDocumentRefusal::IdentifierBounds);
        }
        Ok(PartReturnProof {
            admission_id: self.admission_id,
            body_id: self.body_id,
            part_id: self.part_id,
            host_id: self.host_id,
            boot_id: self.boot_id,
            nonce: self.nonce,
            signature: self
                .signature
                .try_into()
                .map_err(|_| MemberReturnProofDocumentRefusal::SignatureExtent)?,
        })
    }
}

#[cfg(test)]
#[path = "member_return_proof_document_tests.rs"]
mod tests;
