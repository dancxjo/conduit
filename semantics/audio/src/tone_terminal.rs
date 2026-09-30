//! Exact terminal information for the portable `audio/tone` transform.

use crate::{AudioToneTerminal, AudioToneTerminalRepresentation};
use conduit_core::{KindId, StructuredInfoType, StructuredInfoTypeShape};

pub const AUDIO_TONE_TERMINAL_INFO_ID: &str = AudioToneTerminalRepresentation::IDENTITY;
pub const AUDIO_TONE_TERMINAL_ENCODED_LEN: usize = AudioToneTerminalRepresentation::EXACT_BYTES;
pub use conduit_form::rust_binding::NativeRepresentationRefusal as AudioToneTerminalCodecRefusal;

pub fn audio_tone_terminal_type() -> StructuredInfoType {
    AudioToneTerminal::semantic_type().expect("generated audio terminal Type is checked")
}

pub fn audio_tone_terminal_kind_id() -> KindId {
    let semantic = audio_tone_terminal_type();
    let StructuredInfoTypeShape::Variant { schema, .. } = semantic.shape() else {
        unreachable!("checked AudioToneTerminal is a semantic variant")
    };
    schema.clone()
}

impl AudioToneTerminal {
    pub const fn encode(self) -> [u8; AUDIO_TONE_TERMINAL_ENCODED_LEN] {
        AudioToneTerminalRepresentation::encode(self)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, AudioToneTerminalCodecRefusal> {
        AudioToneTerminalRepresentation::decode(encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_exact_bounded_terminal_info() {
        assert_eq!(
            AudioToneTerminal::decode(&[0]),
            Ok(AudioToneTerminal::Cancelled)
        );
        assert!(AudioToneTerminal::decode(&[1]).is_err());
        assert!(AudioToneTerminal::decode(&[]).is_err());
    }
}
