//! Host-owned speech and artifact attachment, before ordinary planning.
impl crate::StdHost {
    /// Attach deterministic proof PCM and an exact WAV artifact resource.
    /// This establishes an artifact effect, not intelligible speech or hearing.
    pub fn attach_deterministic_speech_and_wav_artifact(
        &mut self,
        artifact: crate::hosted_wav_artifact::WavArtifactSelection,
    ) -> Result<(), String> {
        self.attach_spoken_output(artifact, None)
    }

    /// Attach an initialized, explicitly authorized speech provider and its
    /// artifact destination. Both remain exact planned Host resources.
    pub fn attach_espeak_speech_and_wav_artifact(
        &mut self,
        adapter: crate::hosted_speech_synthesis::EspeakSpeechAdapter,
        artifact: crate::hosted_wav_artifact::WavArtifactSelection,
    ) -> Result<(), String> {
        adapter
            .validate_host(
                &self.advertisement.host_id,
                &self.advertisement.boot_id,
                self.advertisement.offer_generation,
            )
            .map_err(|error| error.to_string())?;
        self.attach_spoken_output(artifact, Some(adapter))
    }

    /// The exact grant selected when this provider was initialized. Discovery
    /// and merely finding a local executable never manufacture this authority.
    pub fn speech_synthesis_authority_grant(&self) -> Result<conduit_core::AuthorityGrant, String> {
        self.speech_synthesis
            .as_ref()
            .map(|adapter| adapter.authority_grant())
            .ok_or_else(|| "std Host has no initialized real speech provider".into())
    }

    pub fn streaming_speech_authority_grant(&self) -> Result<conduit_core::AuthorityGrant, String> {
        self.speech_synthesis
            .as_ref()
            .map(|adapter| adapter.streaming_authority_grant())
            .ok_or_else(|| "std Host has no initialized real speech provider".into())
    }
    fn attach_spoken_output(
        &mut self,
        artifact: crate::hosted_wav_artifact::WavArtifactSelection,
        adapter: Option<crate::hosted_speech_synthesis::EspeakSpeechAdapter>,
    ) -> Result<(), String> {
        if self.wav_artifact.is_some()
            || self.speech_synthesis.is_some()
            || artifact.boot_id != self.advertisement.boot_id
            || artifact.offer_generation != self.advertisement.offer_generation
        {
            return Err("spoken artifact is stale or already selected".into());
        }
        let mut advertisement = self.advertisement.clone();
        advertisement.resources.push(conduit_core::resource_offer(
            artifact.pool_id().as_str(),
            conduit_std_offers::AUDIO_WAV_ARTIFACT_RESOURCE_CLASS,
            1,
        ));
        let speech = if let Some(adapter) = &adapter {
            advertisement.resources.push(adapter.resource_offer());
            let commit = conduit_std_offers::generated_speech_commit_offer();
            advertisement
                .capabilities
                .retain(|offer| offer.kind_id != commit.kind_id);
            advertisement.capabilities.push(commit);
            let streaming = adapter.streaming_offer();
            advertisement
                .capabilities
                .retain(|offer| offer.kind_id != streaming.kind_id);
            advertisement.capabilities.push(streaming);
            let selected = adapter.offer();
            // Explicit provider attachment replaces the default realization of
            // this kind, including the deterministic test-composition offer.
            advertisement
                .capabilities
                .retain(|offer| offer.kind_id != selected.kind_id);
            selected
        } else {
            conduit_std_offers::deterministic_speech_offer()
        };
        advertisement.capabilities.extend([
            speech,
            conduit_std_offers::audio_convert_pcm_profile_offer(),
        ]);
        advertisement
            .capabilities
            .extend(conduit_std_offers::spoken_mask_offers());
        if adapter.is_some() {
            let projection = conduit_std_offers::generated_stream_speech_offer();
            advertisement
                .capabilities
                .retain(|offer| offer.kind_id != projection.kind_id);
            advertisement.capabilities.push(projection);
        }
        advertisement
            .capabilities
            .push(conduit_std_offers::audio_write_wav_artifact_offer());
        advertisement.resources.sort();
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let resources = crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = resources;
        self.wav_artifact = Some(artifact);
        self.speech_synthesis = adapter;
        Ok(())
    }
}
