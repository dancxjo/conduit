//! Installation and proof-source attachment for the initialized Whisper Host.
use super::*;

impl StdHost {
    pub fn new_with_whisper_speech_recognition(
        config: StdHostConfig,
        composition: StdHostComposition,
        adapter: hosted_speech_recognition::WhisperSpeechAdapter,
    ) -> Result<Self, String> {
        if adapter.limits().maximum_audio_bytes
            < conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32
            || adapter.limits().maximum_text_bytes
                < conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16
        {
            return Err("initialized Whisper adapter does not satisfy its offered profile".into());
        }
        let mut advertisement =
            composition::build_advertisement(config, composition, None, None, None, false);
        advertisement.resources.push(conduit_core::resource_offer(
            "std/whisper-process",
            conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
            1,
        ));
        advertisement.capabilities.push(adapter.offer());
        #[cfg(all(test, feature = "local-model-proof"))]
        advertisement.capabilities.extend([
            installed_std::test_local_model_io::house_source_offers()[0].clone(),
            installed_std::test_local_model_io::house_text_sink_offer(),
        ]);
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: Some(adapter),
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
        })
    }

    pub fn new_with_whisper_clip_speech_recognition(
        config: StdHostConfig,
        composition: StdHostComposition,
        adapter: hosted_speech_recognition::WhisperSpeechAdapter,
    ) -> Result<Self, String> {
        if adapter.limits().maximum_audio_bytes < conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
            || adapter.limits().maximum_text_bytes
                < conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16
        {
            return Err(
                "initialized Whisper adapter does not satisfy its offered clip profile".into(),
            );
        }
        let mut advertisement =
            composition::build_advertisement(config, composition, None, None, None, false);
        advertisement.resources.push(conduit_core::resource_offer(
            "std/whisper-process",
            conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
            1,
        ));
        advertisement.capabilities.push(adapter.clip_offer());
        #[cfg(all(test, feature = "local-model-proof"))]
        advertisement.capabilities.extend([
            installed_std::test_local_model_io::house_source_offers()[1].clone(),
            installed_std::test_local_model_io::house_text_sink_offer(),
        ]);
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: Some(adapter),
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
        })
    }

    /// Attaches one bounded recorded clip as an explicit proof-only source.
    #[cfg(feature = "local-model-proof")]
    pub fn attach_proof_pcm_clip_source(&mut self, clip: Vec<u8>) -> Result<(), String> {
        self.speech_recognition
            .as_mut()
            .ok_or_else(|| "std Host has no initialized Whisper recognizer".to_string())?
            .set_proof_pcm_clip(clip)
            .map_err(|error| format!("attach proof PCM clip: {error:?}"))?;
        for offer in [
            installed_std::test_local_model_io::house_source_offers()[1].clone(),
            installed_std::test_local_model_io::house_text_sink_offer(),
            installed_std::test_local_model_io::house_recognition_sink_offer(),
        ] {
            if !self
                .advertisement
                .capabilities
                .iter()
                .any(|candidate| candidate.capability_id == offer.capability_id)
            {
                self.advertisement.capabilities.push(offer);
            }
        }
        self.advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        self.kernel_resources = kernel_preparation::KernelResourceLedger::new(&self.advertisement)?;
        Ok(())
    }

    /// Adds one initialized Whisper clip realization and its admitted proof input.
    #[cfg(feature = "local-model-proof")]
    pub fn attach_whisper_clip_proof(
        &mut self,
        mut adapter: hosted_speech_recognition::WhisperSpeechAdapter,
        clip: Vec<u8>,
    ) -> Result<(), String> {
        if self.speech_recognition.is_some() {
            return Err("std Host already has an initialized speech recognizer".into());
        }
        if adapter.limits().maximum_audio_bytes < conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
            || adapter.limits().maximum_text_bytes
                < conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16
        {
            return Err(
                "initialized Whisper adapter does not satisfy its offered clip profile".into(),
            );
        }
        adapter
            .set_proof_pcm_clip(clip)
            .map_err(|error| format!("attach proof PCM clip: {error:?}"))?;
        let mut advertisement = self.advertisement.clone();
        advertisement.resources.push(conduit_core::resource_offer(
            "std/whisper-process",
            conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
            1,
        ));
        advertisement.capabilities.push(adapter.clip_offer());
        for offer in [
            installed_std::test_local_model_io::house_source_offers()[1].clone(),
            installed_std::test_local_model_io::house_text_sink_offer(),
            installed_std::test_local_model_io::house_recognition_sink_offer(),
        ] {
            if !advertisement
                .capabilities
                .iter()
                .any(|candidate| candidate.capability_id == offer.capability_id)
            {
                advertisement.capabilities.push(offer);
            }
        }
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.speech_recognition = Some(adapter);
        self.kernel_resources = kernel_resources;
        Ok(())
    }
}
