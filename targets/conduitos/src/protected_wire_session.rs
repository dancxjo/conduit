//! Canonical ordinary host session frames over one protected ConduitOS line.
//!
//! This adapter owns fixed wire storage and the shared `SessionMachine`. It
//! receives current provider truth from the host supervisor before every
//! operation; replacement terminally loses this Line and never reconnects.

use conduit_protected_line::ProtectedLineError;
use conduit_wire::{
    SessionBinding, SessionMachine, SessionMessage, SessionRole, WireError, decode_session_frame,
    encode_session_frame_into,
};

use crate::{outbound_network::NetworkProvider, protected_line_support::ConduitOsProtectedLineIo};

pub const MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES: usize = 4096;

pub trait NetworkProviderTruth {
    fn current_provider(&self) -> NetworkProvider;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitOsWireSessionRefusal {
    InvalidBounds,
    ProviderLost,
    LineLost,
    Protected(ProtectedLineError),
    Wire(WireError),
}

/// One allocation-stable ordinary session inside an already-protected Line.
pub struct ConduitOsWireSession<'a, P> {
    protected: &'a mut dyn ConduitOsProtectedLineIo,
    provider_truth: &'a P,
    admitted_provider: NetworkProvider,
    machine: SessionMachine,
    maximum_frame_bytes: u32,
    frame: [u8; MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES],
    lost: bool,
}

impl<'a, P: NetworkProviderTruth> ConduitOsWireSession<'a, P> {
    pub fn new(
        protected: &'a mut dyn ConduitOsProtectedLineIo,
        provider_truth: &'a P,
        binding: SessionBinding,
        role: SessionRole,
    ) -> Result<Self, ConduitOsWireSessionRefusal> {
        let maximum_frame_bytes = binding.attachment.limits.maximum_frame_bytes;
        if maximum_frame_bytes == 0
            || maximum_frame_bytes as usize > MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES
        {
            return Err(ConduitOsWireSessionRefusal::InvalidBounds);
        }
        let admitted_provider = provider_truth.current_provider();
        if admitted_provider.provider_generation == 0 {
            return Err(ConduitOsWireSessionRefusal::ProviderLost);
        }
        let machine =
            SessionMachine::new(binding, role).map_err(ConduitOsWireSessionRefusal::Wire)?;
        Ok(Self {
            protected,
            provider_truth,
            admitted_provider,
            machine,
            maximum_frame_bytes,
            frame: [0; MAXIMUM_CONDUITOS_WIRE_FRAME_BYTES],
            lost: false,
        })
    }

    pub fn machine(&self) -> &SessionMachine {
        &self.machine
    }

    pub fn send(&mut self, message: SessionMessage<'_>) -> Result<(), ConduitOsWireSessionRefusal> {
        self.check_provider()?;
        let mut next = self.machine.clone();
        let binding = next.binding().clone();
        next.admit_outbound(binding.frame(message))
            .map_err(ConduitOsWireSessionRefusal::Wire)?;
        let length = encode_session_frame_into(
            binding.frame(message),
            &mut self.frame,
            binding.limits.maximum_payload_bytes,
            self.maximum_frame_bytes,
        )
        .map_err(ConduitOsWireSessionRefusal::Wire)?;
        if let Err(error) = self.protected.send(&self.frame[..length]) {
            self.frame[..length].fill(0);
            return Err(self.lose_for(error));
        }
        self.frame[..length].fill(0);
        self.machine = next;
        Ok(())
    }

    pub fn receive<R>(
        &mut self,
        inspect: impl FnOnce(SessionMessage<'_>) -> R,
    ) -> Result<R, ConduitOsWireSessionRefusal> {
        self.check_provider()?;
        let received = match self.protected.receive() {
            Ok(received) => received,
            Err(error) => return Err(self.lose_for(error)),
        };
        if received.len() > self.maximum_frame_bytes as usize {
            return Err(ConduitOsWireSessionRefusal::Wire(WireError::OversizedFrame));
        }
        self.frame[..received.len()].copy_from_slice(received);
        let decoded = decode_session_frame(
            &self.frame[..received.len()],
            self.machine.binding().limits.maximum_payload_bytes,
            self.maximum_frame_bytes,
        )
        .map_err(ConduitOsWireSessionRefusal::Wire)?;
        let mut next = self.machine.clone();
        next.admit_inbound(decoded)
            .map_err(ConduitOsWireSessionRefusal::Wire)?;
        let result = inspect(decoded.message);
        self.frame[..received.len()].fill(0);
        self.machine = next;
        Ok(result)
    }

    fn check_provider(&mut self) -> Result<(), ConduitOsWireSessionRefusal> {
        if self.lost {
            return Err(ConduitOsWireSessionRefusal::LineLost);
        }
        if self.provider_truth.current_provider() != self.admitted_provider {
            self.lost = true;
            return Err(ConduitOsWireSessionRefusal::ProviderLost);
        }
        Ok(())
    }

    fn lose_for(&mut self, error: ProtectedLineError) -> ConduitOsWireSessionRefusal {
        if error == ProtectedLineError::OuterCarrierLost {
            self.lost = true;
            ConduitOsWireSessionRefusal::LineLost
        } else {
            ConduitOsWireSessionRefusal::Protected(error)
        }
    }
}

#[cfg(test)]
mod tests;
