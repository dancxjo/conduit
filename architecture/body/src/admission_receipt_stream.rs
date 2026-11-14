//! Exact receipt authentication without retaining the complete receipt bytes.
use crate::{
    AdmissionReceiptAttestationRefusal as R, FederationPeer, MAX_ATTESTED_ADMISSION_RECEIPT_BYTES,
    MAX_LIFECYCLE_ID_BYTES, SIGNED_ADMISSION_RECEIPT_SCHEMA,
};
use sha2::{Digest, Sha256};

/// The complete byte extent and caller-local chunk ceiling, not a resource grant.
/// The existing attestation profile's 16 KiB maximum remains in force.
pub struct AdmissionReceiptStreamBounds {
    pub receipt_bytes: usize,
    pub maximum_chunks: usize,
}

/// Incrementally computes the existing transcript, including its exact length
/// prefix. This neither parses receipt meaning nor authenticates the result.
pub struct AdmissionReceiptTranscript {
    digest: Sha256,
    expected: usize,
    received: usize,
    chunks: usize,
    maximum_chunks: usize,
    owner_generation: u64,
    owner_key: [u8; 32],
    member_key: [u8; 32],
}

impl AdmissionReceiptTranscript {
    pub fn new(
        bounds: AdmissionReceiptStreamBounds,
        owner_host: &str,
        owner_boot: &str,
        owner_generation: u64,
        owner_key: &[u8; 32],
        member_key: &[u8; 32],
    ) -> Result<Self, R> {
        if bounds.receipt_bytes == 0
            || bounds.receipt_bytes > MAX_ATTESTED_ADMISSION_RECEIPT_BYTES
            || bounds.maximum_chunks == 0
            || bounds.maximum_chunks > MAX_ATTESTED_ADMISSION_RECEIPT_BYTES
        {
            return Err(R::Bounds);
        }
        if owner_host.is_empty()
            || owner_boot.is_empty()
            || owner_host.len() > MAX_LIFECYCLE_ID_BYTES
            || owner_boot.len() > MAX_LIFECYCLE_ID_BYTES
            || *owner_key == [0; 32]
            || *member_key == [0; 32]
        {
            return Err(R::ReceiptBasis);
        }
        let mut digest = Sha256::new();
        for bytes in [
            SIGNED_ADMISSION_RECEIPT_SCHEMA.as_bytes(),
            owner_host.as_bytes(),
            owner_boot.as_bytes(),
        ] {
            digest.update((bytes.len() as u32).to_le_bytes());
            digest.update(bytes);
        }
        digest.update((bounds.receipt_bytes as u32).to_le_bytes());
        Ok(Self {
            digest,
            expected: bounds.receipt_bytes,
            received: 0,
            chunks: 0,
            maximum_chunks: bounds.maximum_chunks,
            owner_generation,
            owner_key: *owner_key,
            member_key: *member_key,
        })
    }

    /// Rejected input changes neither the digest nor the accepted prefix.
    /// Chunk order is the caller's message order; the signature fences changes
    /// to that order. No receipt meaning may be accepted before verification.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), R> {
        if bytes.is_empty()
            || self.chunks == self.maximum_chunks
            || bytes.len() > self.expected - self.received
        {
            return Err(R::Bounds);
        }
        self.digest.update(bytes);
        self.received += bytes.len();
        self.chunks += 1;
        Ok(())
    }

    pub fn finish(mut self) -> Result<[u8; 32], R> {
        if self.received != self.expected {
            return Err(R::Bounds);
        }
        self.digest.update(self.owner_generation.to_le_bytes());
        self.digest.update(self.owner_key);
        self.digest.update(self.member_key);
        Ok(self.digest.finalize().into())
    }
}

/// Strict verification against an independently retained peer pin. Success
/// authenticates the ordered bytes only: callers still owe exact Body/Part,
/// credential, Boot and offer checks, persistence and any effect authorization.
pub struct AdmissionReceiptVerifier<'a> {
    transcript: AdmissionReceiptTranscript,
    owner: &'a FederationPeer,
    signature: &'a [u8; 64],
}
impl<'a> AdmissionReceiptVerifier<'a> {
    pub(crate) fn new(
        transcript: AdmissionReceiptTranscript,
        owner: &'a FederationPeer,
        signature: &'a [u8; 64],
    ) -> Self {
        Self {
            transcript,
            owner,
            signature,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<(), R> {
        self.transcript.push(bytes)
    }

    pub fn finish(self) -> Result<(), R> {
        crate::admission_receipt_attestation::verify_receipt_transcript(
            &self.transcript.finish()?,
            self.owner,
            self.signature,
        )
    }
}
