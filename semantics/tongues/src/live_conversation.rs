//! Portable finite requirements for one live spoken conversation turn.
//!
//! This module owns semantic capacity. Provider names, executables, process
//! layout, sample-rate quirks, and other realization facts remain Host truth.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveConversationSpeechRequirements {
    /// Capacity required for one recognition request admitted by the portable
    /// speech face.
    pub recognition_audio_bytes: u32,
    /// Capacity required for the exact bounded recognized text value.
    pub recognized_text_bytes: u16,
    /// Capacity required for one language-aware committed speech segment.
    pub speakable_segment_bytes: u32,
    /// Capacity required for one portable synthesized PCM extent.
    pub synthesized_pcm_bytes: u32,
}

pub const fn live_conversation_speech_requirements() -> LiveConversationSpeechRequirements {
    LiveConversationSpeechRequirements {
        recognition_audio_bytes: MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
        recognized_text_bytes: MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        speakable_segment_bytes: MAXIMUM_SPEAKABLE_SEGMENT_BYTES as u32,
        synthesized_pcm_bytes: MAXIMUM_PCM_BYTES,
    }
}

pub fn recognition_capacity_satisfies_live_conversation(
    maximum_audio_bytes: u32,
    maximum_text_bytes: u16,
) -> bool {
    let required = live_conversation_speech_requirements();
    maximum_audio_bytes >= required.recognition_audio_bytes
        && maximum_text_bytes >= required.recognized_text_bytes
}

pub fn synthesis_capacity_satisfies_live_conversation(
    maximum_text_bytes: u32,
    maximum_pcm_bytes: u32,
) -> bool {
    let required = live_conversation_speech_requirements();
    maximum_text_bytes >= required.speakable_segment_bytes
        && maximum_pcm_bytes >= required.synthesized_pcm_bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_conversation_capacity_is_derived_only_from_portable_speech_bounds() {
        let required = live_conversation_speech_requirements();
        assert_eq!(
            required.recognition_audio_bytes,
            MAXIMUM_RECOGNITION_AUDIO_BYTES as u32
        );
        assert_eq!(
            required.recognized_text_bytes,
            MAXIMUM_RECOGNIZED_TEXT_BYTES as u16
        );
        assert_eq!(
            required.speakable_segment_bytes,
            MAXIMUM_SPEAKABLE_SEGMENT_BYTES as u32
        );
        assert_eq!(required.synthesized_pcm_bytes, MAXIMUM_PCM_BYTES);
        assert!(recognition_capacity_satisfies_live_conversation(
            required.recognition_audio_bytes,
            required.recognized_text_bytes
        ));
        assert!(synthesis_capacity_satisfies_live_conversation(
            required.speakable_segment_bytes,
            required.synthesized_pcm_bytes
        ));
    }
}
