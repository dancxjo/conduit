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
    pub fn new_with_voice_providers(
        config: StdHostConfig,
        composition: StdHostComposition,
        providers: VoiceHostProviders,
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
                conduit_std_offers::speech_window_to_clip_std_offer(),
                conduit_std_offers::speech_result_to_event_stream_std_offer(),
                // A provider-ready Voice Host can realize the supervisor-owned
                // Body context source even before one current value is
                // published. The value itself is installed only after an exact
                // Body Wake/Plan exists.
                conduit_std_offers::body_conversation_context_std_offer(),
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
        // Retain the old single-shot face, and expose the explicit clip leaf
        // selected by the Tongues streaming-recognition Back.
        host.advertisement
            .capabilities
            .push(conduit_std_offers::whisper_speech_offer());
        host.advertisement
            .capabilities
            .push(conduit_std_offers::whisper_clip_speech_offer());
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
        host.kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement)?;
        Ok(host)
    }

    pub fn new_with_voice_conversation(
        config: StdHostConfig,
        composition: StdHostComposition,
        providers: VoiceHostProviders,
        context: &conduit_body::BodyConversationContext,
    ) -> Result<Self, String> {
        let mut host = Self::new_with_voice_providers(config, composition, providers)?;
        host.install_body_conversation_context(context)?;
        Ok(host)
    }
}

fn validate_recognition(adapter: &WhisperSpeechAdapter) -> Result<(), String> {
    let limits = adapter.limits();
    if !conduit_tongues::recognition_capacity_satisfies_live_conversation(
        limits.maximum_audio_bytes,
        limits.maximum_text_bytes,
    ) {
        return Err("initialized Whisper adapter does not satisfy Live Conversation".into());
    }
    Ok(())
}

fn validate_synthesis(adapter: &PiperSpeechAdapter) -> Result<(), String> {
    let limits = adapter.limits();
    let pcm_bytes = limits.maximum_frames.saturating_mul(2);
    if adapter.discovery().sample_rate_hz != 22_050
        || limits.maximum_blocks < conduit_std_offers::PIPER_MAXIMUM_BLOCKS
        || !conduit_tongues::synthesis_capacity_satisfies_live_conversation(
            limits.maximum_text_bytes,
            pcm_bytes,
        )
    {
        return Err("initialized Piper adapter does not satisfy Live Conversation".into());
    }
    Ok(())
}
