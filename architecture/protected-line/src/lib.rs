#![no_std]

extern crate alloc;

mod relay;
pub use relay::*;
#[cfg(feature = "noise-session")]
mod supplied_x25519;
mod transport;
pub use transport::*;

use alloc::string::String;
#[cfg(feature = "noise-session")]
use alloc::vec::Vec;
#[cfg(feature = "noise-session")]
use noise_protocol::{
    patterns::noise_nn_psk0, CipherState, HandshakeState, HandshakeStateBuilder, U8Array,
};
#[cfg(feature = "noise-session")]
use noise_rust_crypto::{sensitive::Sensitive, ChaCha20Poly1305, Sha256};
#[cfg(feature = "noise-session")]
use supplied_x25519::SuppliedX25519;

pub const IMPLEMENTATION_ID: &str = "conduit.line/noise-nnpsk0-25519-chachapoly-sha256@1";
pub const PROTOCOL_NAME: &str = "Noise_NNpsk0_25519_ChaChaPoly_SHA256";
pub const MAXIMUM_BINDING_BYTES: usize = 2_048;
#[cfg(feature = "noise-session")]
const HEADER_BYTES: usize = 4 + 1 + 1 + 8 + 4;
#[cfg(feature = "noise-session")]
const TAG_BYTES: usize = 16;
#[cfg(feature = "noise-session")]
const MAGIC: [u8; 4] = *b"CNDP";
#[cfg(feature = "noise-session")]
const VERSION: u8 = 1;

#[cfg(feature = "noise-session")]
type NoiseHandshake = HandshakeState<SuppliedX25519, ChaCha20Poly1305, Sha256>;
#[cfg(feature = "noise-session")]
type NoiseCipher = CipherState<ChaCha20Poly1305>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Initiator,
    Responder,
}

impl Role {
    #[cfg(feature = "noise-session")]
    fn outbound(self) -> u8 {
        match self {
            Self::Initiator => 0,
            Self::Responder => 1,
        }
    }

    #[cfg(feature = "noise-session")]
    fn inbound(self) -> u8 {
        self.outbound() ^ 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointBinding {
    pub host_id: String,
    pub boot_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBinding {
    pub initiator: EndpointBinding,
    pub responder: EndpointBinding,
    pub negotiation_id: String,
    pub line_session_id: String,
    pub candidate_binding: String,
    pub transport_binding: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLimits {
    pub maximum_payload_bytes: u32,
    pub maximum_frames_per_direction: u64,
    pub maximum_bytes_per_direction: u64,
}

impl SessionLimits {
    pub fn validate(self) -> Result<Self, ProtectedLineError> {
        if self.maximum_payload_bytes == 0
            || self.maximum_payload_bytes > 65_519
            || self.maximum_frames_per_direction == 0
            || self.maximum_frames_per_direction > 1_000_000
            || self.maximum_bytes_per_direction < self.maximum_payload_bytes as u64
            || self.maximum_bytes_per_direction > 1_073_741_824
        {
            return Err(ProtectedLineError::InvalidLimits);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingMismatch {
    InitiatorHost,
    InitiatorBoot,
    ResponderHost,
    ResponderBoot,
    Negotiation,
    LineSession,
    Candidate,
    Transport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedLineError {
    EmptyEphemeralKey,
    EmptyPresharedKey,
    InvalidBinding,
    BindingTooLarge,
    BindingMismatch(BindingMismatch),
    InvalidLimits,
    WrongHandshakeTurn,
    HandshakeFrameLength,
    AuthenticationFailed,
    HandshakeIncomplete,
    FrameTooLarge,
    OutputTooSmall,
    MalformedFrame,
    TruncatedFrame,
    WrongDirection,
    Replay,
    Reordered,
    FrameLimitExhausted,
    ByteLimitExhausted,
    InvalidPolicy,
    SessionCapacity,
    Pressure,
    HandshakeTimedOut,
    SessionTimedOut,
    OuterCarrierLost,
    Cancelled,
    Closed,
}

impl SessionBinding {
    pub fn compare(&self, candidate: &Self) -> Result<(), ProtectedLineError> {
        let mismatch = if self.initiator.host_id != candidate.initiator.host_id {
            BindingMismatch::InitiatorHost
        } else if self.initiator.boot_id != candidate.initiator.boot_id {
            BindingMismatch::InitiatorBoot
        } else if self.responder.host_id != candidate.responder.host_id {
            BindingMismatch::ResponderHost
        } else if self.responder.boot_id != candidate.responder.boot_id {
            BindingMismatch::ResponderBoot
        } else if self.negotiation_id != candidate.negotiation_id {
            BindingMismatch::Negotiation
        } else if self.line_session_id != candidate.line_session_id {
            BindingMismatch::LineSession
        } else if self.candidate_binding != candidate.candidate_binding {
            BindingMismatch::Candidate
        } else if self.transport_binding != candidate.transport_binding {
            BindingMismatch::Transport
        } else {
            return Ok(());
        };
        Err(ProtectedLineError::BindingMismatch(mismatch))
    }

    #[cfg(feature = "noise-session")]
    fn prologue(&self, limits: SessionLimits) -> Result<Vec<u8>, ProtectedLineError> {
        let mut bytes = Vec::with_capacity(MAXIMUM_BINDING_BYTES);
        push(&mut bytes, IMPLEMENTATION_ID)?;
        for value in [
            self.initiator.host_id.as_str(),
            self.initiator.boot_id.as_str(),
            self.responder.host_id.as_str(),
            self.responder.boot_id.as_str(),
            self.negotiation_id.as_str(),
            self.line_session_id.as_str(),
            self.candidate_binding.as_str(),
            self.transport_binding.as_str(),
        ] {
            push(&mut bytes, value)?;
        }
        if bytes
            .len()
            .checked_add(4 + 8 + 8)
            .is_none_or(|length| length > MAXIMUM_BINDING_BYTES)
        {
            return Err(ProtectedLineError::BindingTooLarge);
        }
        bytes.extend_from_slice(&limits.maximum_payload_bytes.to_le_bytes());
        bytes.extend_from_slice(&limits.maximum_frames_per_direction.to_le_bytes());
        bytes.extend_from_slice(&limits.maximum_bytes_per_direction.to_le_bytes());
        Ok(bytes)
    }
}

#[cfg(feature = "noise-session")]
fn push(output: &mut Vec<u8>, value: &str) -> Result<(), ProtectedLineError> {
    if value.is_empty() || value.len() > u16::MAX as usize {
        return Err(ProtectedLineError::InvalidBinding);
    }
    if output
        .len()
        .checked_add(2 + value.len())
        .is_none_or(|length| length > MAXIMUM_BINDING_BYTES)
    {
        return Err(ProtectedLineError::BindingTooLarge);
    }
    output.extend_from_slice(&(value.len() as u16).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

#[cfg(feature = "noise-session")]
pub struct ProtectedHandshake {
    role: Role,
    binding: SessionBinding,
    limits: SessionLimits,
    state: Option<NoiseHandshake>,
}

#[cfg(feature = "noise-session")]
impl ProtectedHandshake {
    pub fn new(
        role: Role,
        binding: &SessionBinding,
        limits: SessionLimits,
        preshared_key: [u8; 32],
        ephemeral_private_key: [u8; 32],
    ) -> Result<Self, ProtectedLineError> {
        let limits = limits.validate()?;
        if preshared_key == [0; 32] {
            return Err(ProtectedLineError::EmptyPresharedKey);
        }
        if ephemeral_private_key == [0; 32] {
            return Err(ProtectedLineError::EmptyEphemeralKey);
        }
        let prologue = binding.prologue(limits)?;
        let mut builder = HandshakeStateBuilder::<SuppliedX25519>::new();
        builder
            .set_pattern(noise_nn_psk0())
            .set_is_initiator(role == Role::Initiator)
            .set_prologue(&prologue)
            .set_e(Sensitive::from_slice(&ephemeral_private_key));
        let mut state = builder.build_handshake_state::<ChaCha20Poly1305, Sha256>();
        state.push_psk(&preshared_key);
        Ok(Self {
            role,
            binding: binding.clone(),
            limits,
            state: Some(state),
        })
    }

    pub fn next_message_bytes(&self) -> Result<usize, ProtectedLineError> {
        let state = self.state.as_ref().ok_or(ProtectedLineError::Closed)?;
        if state.completed() {
            return Err(ProtectedLineError::HandshakeIncomplete);
        }
        Ok(state.get_next_message_overhead())
    }

    pub fn is_write_turn(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(HandshakeState::is_write_turn)
    }

    pub fn write_message(&mut self, output: &mut [u8]) -> Result<(), ProtectedLineError> {
        let state = self.state.as_mut().ok_or(ProtectedLineError::Closed)?;
        if !state.is_write_turn() {
            return Err(ProtectedLineError::WrongHandshakeTurn);
        }
        if output.len() != state.get_next_message_overhead() {
            return Err(ProtectedLineError::HandshakeFrameLength);
        }
        state
            .write_message(&[], output)
            .map_err(|_| ProtectedLineError::AuthenticationFailed)
    }

    pub fn read_message(&mut self, input: &[u8]) -> Result<(), ProtectedLineError> {
        let state = self.state.as_mut().ok_or(ProtectedLineError::Closed)?;
        if state.is_write_turn() {
            return Err(ProtectedLineError::WrongHandshakeTurn);
        }
        if input.len() != state.get_next_message_overhead() {
            self.state = None;
            return Err(ProtectedLineError::HandshakeFrameLength);
        }
        if state.read_message(input, &mut []).is_err() {
            self.state = None;
            return Err(ProtectedLineError::AuthenticationFailed);
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<ProtectedSession, ProtectedLineError> {
        let state = self.state.take().ok_or(ProtectedLineError::Closed)?;
        if !state.completed() {
            return Err(ProtectedLineError::HandshakeIncomplete);
        }
        let (initiator_send, responder_send) = state.get_ciphers();
        let (send, receive) = match self.role {
            Role::Initiator => (initiator_send, responder_send),
            Role::Responder => (responder_send, initiator_send),
        };
        Ok(ProtectedSession {
            role: self.role,
            binding: self.binding,
            limits: self.limits,
            send: Some(send),
            receive: Some(receive),
            sent_frames: 0,
            sent_bytes: 0,
            received_frames: 0,
            received_bytes: 0,
            disposition: SessionDisposition::Open,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDisposition {
    Open,
    Closed,
    Cancelled,
    Failed(ProtectedLineError),
}

#[cfg(feature = "noise-session")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEvidence<'a> {
    pub implementation_id: &'static str,
    pub protocol_name: &'static str,
    pub role: Role,
    pub binding: &'a SessionBinding,
    pub limits: SessionLimits,
    pub sent_frames: u64,
    pub sent_bytes: u64,
    pub received_frames: u64,
    pub received_bytes: u64,
    pub disposition: SessionDisposition,
}

#[cfg(feature = "noise-session")]
pub struct ProtectedSession {
    role: Role,
    binding: SessionBinding,
    limits: SessionLimits,
    send: Option<NoiseCipher>,
    receive: Option<NoiseCipher>,
    sent_frames: u64,
    sent_bytes: u64,
    received_frames: u64,
    received_bytes: u64,
    disposition: SessionDisposition,
}

#[cfg(feature = "noise-session")]
impl ProtectedSession {
    pub fn maximum_frame_bytes(&self) -> usize {
        HEADER_BYTES + self.limits.maximum_payload_bytes as usize + TAG_BYTES
    }

    pub fn limits(&self) -> SessionLimits {
        self.limits
    }

    pub fn seal(
        &mut self,
        plaintext: &[u8],
        output: &mut [u8],
    ) -> Result<usize, ProtectedLineError> {
        if self.send.is_none() {
            return Err(ProtectedLineError::Closed);
        }
        if plaintext.len() > self.limits.maximum_payload_bytes as usize {
            return Err(ProtectedLineError::FrameTooLarge);
        }
        if self.sent_frames == self.limits.maximum_frames_per_direction {
            self.fail(ProtectedLineError::FrameLimitExhausted);
            return Err(ProtectedLineError::FrameLimitExhausted);
        }
        let next_bytes = self
            .sent_bytes
            .checked_add(plaintext.len() as u64)
            .ok_or(ProtectedLineError::ByteLimitExhausted)?;
        if next_bytes > self.limits.maximum_bytes_per_direction {
            self.fail(ProtectedLineError::ByteLimitExhausted);
            return Err(ProtectedLineError::ByteLimitExhausted);
        }
        let length = HEADER_BYTES + plaintext.len() + TAG_BYTES;
        if output.len() < length {
            return Err(ProtectedLineError::OutputTooSmall);
        }
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = self.role.outbound();
        output[6..14].copy_from_slice(&self.sent_frames.to_le_bytes());
        output[14..18].copy_from_slice(&(plaintext.len() as u32).to_le_bytes());
        let (header, ciphertext) = output[..length].split_at_mut(HEADER_BYTES);
        let cipher = self.send.as_mut().ok_or(ProtectedLineError::Closed)?;
        cipher.encrypt_ad(header, plaintext, ciphertext);
        self.sent_frames += 1;
        self.sent_bytes = next_bytes;
        Ok(length)
    }

    pub fn open(&mut self, frame: &[u8], output: &mut [u8]) -> Result<usize, ProtectedLineError> {
        if self.receive.is_none() {
            return Err(ProtectedLineError::Closed);
        }
        if frame.len() < HEADER_BYTES + TAG_BYTES || frame[..4] != MAGIC || frame[4] != VERSION {
            self.fail(ProtectedLineError::MalformedFrame);
            return Err(ProtectedLineError::MalformedFrame);
        }
        if frame[5] != self.role.inbound() {
            self.fail(ProtectedLineError::WrongDirection);
            return Err(ProtectedLineError::WrongDirection);
        }
        let sequence = u64::from_le_bytes(frame[6..14].try_into().unwrap());
        if sequence < self.received_frames {
            self.fail(ProtectedLineError::Replay);
            return Err(ProtectedLineError::Replay);
        }
        if sequence > self.received_frames {
            self.fail(ProtectedLineError::Reordered);
            return Err(ProtectedLineError::Reordered);
        }
        if self.received_frames == self.limits.maximum_frames_per_direction {
            self.fail(ProtectedLineError::FrameLimitExhausted);
            return Err(ProtectedLineError::FrameLimitExhausted);
        }
        let payload_bytes = u32::from_le_bytes(frame[14..18].try_into().unwrap()) as usize;
        if payload_bytes > self.limits.maximum_payload_bytes as usize {
            self.fail(ProtectedLineError::FrameTooLarge);
            return Err(ProtectedLineError::FrameTooLarge);
        }
        let expected_frame_bytes = HEADER_BYTES + payload_bytes + TAG_BYTES;
        if frame.len() < expected_frame_bytes {
            self.fail(ProtectedLineError::TruncatedFrame);
            return Err(ProtectedLineError::TruncatedFrame);
        }
        if frame.len() > expected_frame_bytes {
            self.fail(ProtectedLineError::MalformedFrame);
            return Err(ProtectedLineError::MalformedFrame);
        }
        if output.len() < payload_bytes {
            return Err(ProtectedLineError::OutputTooSmall);
        }
        let next_bytes = self
            .received_bytes
            .checked_add(payload_bytes as u64)
            .ok_or(ProtectedLineError::ByteLimitExhausted)?;
        if next_bytes > self.limits.maximum_bytes_per_direction {
            self.fail(ProtectedLineError::ByteLimitExhausted);
            return Err(ProtectedLineError::ByteLimitExhausted);
        }
        let result = self
            .receive
            .as_mut()
            .ok_or(ProtectedLineError::Closed)?
            .decrypt_ad(
                &frame[..HEADER_BYTES],
                &frame[HEADER_BYTES..],
                &mut output[..payload_bytes],
            );
        if result.is_err() {
            self.fail(ProtectedLineError::AuthenticationFailed);
            return Err(ProtectedLineError::AuthenticationFailed);
        }
        self.received_frames += 1;
        self.received_bytes = next_bytes;
        Ok(payload_bytes)
    }

    pub fn evidence(&self) -> SessionEvidence<'_> {
        SessionEvidence {
            implementation_id: IMPLEMENTATION_ID,
            protocol_name: PROTOCOL_NAME,
            role: self.role,
            binding: &self.binding,
            limits: self.limits,
            sent_frames: self.sent_frames,
            sent_bytes: self.sent_bytes,
            received_frames: self.received_frames,
            received_bytes: self.received_bytes,
            disposition: self.disposition,
        }
    }

    pub fn close(&mut self) {
        self.retire(SessionDisposition::Closed);
    }

    pub fn cancel(&mut self) {
        self.retire(SessionDisposition::Cancelled);
    }

    pub(crate) fn fail(&mut self, error: ProtectedLineError) {
        self.retire(SessionDisposition::Failed(error));
    }

    fn retire(&mut self, disposition: SessionDisposition) {
        if self.disposition == SessionDisposition::Open {
            self.send = None;
            self.receive = None;
            self.disposition = disposition;
        }
    }
}
