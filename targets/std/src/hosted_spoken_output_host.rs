impl crate::StdHost {
    pub fn attach_piper_speech_and_wav_artifact(
        &mut self,
        adapter: crate::hosted_speech::PiperSpeechAdapter,
        artifact: crate::hosted_wav_artifact::WavArtifactSelection,
    ) -> Result<(), String> {
        if self.speech_synthesis.is_some() || self.playback.is_some() || self.wav_artifact.is_some()
        {
            return Err("std Host already has initialized speech or audio output".into());
        }
        if adapter.discovery().sample_rate_hz != 22_050
            || adapter.limits().maximum_frames < conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2)
            || adapter.limits().maximum_blocks < conduit_std_offers::PIPER_MAXIMUM_BLOCKS
        {
            return Err("initialized Piper adapter does not satisfy its offered profile".into());
        }
        if artifact.boot_id != self.advertisement.boot_id
            || artifact.offer_generation != self.advertisement.offer_generation
        {
            return Err(
                "WAV artifact selection does not match the advertised Boot/generation".into(),
            );
        }
        let mut advertisement = self.advertisement.clone();
        advertisement.resources.extend([
            crate::hosted_speech::process_resource_offer(),
            conduit_core::resource_offer(
                artifact.pool_id().as_str(),
                conduit_std_offers::AUDIO_WAV_ARTIFACT_RESOURCE_CLASS,
                1,
            ),
        ]);
        advertisement.capabilities.retain(|offer| {
            offer.implementation.implementation_id.as_str()
                != conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION
        });
        advertisement.capabilities.extend([
            conduit_std_offers::piper_speech_offer(),
            conduit_std_offers::audio_convert_pcm_profile_offer(),
            conduit_std_offers::audio_write_wav_artifact_offer(),
        ]);
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.speech_synthesis = Some(adapter);
        self.wav_artifact = Some(artifact);
        self.kernel_resources = kernel_resources;
        Ok(())
    }

    pub fn attach_piper_speech_and_playback(
        &mut self,
        adapter: crate::hosted_speech::PiperSpeechAdapter,
        playback: crate::hosted_audio::HostedPlaybackSelection,
    ) -> Result<(), String> {
        if self.speech_synthesis.is_some() || self.playback.is_some() {
            return Err("std Host already has initialized speech or playback".into());
        }
        if adapter.discovery().sample_rate_hz != 22_050
            || adapter.limits().maximum_frames < conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2)
            || adapter.limits().maximum_blocks < conduit_std_offers::PIPER_MAXIMUM_BLOCKS
        {
            return Err("initialized Piper adapter does not satisfy its offered profile".into());
        }
        if playback.boot_id != self.advertisement.boot_id
            || playback.offer_generation != self.advertisement.offer_generation
        {
            return Err(
                "playback observation does not match the advertised Boot/generation".into(),
            );
        }
        let mut advertisement = self.advertisement.clone();
        advertisement.resources.extend([
            crate::hosted_speech::process_resource_offer(),
            conduit_core::resource_offer(
                playback.pool_id().as_str(),
                conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS,
                1,
            ),
        ]);
        advertisement.capabilities.retain(|offer| {
            offer.implementation.implementation_id.as_str()
                != conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION
        });
        advertisement.capabilities.extend([
            conduit_std_offers::piper_speech_offer(),
            conduit_std_offers::audio_convert_pcm_profile_offer(),
            conduit_std_offers::audio_play_alsa_hw_offer(),
        ]);
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.speech_synthesis = Some(adapter);
        self.playback = Some(playback);
        self.kernel_resources = kernel_resources;
        Ok(())
    }
}
