//! Device-to-owner evidence below the ordinary Body lifecycle. A receipt binds
//! the exact signed command; it neither creates membership nor starts a Body.
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum State {
    StartedHeld = 0,
    Released = 1,
    Quiesced = 2,
    Observed = 3,
    Retired = 4,
    EvidenceAccepted = 5,
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub sequence: u64,
    pub committed_millis: u64,
    pub observed_millis: u64,
    pub channel: u8,
    pub bytes: u16,
    pub sha256: [u8; 32],
}

impl Snapshot {
    /// Conservative age at the owning Host: caller adds its measured request
    /// round trip, never substitutes a wall clock for the signed Boot clock.
    pub fn fresh_within(&self, maximum_age_ms: u64, round_trip_ms: u64) -> bool {
        self.observed_millis
            .checked_sub(self.committed_millis)
            .and_then(|age| age.checked_add(round_trip_ms))
            .is_some_and(|age| age <= maximum_age_ms)
    }
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub command: [u8; 32],
    pub state: State,
    pub snapshot: Option<Snapshot>,
}
impl Receipt {
    pub fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/root-owner-acknowledgement@2");
        hash.update(self.command);
        hash.update([self.state as u8]);
        match self.snapshot {
            Some(snapshot) => {
                hash.update([1]);
                hash.update(snapshot.sequence.to_le_bytes());
                hash.update(snapshot.committed_millis.to_le_bytes());
                hash.update(snapshot.observed_millis.to_le_bytes());
                hash.update([snapshot.channel]);
                hash.update(snapshot.bytes.to_le_bytes());
                hash.update(snapshot.sha256);
            }
            None => hash.update([0]),
        }
        hash.finalize().into()
    }
    /// `key` comes from this owner's already verified ambient admission for the
    /// current Boot, never from the reply or an unauthenticated advertised key.
    pub fn verify(
        &self,
        key: &[u8; 32],
        expected_command: &[u8; 32],
        expected_state: State,
        signature: &[u8; 64],
    ) -> Result<VerifiedReceipt, Refusal> {
        if self.command == [0; 32]
            || self.command != *expected_command
            || self.state != expected_state
        {
            return Err(Refusal::Scope);
        }
        if (self.state == State::Observed) != self.snapshot.is_some()
            || self.snapshot.is_some_and(|s| {
                s.committed_millis > s.observed_millis
                    || s.sequence == 0
                    || s.channel >= 4
                    || s.bytes == 0
                    || s.bytes > 4096
                    || s.sha256 == [0; 32]
            })
        {
            return Err(Refusal::Scope);
        }
        let key = VerifyingKey::from_bytes(key).map_err(|_| Refusal::Key)?;
        if key.is_weak() {
            return Err(Refusal::Key);
        }
        key.verify_strict(&self.transcript(), &Signature::from_bytes(signature))
            .map_err(|_| Refusal::Signature)?;
        Ok(VerifiedReceipt { state: self.state })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Scope,
    Key,
    Signature,
}
/// Only verification constructs this non-Clone, non-deserializable witness.
/// The caller must also retain its current pending command and ordinary owner;
/// accepting historical evidence cannot reopen a lifecycle or confer authority.
pub struct VerifiedReceipt {
    state: State,
}
impl VerifiedReceipt {
    pub fn into_state(self) -> State {
        self.state
    }
}
