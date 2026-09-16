//! Truthful composition of the initialized providers needed by Live Conversation.

use crate::{
    hosted_local_model::HostedLocalModelAdapter, hosted_speech::PiperSpeechAdapter,
    hosted_speech_recognition::WhisperSpeechAdapter, StdHost, StdHostComposition, StdHostConfig,
};

pub struct VoiceHostProviders {
    pub recognition: WhisperSpeechAdapter,
    pub model: Box<dyn HostedLocalModelAdapter>,
    pub synthesis: PiperSpeechAdapter,
}

impl StdHost {
    pub fn new_with_voice_conversation(
        config: StdHostConfig,
        composition: StdHostComposition,
        providers: VoiceHostProviders,
        context: &conduit_body::BodyConversationContext,
    ) -> Result<Self, String> {
        validate_recognition(&providers.recognition)?;
        validate_synthesis(&providers.synthesis)?;

        let mut host = Self::new_with_local_model_capabilities(
            config,
            composition,
            providers.model,
            vec![
                conduit_std_offers::recognized_turn_commit_offer(),
                conduit_std_offers::generated_speech_commit_offer(),
            ],
        )?;
        host.advertisement.resources.extend([
            conduit_core::resource_offer(
                "std/whisper-process",
                conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
                1,
            ),
            crate::hosted_speech::process_resource_offer(),
        ]);
        host.advertisement
            .capabilities
            .push(conduit_std_offers::whisper_speech_offer());
        host.advertisement
            .capabilities
            .push(conduit_std_offers::piper_streaming_speech_offer());
        host.advertisement
            .capabilities
            .push(conduit_std_offers::audio_convert_pcm_profile_offer());
        host.advertisement.resources.sort();
        host.advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        host.speech_recognition = Some(providers.recognition);
        host.speech_synthesis = Some(providers.synthesis);
        host.install_body_conversation_context(context)?;
        host.kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement)?;
        Ok(host)
    }
}

fn validate_recognition(adapter: &WhisperSpeechAdapter) -> Result<(), String> {
    if adapter.limits().maximum_audio_bytes
        < conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32
        || adapter.limits().maximum_text_bytes
            < conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16
    {
        return Err("initialized Whisper adapter does not satisfy Live Conversation".into());
    }
    Ok(())
}

fn validate_synthesis(adapter: &PiperSpeechAdapter) -> Result<(), String> {
    if adapter.discovery().sample_rate_hz != 22_050
        || adapter.limits().maximum_frames < conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2)
        || adapter.limits().maximum_blocks < conduit_std_offers::PIPER_MAXIMUM_BLOCKS
        || adapter.limits().maximum_text_bytes
            < conduit_tongues::MAXIMUM_SPEAKABLE_SEGMENT_BYTES as u32
    {
        return Err("initialized Piper adapter does not satisfy Live Conversation".into());
    }
    Ok(())
}
