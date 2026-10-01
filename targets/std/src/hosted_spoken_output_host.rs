impl crate::StdHost {
    /// Attach the bounded repository-owned deterministic speech implementation
    /// and an exact WAV artifact resource. This is an artifact-effect route:
    /// it proves accepted synthesis and file retention, never playback or
    /// human hearing.
    pub fn attach_deterministic_speech_and_wav_artifact(
        &mut self,
        artifact: crate::hosted_wav_artifact::WavArtifactSelection,
    ) -> Result<(), String> {
        if self.wav_artifact.is_some()
            || artifact.boot_id != self.advertisement.boot_id
            || artifact.offer_generation != self.advertisement.offer_generation
        {
            return Err("deterministic spoken proof artifact is stale or already selected".into());
        }
        self.advertisement
            .resources
            .push(conduit_core::resource_offer(
                artifact.pool_id().as_str(),
                conduit_std_offers::AUDIO_WAV_ARTIFACT_RESOURCE_CLASS,
                1,
            ));
        self.advertisement.capabilities.extend([
            conduit_std_offers::deterministic_speech_offer(),
            conduit_std_offers::audio_convert_pcm_profile_offer(),
        ]);
        self.advertisement
            .capabilities
            .extend(conduit_std_offers::spoken_mask_offers());
        self.advertisement.resources.sort();
        crate::normalize_capability_offers(&mut self.advertisement.capabilities)?;
        self.wav_artifact = Some(artifact);
        self.kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&self.advertisement)?;
        Ok(())
    }
}
