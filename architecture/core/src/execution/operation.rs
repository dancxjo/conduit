//! Owner-held accounting for one already admitted correlated request.
//! The owner retains command storage and the existing envelope. This helper owns
//! that envelope and binds bytes by SHA-256 and length, avoiding self-borrows.
//! It grants no admission, scheduler, request identity or native enforcement.
use crate::ProtectedEntryBudget;
use sha2::{Digest, Sha256};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum OperationRefusal {
    InvalidPolicy,
    NotBegun,
    DuplicateBegin,
    WrongRequest,
    StaleSlice,
    SliceOutstanding,
    InvalidCount,
    InvalidResponse,
    ResponseNotReady,
    TotalExhausted,
    Terminal,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SliceReturn {
    Gate,
    Preempted,
}
/// Private epoch fields prevent callers from manufacturing a fresh grant.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct OperationSlice<E: Copy + Eq> {
    envelope: E,
    command_digest: [u8; 32],
    command_len: usize,
    epoch: u32,
    interrupts: u32,
}
impl<E: Copy + Eq> OperationSlice<E> {
    pub const fn interrupts(self) -> u32 {
        self.interrupts
    }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum State {
    New,
    Active,
    Completed,
    Revoked,
}
/// Exactly one helper belongs to the existing owner's admitted operation.
/// Constructing a replacement while that request is live is forbidden by the
/// owner contract, just as replacing its existing request envelope would be.
pub struct ProtectedOperation<E: Copy + Eq> {
    policy: ProtectedEntryBudget,
    envelope: Option<E>,
    command_digest: [u8; 32],
    command_len: usize,
    remaining: u32,
    epoch: u32,
    outstanding: Option<OperationSlice<E>>,
    state: State,
    response_ready: bool,
}
impl<E: Copy + Eq> ProtectedOperation<E> {
    pub fn new(policy: ProtectedEntryBudget) -> Result<Self, OperationRefusal> {
        if !policy.is_valid() {
            return Err(OperationRefusal::InvalidPolicy);
        }
        Ok(Self {
            remaining: policy.total_interrupts,
            policy,
            envelope: None,
            command_digest: [0; 32],
            command_len: 0,
            epoch: 0,
            outstanding: None,
            state: State::New,
            response_ready: false,
        })
    }
    /// Called once, after the existing owner admits the exact request bytes.
    pub fn begin(&mut self, envelope: E, command: &[u8]) -> Result<(), OperationRefusal> {
        match self.state {
            State::New => {}
            State::Active => return Err(OperationRefusal::DuplicateBegin),
            _ => return Err(OperationRefusal::Terminal),
        }
        self.command_digest = Sha256::digest(command).into();
        self.command_len = command.len();
        self.envelope = Some(envelope);
        self.state = State::Active;
        Ok(())
    }
    fn check(&self, envelope: &E, command: &[u8]) -> Result<(), OperationRefusal> {
        match self.state {
            State::New => return Err(OperationRefusal::NotBegun),
            State::Active => {}
            _ => return Err(OperationRefusal::Terminal),
        }
        if self.envelope.as_ref() != Some(envelope)
            || command.len() != self.command_len
            || <[u8; 32]>::from(Sha256::digest(command)) != self.command_digest
        {
            return Err(OperationRefusal::WrongRequest);
        }
        Ok(())
    }
    pub fn grant(
        &mut self,
        envelope: &E,
        command: &[u8],
    ) -> Result<OperationSlice<E>, OperationRefusal> {
        self.check(envelope, command)?;
        if self.outstanding.is_some() {
            return Err(OperationRefusal::SliceOutstanding);
        }
        let Some(epoch) = self.epoch.checked_add(1) else {
            self.revoke();
            return Err(OperationRefusal::TotalExhausted);
        };
        let grant = OperationSlice {
            envelope: *envelope,
            command_digest: self.command_digest,
            command_len: self.command_len,
            epoch,
            interrupts: self.policy.quantum_interrupts.min(self.remaining),
        };
        self.response_ready = false;
        self.epoch = epoch;
        self.outstanding = Some(grant);
        Ok(grant)
    }
    /// Count only actual observed accounting interrupts. Gate and preemption
    /// both retain the remaining count; neither retires or renews the request.
    pub fn finish_slice(
        &mut self,
        envelope: &E,
        command: &[u8],
        grant: OperationSlice<E>,
        consumed: u32,
        returned: SliceReturn,
    ) -> Result<(), OperationRefusal> {
        self.check(envelope, command)?;
        if grant.envelope != *envelope
            || grant.command_digest != self.command_digest
            || grant.command_len != self.command_len
        {
            return Err(OperationRefusal::WrongRequest);
        }
        if self.outstanding != Some(grant) {
            return Err(OperationRefusal::StaleSlice);
        }
        if consumed > grant.interrupts || consumed > self.remaining {
            self.revoke();
            return Err(OperationRefusal::InvalidCount);
        }
        self.outstanding = None;
        self.remaining -= consumed;
        self.response_ready = returned == SliceReturn::Gate;
        if self.remaining == 0 {
            self.revoke();
            return Err(OperationRefusal::TotalExhausted);
        }
        Ok(())
    }
    /// Only the existing owner's complete bounded response/envelope validator
    /// can retire the request. The callback must not interpret Gate as success.
    pub fn complete(
        &mut self,
        envelope: &E,
        command: &[u8],
        validate_full_response: impl FnOnce(&E, &[u8]) -> bool,
    ) -> Result<(), OperationRefusal> {
        self.check(envelope, command)?;
        if self.outstanding.is_some() {
            return Err(OperationRefusal::SliceOutstanding);
        }
        if !self.response_ready {
            return Err(OperationRefusal::ResponseNotReady);
        }
        if !validate_full_response(envelope, command) {
            self.revoke();
            return Err(OperationRefusal::InvalidResponse);
        }
        self.state = State::Completed;
        Ok(())
    }
    /// Fault/cancellation is terminal. A foreign cancellation cannot revoke the
    /// healthy current operation, because identity/bytes are checked first.
    pub fn cancel_or_fault(
        &mut self,
        envelope: &E,
        command: &[u8],
    ) -> Result<(), OperationRefusal> {
        self.check(envelope, command)?;
        self.revoke();
        Ok(())
    }
    fn revoke(&mut self) {
        self.outstanding = None;
        self.response_ready = false;
        self.remaining = 0;
        self.state = State::Revoked;
    }
    pub const fn remaining(&self) -> u32 {
        self.remaining
    }
    pub const fn is_terminal(&self) -> bool {
        matches!(self.state, State::Completed | State::Revoked)
    }
}

#[cfg(test)]
#[path = "operation_tests.rs"]
mod tests;
