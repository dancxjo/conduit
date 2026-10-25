//! One admitted evidence page; custody is retained until exact owner acceptance.
//! Shared by the MCU and deterministic host proofs. No scheduler or retry loop.
use crate::native_owner_control::{EvidenceReference, Refusal};
use alloc::boxed::Box;
use conduit_environment_runtime::display_evidence as wire;
use conduit_kernel as k;
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
pub type Log = k::FixedSignLog<{ wire::MAXIMUM_EVENTS }>;
pub fn fresh_log() -> Result<Log, k::SignError> {
    Log::new_with_remote_storage(
        (wire::MAXIMUM_EVENTS * core::mem::size_of::<k::KernelEvent>()) as u32,
        6,
        k::remote_sign_storage_bytes(6).ok_or(k::SignError::InvalidBudget)?,
    )
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub command: [u8; 32],
    pub page: EvidenceReference,
    pub bytes: u16,
}
impl Offer {
    pub fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/display-evidence-offer@1");
        hash.update(self.command);
        hash.update(self.page.invocation.to_le_bytes());
        hash.update(self.page.sha256);
        hash.update(self.bytes.to_le_bytes());
        hash.finalize().into()
    }
    pub fn verify(
        &self,
        key: &[u8; 32],
        command: &[u8; 32],
        signature: &[u8; 64],
        bytes: &[u8],
    ) -> Result<(), Refusal> {
        if self.command != *command
            || *command == [0; 32]
            || self.bytes as usize != bytes.len()
            || bytes.len() > wire::MAXIMUM_BYTES
            || self.page.sha256 != <[u8; 32]>::from(Sha256::digest(bytes))
            || wire::Page::parse(bytes)
                .map_err(|_| Refusal::Malformed)?
                .invocation()
                != self.page.invocation
        {
            return Err(Refusal::Malformed);
        }
        let key = VerifyingKey::from_bytes(key).map_err(|_| Refusal::InvalidKey)?;
        if key.is_weak() {
            return Err(Refusal::InvalidKey);
        }
        key.verify_strict(&self.transcript(), &Signature::from_bytes(signature))
            .map_err(|_| Refusal::Signature)
    }
}
pub struct Custody {
    bytes: Box<[u8; wire::MAXIMUM_BYTES]>,
    length: usize,
    pending: Option<EvidenceReference>,
    accepted: bool,
    archived: Option<EvidenceReference>,
}
impl Custody {
    /// Allocate only during independently admitted task preparation.
    pub fn prepare() -> Self {
        Self {
            bytes: Box::new([0; wire::MAXIMUM_BYTES]),
            length: 0,
            pending: None,
            accepted: false,
            archived: None,
        }
    }
    pub fn pending(&self) -> Option<EvidenceReference> {
        self.pending
    }
    pub fn publish(&mut self, invocation: u64, log: &Log) -> Result<bool, &'static str> {
        if self.pending.is_some() {
            return Err("display-evidence-pressure");
        }
        let length = wire::encode(invocation, log.events(), log, &mut self.bytes[..])
            .map_err(|_| "display-evidence-encoding")?;
        let page = EvidenceReference {
            invocation,
            sha256: Sha256::digest(&self.bytes[..length]).into(),
        };
        if self.archived == Some(page) {
            self.length = 0;
            return Ok(false);
        }
        self.length = length;
        self.pending = Some(page);
        self.accepted = false;
        Ok(true)
    }
    pub fn copy(
        &self,
        output: &mut [u8],
    ) -> Result<Option<(EvidenceReference, usize)>, &'static str> {
        let Some(page) = self.pending else {
            return Ok(None);
        };
        if output.len() < self.length {
            return Err("display-evidence-capacity");
        }
        output[..self.length].copy_from_slice(&self.bytes[..self.length]);
        Ok(Some((page, self.length)))
    }
    pub fn accept(&mut self, page: EvidenceReference) -> Result<(), &'static str> {
        if self.pending != Some(page) || self.accepted {
            return Err("display-evidence-acknowledgement");
        }
        self.accepted = true;
        Ok(())
    }
    /// Only exact acceptance releases the retained page. No failed signature,
    /// timeout, inspection, cancellation or future page can clear its custody.
    pub fn release_accepted(&mut self) -> Result<EvidenceReference, &'static str> {
        if !self.accepted {
            return Err("display-evidence-unaccepted");
        }
        let page = self.pending.take().ok_or("display-evidence-unavailable")?;
        self.archived = Some(page);
        self.length = 0;
        self.accepted = false;
        Ok(page)
    }
}
