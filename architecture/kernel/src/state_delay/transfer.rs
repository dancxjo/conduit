//! Allocation-free movement of committed State between fixed storage profiles.
//!
//! This is a numeric kernel mechanism, not migration permission. The caller must
//! admit exact semantic identities, value kinds and destination realization
//! before using it. It carries no Host operation, grant or Resource binding.

use super::StateDelay;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTransferError {
    CandidatePending,
    InvalidDestinationBounds,
    InsufficientDestinationCapacity,
}

/// A refused transfer returns ownership of the unchanged source cell.
pub struct RefusedStateTransfer<const BYTES: usize> {
    pub reason: StateTransferError,
    pub source: StateDelay<BYTES>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateTransferEvidence {
    pub source_slot: u16,
    pub destination_slot: u16,
    pub generation: u64,
    pub value_bytes: usize,
    pub source_capacity: usize,
    pub destination_capacity: usize,
}

impl<const BYTES: usize> StateDelay<BYTES> {
    /// Consume one committed cell into a separately admitted storage profile.
    ///
    /// Success moves ownership rather than cloning active State. Refusal returns
    /// the original cell, including any candidate. The existing continuation
    /// allowance and generation are preserved: transfer cannot renew a budget.
    /// Candidate handling must be explicit before transfer; no pending update is
    /// silently dropped or committed. This performs no semantic conversion.
    pub fn try_transfer<const DESTINATION_BYTES: usize>(
        self,
        destination_slot: u16,
        destination_capacity: usize,
    ) -> Result<(StateDelay<DESTINATION_BYTES>, StateTransferEvidence), RefusedStateTransfer<BYTES>>
    {
        let refusal = if self.candidate_len.is_some() {
            Some(StateTransferError::CandidatePending)
        } else if destination_capacity == 0 || destination_capacity > DESTINATION_BYTES {
            Some(StateTransferError::InvalidDestinationBounds)
        } else if self.current_len > destination_capacity {
            Some(StateTransferError::InsufficientDestinationCapacity)
        } else {
            None
        };
        if let Some(reason) = refusal {
            return Err(RefusedStateTransfer {
                reason,
                source: self,
            });
        }
        let mut current = [0; DESTINATION_BYTES];
        current[..self.current_len].copy_from_slice(self.current());
        let evidence = StateTransferEvidence {
            source_slot: self.state,
            destination_slot,
            generation: self.generation,
            value_bytes: self.current_len,
            source_capacity: self.maximum_bytes,
            destination_capacity,
        };
        Ok((
            StateDelay {
                state: destination_slot,
                maximum_bytes: destination_capacity,
                maximum_transitions: self.maximum_transitions,
                generation: self.generation,
                current_len: self.current_len,
                candidate_len: None,
                current,
                candidate: [0; DESTINATION_BYTES],
            },
            evidence,
        ))
    }
}

#[cfg(test)]
mod tests;
