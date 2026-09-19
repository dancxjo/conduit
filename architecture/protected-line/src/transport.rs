#[cfg(feature = "noise-session")]
use alloc::vec;
#[cfg(feature = "noise-session")]
use alloc::vec::Vec;

#[cfg(feature = "noise-session")]
use crate::{ProtectedHandshake, ProtectedSession, Role, SessionBinding};
use crate::{ProtectedLineError, SessionLimits};

/// Finite work and retention limits shared by every target adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectedSessionPolicy {
    pub traffic: SessionLimits,
    pub maximum_simultaneous_sessions: u16,
    pub maximum_pending_frames_per_session: u8,
    pub handshake_work_units: u8,
    pub handshake_timeout_millis: u32,
    pub idle_timeout_millis: u32,
}

impl ProtectedSessionPolicy {
    pub fn validate(self) -> Result<Self, ProtectedLineError> {
        self.traffic.validate()?;
        if self.maximum_simultaneous_sessions == 0
            || self.maximum_simultaneous_sessions > 64
            // The profile is strictly ordered and admits exactly one pending frame.
            || self.maximum_pending_frames_per_session != 1
            // NNpsk0 is exactly one write and one read at each endpoint.
            || self.handshake_work_units != 2
            || self.handshake_timeout_millis == 0
            || self.handshake_timeout_millis > 300_000
            || self.idle_timeout_millis == 0
            || self.idle_timeout_millis > 3_600_000
        {
            return Err(ProtectedLineError::InvalidPolicy);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierFailure {
    Pressure,
    TimedOut,
    Cancelled,
    Lost,
}

impl CarrierFailure {
    #[cfg(feature = "noise-session")]
    fn protected(self, handshake: bool) -> ProtectedLineError {
        match self {
            Self::Pressure => ProtectedLineError::Pressure,
            Self::TimedOut if handshake => ProtectedLineError::HandshakeTimedOut,
            Self::TimedOut => ProtectedLineError::SessionTimedOut,
            Self::Cancelled => ProtectedLineError::Cancelled,
            Self::Lost => ProtectedLineError::OuterCarrierLost,
        }
    }
}

/// One-message-at-a-time carrier boundary. Implementations install the supplied
/// timeout on each receive and never retain the caller's buffers.
pub trait ProtectedFrameCarrier {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure>;
    fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout_millis: u32,
    ) -> Result<usize, CarrierFailure>;
    fn close(&mut self) -> Result<(), CarrierFailure>;
}

/// Fixed-capacity admission state. Slots are finite and must be retired exactly
/// once by the target adapter on close, cancellation, or failure.
pub struct ProtectedSessionAdmission<const SESSIONS: usize> {
    policy: ProtectedSessionPolicy,
    occupied: [bool; SESSIONS],
}

impl<const SESSIONS: usize> ProtectedSessionAdmission<SESSIONS> {
    pub fn new(policy: ProtectedSessionPolicy) -> Result<Self, ProtectedLineError> {
        let policy = policy.validate()?;
        if usize::from(policy.maximum_simultaneous_sessions) > SESSIONS {
            return Err(ProtectedLineError::InvalidPolicy);
        }
        Ok(Self {
            policy,
            occupied: [false; SESSIONS],
        })
    }

    pub fn policy(&self) -> ProtectedSessionPolicy {
        self.policy
    }

    pub fn admit(&mut self) -> Result<usize, ProtectedLineError> {
        let admitted = usize::from(self.policy.maximum_simultaneous_sessions);
        let slot = self.occupied[..admitted]
            .iter()
            .position(|occupied| !occupied)
            .ok_or(ProtectedLineError::SessionCapacity)?;
        self.occupied[slot] = true;
        Ok(slot)
    }

    pub fn retire(&mut self, slot: usize) -> Result<(), ProtectedLineError> {
        let Some(occupied) = self.occupied.get_mut(slot) else {
            return Err(ProtectedLineError::Closed);
        };
        if !*occupied {
            return Err(ProtectedLineError::Closed);
        }
        *occupied = false;
        Ok(())
    }
}

/// Establish the exact profile over any bounded frame carrier. The caller owns
/// fresh entropy and the rendezvous PSK; neither is retained after the Noise
/// handshake derives the two transport keys.
#[cfg(feature = "noise-session")]
pub fn establish_protected_session<C: ProtectedFrameCarrier>(
    carrier: &mut C,
    role: Role,
    binding: &SessionBinding,
    policy: ProtectedSessionPolicy,
    preshared_key: [u8; 32],
    ephemeral_private_key: [u8; 32],
) -> Result<ProtectedSession, ProtectedLineError> {
    let policy = policy.validate()?;
    let mut handshake = ProtectedHandshake::new(
        role,
        binding,
        policy.traffic,
        preshared_key,
        ephemeral_private_key,
    )?;
    let maximum_handshake_bytes = 64;
    let mut frame = [0_u8; 64];
    for _ in 0..policy.handshake_work_units {
        if handshake.next_message_bytes()? > maximum_handshake_bytes {
            return Err(ProtectedLineError::HandshakeFrameLength);
        }
        if handshake.is_write_turn() {
            let length = handshake.next_message_bytes()?;
            handshake.write_message(&mut frame[..length])?;
            carrier
                .send_frame(&frame[..length])
                .map_err(|failure| failure.protected(true))?;
        } else {
            let length = carrier
                .receive_frame(&mut frame, policy.handshake_timeout_millis)
                .map_err(|failure| failure.protected(true))?;
            if length > frame.len() {
                return Err(ProtectedLineError::Pressure);
            }
            handshake.read_message(&frame[..length])?;
        }
    }
    handshake.finish()
}

/// An established session with one reusable send and receive buffer. Storage is
/// allocated once from the admitted maximum and never grows with lifetime traffic.
#[cfg(feature = "noise-session")]
pub struct ProtectedCarrier<C> {
    carrier: C,
    session: ProtectedSession,
    send: Vec<u8>,
    receive: Vec<u8>,
    plaintext: Vec<u8>,
    timeout_millis: u32,
}

#[cfg(feature = "noise-session")]
impl<C: ProtectedFrameCarrier> ProtectedCarrier<C> {
    pub fn new(
        carrier: C,
        session: ProtectedSession,
        policy: ProtectedSessionPolicy,
    ) -> Result<Self, ProtectedLineError> {
        let policy = policy.validate()?;
        if session.limits() != policy.traffic {
            return Err(ProtectedLineError::InvalidPolicy);
        }
        let frame_bytes = session.maximum_frame_bytes();
        Ok(Self {
            carrier,
            session,
            send: vec![0; frame_bytes],
            receive: vec![0; frame_bytes],
            plaintext: vec![0; policy.traffic.maximum_payload_bytes as usize],
            timeout_millis: policy.idle_timeout_millis,
        })
    }

    pub fn send(&mut self, plaintext: &[u8]) -> Result<(), ProtectedLineError> {
        let length = self.session.seal(plaintext, &mut self.send)?;
        if let Err(failure) = self.carrier.send_frame(&self.send[..length]) {
            let error = failure.protected(false);
            self.retire_carrier_failure(failure, error);
            return Err(error);
        }
        Ok(())
    }

    pub fn receive(&mut self) -> Result<&[u8], ProtectedLineError> {
        let length = match self
            .carrier
            .receive_frame(&mut self.receive, self.timeout_millis)
        {
            Ok(length) => length,
            Err(failure) => {
                let error = failure.protected(false);
                self.retire_carrier_failure(failure, error);
                return Err(error);
            }
        };
        if length > self.receive.len() {
            self.session.fail(ProtectedLineError::Pressure);
            return Err(ProtectedLineError::Pressure);
        }
        let plaintext = self
            .session
            .open(&self.receive[..length], &mut self.plaintext)?;
        Ok(&self.plaintext[..plaintext])
    }

    pub fn close(&mut self) -> Result<(), ProtectedLineError> {
        self.session.close();
        self.carrier
            .close()
            .map_err(|failure| failure.protected(false))?;
        Ok(())
    }

    pub fn session(&self) -> &ProtectedSession {
        &self.session
    }

    fn retire_carrier_failure(&mut self, failure: CarrierFailure, error: ProtectedLineError) {
        if failure == CarrierFailure::Cancelled {
            self.session.cancel();
        } else {
            self.session.fail(error);
        }
    }
}
