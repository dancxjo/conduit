//! Explicit external-Fore scope; public metadata is never possession or authority.
use super::machine::{FrameDirection, Scope};
use super::*;

const MAGIC: [u8; 4] = *b"CNDF";
const VERSION: u8 = 1;

/// A per-Fore scope commitment independently pinned by the trusted consumer.
/// Derive it from ORIGINAL verified Body, local Host/Boot, Plan/active Play,
/// fragment, exact Fore endpoint/Cord/direction/front ABI and value contract,
/// authorized caller Host/Boot, full selected Line contract/limits and actual
/// session identity. A workset digest shared across Fore routes is insufficient.
/// The derivation must commit these lifecycle limits and abnormal kind too.
///
/// Nonzero hashes validate representation only. Receipt of this metadata never
/// grants authority: the Root admission ticket, physical/protected carrier
/// possession and authentication must be independently checked by the consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalForeSessionBinding {
    pub scope_commitment: [u8; 32],
    pub limits: SessionLimits,
    pub abnormal_kind: Option<KindId>,
    pub maximum_frame_bytes: u32,
}
impl ExternalForeSessionBinding {
    pub fn validate(&self) -> Result<(), WireError> {
        // Header, sequence and byte-field length precede either bounded payload.
        let maximum_offer_bytes = 40_u32
            .checked_add(8)
            .and_then(|bytes| bytes.checked_add(4))
            .and_then(|bytes| bytes.checked_add(self.limits.maximum_payload_bytes))
            .ok_or(WireError::InvalidSession)?;
        if self.scope_commitment == [0; 32]
            || self.limits.maximum_in_flight_items == 0
            || self.limits.maximum_payload_bytes == 0
            || self.limits.maximum_payload_bytes > self.limits.maximum_buffered_bytes
            || self.maximum_frame_bytes < maximum_offer_bytes
            || self
                .abnormal_kind
                .as_ref()
                .is_some_and(|k| k.as_str().is_empty() || k.as_str().len() > MAX_ID_BYTES)
        {
            return Err(WireError::InvalidSession);
        }
        Ok(())
    }
    pub fn frame<'a>(
        &self,
        message: ExternalForeSessionMessage<'a>,
    ) -> ExternalForeSessionFrame<'a> {
        ExternalForeSessionFrame {
            scope_commitment: self.scope_commitment,
            message,
        }
    }
}
impl Scope for ExternalForeSessionBinding {
    fn limits(&self) -> SessionLimits {
        self.limits
    }
    fn abnormal_kind(&self) -> Option<&KindId> {
        self.abnormal_kind.as_ref()
    }
}
/// Hello has no invented fragment, active Play or remote-Cord attachment fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalForeSessionMessage<'a> {
    Hello,
    /// All existing non-Hello messages retain their current meaning and codec.
    Message(SessionMessage<'a>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalForeSessionFrame<'a> {
    pub scope_commitment: [u8; 32],
    pub message: ExternalForeSessionMessage<'a>,
}
/// Same delivery verifier as remote Cords, with a distinct exact identity scope.
/// Delivered acknowledges transport/kernel custody, never durable recording.
/// No external attachment resume is provided: a fresh authenticated session and
/// application cursor recovery remain separate obligations after interruption.
#[derive(Clone)]
pub struct ExternalForeSessionMachine {
    lifecycle: SessionMachine<ExternalForeSessionBinding>,
}
impl ExternalForeSessionMachine {
    pub fn new(
        binding: ExternalForeSessionBinding,
        role: crate::SessionRole,
    ) -> Result<Self, WireError> {
        binding.validate()?;
        Ok(Self {
            lifecycle: SessionMachine::with_scope(binding, role),
        })
    }
    pub fn binding(&self) -> &ExternalForeSessionBinding {
        &self.lifecycle.binding
    }
    pub fn is_active(&self) -> bool {
        self.lifecycle.is_active()
    }
    pub fn is_terminal(&self) -> bool {
        self.lifecycle.is_terminal()
    }
    pub fn next_sequence(&self) -> u64 {
        self.lifecycle.next_sequence()
    }
    pub fn admit_inbound(&mut self, frame: ExternalForeSessionFrame<'_>) -> Result<(), WireError> {
        self.admit(FrameDirection::Inbound, frame)
    }
    pub fn admit_outbound(&mut self, frame: ExternalForeSessionFrame<'_>) -> Result<(), WireError> {
        self.admit(FrameDirection::Outbound, frame)
    }
    fn admit(
        &mut self,
        direction: FrameDirection,
        frame: ExternalForeSessionFrame<'_>,
    ) -> Result<(), WireError> {
        if frame.scope_commitment != self.lifecycle.binding.scope_commitment {
            return Err(WireError::InvalidSession);
        }
        match frame.message {
            ExternalForeSessionMessage::Hello => self.lifecycle.mark_hello(direction),
            ExternalForeSessionMessage::Message(message) => {
                self.lifecycle.admit_message(direction, message)
            }
        }
    }
}
/// Encoding is not admission; consumers must independently check exact scope.
pub fn encode_external_fore_session_frame_into(
    frame: ExternalForeSessionFrame<'_>,
    output: &mut [u8],
    maximum_payload_bytes: u32,
    maximum_frame_bytes: u32,
) -> Result<usize, WireError> {
    let mut writer = Writer::new(
        output,
        usize::try_from(maximum_frame_bytes).map_err(|_| WireError::InvalidLimits)?,
    )?;
    writer.bytes(&MAGIC)?;
    writer.u8(VERSION)?;
    let kind = match frame.message {
        ExternalForeSessionMessage::Hello => 1,
        ExternalForeSessionMessage::Message(SessionMessage::Hello(_)) => {
            return Err(WireError::InvalidMessageKind)
        }
        ExternalForeSessionMessage::Message(message) => message_kind(message),
    };
    writer.u8(kind)?;
    writer.u16(PROTOCOL_VERSION)?;
    writer.bytes(&frame.scope_commitment)?;
    if let ExternalForeSessionMessage::Message(message) = frame.message {
        message_codec::encode(&mut writer, message, maximum_payload_bytes)?;
    }
    Ok(writer.len())
}
pub fn decode_external_fore_session_frame(
    frame: &[u8],
    maximum_payload_bytes: u32,
    maximum_frame_bytes: u32,
) -> Result<ExternalForeSessionFrame<'_>, WireError> {
    if frame.len() > usize::try_from(maximum_frame_bytes).map_err(|_| WireError::InvalidLimits)? {
        return Err(WireError::OversizedFrame);
    }
    let mut cursor = Cursor::new(frame);
    if cursor.take(4)? != MAGIC {
        return Err(WireError::InvalidMagic);
    }
    if cursor.u8()? != VERSION {
        return Err(WireError::UnsupportedWireFormat);
    }
    let kind = cursor.u8()?;
    if cursor.u16()? != PROTOCOL_VERSION {
        return Err(WireError::WrongProtocolVersion);
    }
    let mut scope_commitment = [0; 32];
    scope_commitment.copy_from_slice(cursor.take(32)?);
    let message = if kind == 1 {
        ExternalForeSessionMessage::Hello
    } else {
        ExternalForeSessionMessage::Message(message_codec::decode(
            &mut cursor,
            kind,
            maximum_payload_bytes,
        )?)
    };
    if !cursor.is_empty() {
        return Err(WireError::TrailingGarbage);
    }
    Ok(ExternalForeSessionFrame {
        scope_commitment,
        message,
    })
}
#[cfg(test)]
mod tests;
