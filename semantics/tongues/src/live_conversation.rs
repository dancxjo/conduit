//! Portable finite requirements for one live spoken conversation turn.
//!
//! This module owns semantic capacity. Provider names, executables, process
//! layout, sample-rate quirks, and other realization facts remain Host truth.

use crate::{
    LiveConversationSpeechRequirements, MAXIMUM_PCM_BYTES, MAXIMUM_RECOGNIZED_TEXT_BYTES,
    MAXIMUM_SPEAKABLE_SEGMENT_BYTES,
};

pub fn live_conversation_speech_requirements() -> LiveConversationSpeechRequirements {
    LiveConversationSpeechRequirements::new(
        conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
        MAXIMUM_SPEAKABLE_SEGMENT_BYTES as u32,
        MAXIMUM_PCM_BYTES,
    )
    .expect("portable speech bounds are valid native capacity values")
}

pub fn recognition_capacity_satisfies_live_conversation(
    maximum_audio_bytes: u32,
    maximum_text_bytes: u16,
) -> bool {
    let required = live_conversation_speech_requirements();
    maximum_audio_bytes >= required.recognition_audio_bytes()
        && maximum_text_bytes >= required.recognized_text_bytes()
}

pub fn synthesis_capacity_satisfies_live_conversation(
    maximum_text_bytes: u32,
    maximum_pcm_bytes: u32,
) -> bool {
    let required = live_conversation_speech_requirements();
    maximum_text_bytes >= required.speakable_segment_bytes()
        && maximum_pcm_bytes >= required.synthesized_pcm_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_conversation_capacity_is_derived_only_from_portable_speech_bounds() {
        let required = live_conversation_speech_requirements();
        assert_eq!(
            required.recognition_audio_bytes(),
            conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
        );
        assert_eq!(
            required.recognized_text_bytes(),
            MAXIMUM_RECOGNIZED_TEXT_BYTES as u16
        );
        assert_eq!(
            required.speakable_segment_bytes(),
            MAXIMUM_SPEAKABLE_SEGMENT_BYTES as u32
        );
        assert_eq!(required.synthesized_pcm_bytes(), MAXIMUM_PCM_BYTES);
        assert!(recognition_capacity_satisfies_live_conversation(
            required.recognition_audio_bytes(),
            required.recognized_text_bytes()
        ));
        assert!(synthesis_capacity_satisfies_live_conversation(
            required.speakable_segment_bytes(),
            required.synthesized_pcm_bytes()
        ));
    }
}
