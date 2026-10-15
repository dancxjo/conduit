//! Ordered sound contract inventory.
use super::*;
pub fn sound_contracts_with_revisions() -> [(StandardKindContract, &'static str); 14] {
    [
        (
            crate::audio_time_analysis_contract(),
            crate::AUDIO_TIME_ANALYSIS_REVISION,
        ),
        (
            crate::audio_spectral_analysis_contract(),
            crate::AUDIO_SPECTRAL_ANALYSIS_REVISION,
        ),
        (sound_tone_play_contract(), SOUND_TONE_PLAY_REVISION),
        (music_input_contract(), MUSIC_INPUT_REVISION),
        (music_play_contract(), MUSIC_PLAY_REVISION),
        (music_synth_contract(), MUSIC_SYNTH_REVISION),
        (audio_render_demand_contract(), AUDIO_RENDER_DEMAND_REVISION),
        (audio_play_contract(), AUDIO_PLAY_REVISION),
        (
            audio_continuous_play_contract(),
            AUDIO_CONTINUOUS_PLAY_REVISION,
        ),
        (audio_tone_contract(), AUDIO_TONE_REVISION),
        (
            audio_continuous_tone_contract(),
            AUDIO_CONTINUOUS_TONE_REVISION,
        ),
        (audio_gain_contract(), AUDIO_GAIN_REVISION),
        (
            audio_capture_push_to_talk_contract(),
            AUDIO_CAPTURE_PUSH_TO_TALK_REVISION,
        ),
        (
            audio_convert_pcm_profile_contract(),
            AUDIO_CONVERT_PCM_PROFILE_REVISION,
        ),
    ]
}

pub fn sound_contract_revision(kind: &str) -> Option<KindIdentity> {
    sound_contracts_with_revisions()
        .into_iter()
        .find(|(contract, _)| contract.kind_id.as_str() == kind)
        .map(|(_, revision)| KindIdentity::from(revision))
}
