//! Exact terminal information for the portable `audio/tone` transform.

use crate::SoundInfoError;

pub const AUDIO_TONE_TERMINAL_INFO_ID: &str = "audio/tone-terminal@1";
pub const AUDIO_TONE_TERMINAL_ENCODED_LEN: usize = 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AudioToneTerminal {
    Cancelled,
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
