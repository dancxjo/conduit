//! Exact terminal information for the portable `audio/tone` transform.

use crate::{AudioToneTerminal, SoundInfoError};
use conduit_core::{KindId, StructuredInfoType, StructuredInfoTypeShape};

pub const AUDIO_TONE_TERMINAL_ENCODED_LEN: usize = 1;

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
        [0]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SoundInfoError> {
        match encoded {
            [0] => Ok(Self::Cancelled),
            [actual] => Err(SoundInfoError::InvalidTag {
                field: "audio-tone-terminal",
                actual: *actual,
            }),
            actual => Err(SoundInfoError::WrongLength {
                expected: AUDIO_TONE_TERMINAL_ENCODED_LEN,
                actual: actual.len(),
            }),
        }
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
