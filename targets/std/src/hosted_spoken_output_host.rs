//! Host-owned speech and artifact attachment, before ordinary planning.
impl crate::StdHost {
    pub(crate) fn advance_retained_spoken_offer_generation(
        &mut self,
        next: conduit_core::OfferGeneration,
    ) -> Result<(), String> {
        let prior = self.advertisement.offer_generation;
        let boot = &self.advertisement.boot_id;
        if self
            .playback
            .as_ref()
            .is_some_and(|selected| selected.boot_id != *boot || selected.offer_generation != prior)
            || self.wav_artifact.as_ref().is_some_and(|artifact| {
                artifact.boot_id != *boot || artifact.offer_generation != prior
            })
        {
            return Err("retained spoken resource differs from current Host offer".into());
        }
        if let Some(adapter) = &mut self.speech_synthesis {
            adapter
                .advance_offer_generation(&self.advertisement.host_id, boot, prior, next)
                .map_err(|error| format!("retained speech provider: {error:?}"))?;
        }
        if let Some(selected) = &mut self.playback {
            selected.offer_generation = next;
        }
        if let Some(artifact) = &mut self.wav_artifact {
            artifact.offer_generation = next;
        }
        Ok(())
    }

    /// Current selected provider possession, independent of retained
    /// artifact capacity for a later Play.
    pub fn spoken_mask_provider_is_current(&self) -> bool {
        self.speech_synthesis
            .as_ref()
            .is_some_and(|provider| provider.provider_is_current())
            && self.kernel_resources.is_idle()
    }

    /// A new artifact Play additionally needs the selected retained pool to
    /// have a free slot or its fixed destination to be unpublished.
    pub fn spoken_mask_artifact_route_is_current(&self) -> bool {
        self.spoken_mask_provider_is_current()
            && self
                .wav_artifact
                .as_ref()
                .is_some_and(|artifact| artifact.is_unpublished())
    }

    /// An explicitly attached artifact route cannot stand in for selected
    /// speaker equipment that was lost or omitted by the caller.
    pub fn spoken_artifact_only_route_is_current(&self) -> bool {
        self.playback.is_none() && self.spoken_mask_artifact_route_is_current()
    }

    /// Check the preattached route without changing this Boot's offers.
    pub fn selected_spoken_equipment_matches(
        &self,
        playback: &crate::hosted_audio::HostedPlaybackSelection,
        provider_sha256: &str,
        realization_properties: &[conduit_core::StructuredConfigurationValue],
    ) -> bool {
        self.playback.as_ref() == Some(playback)
            && self.speech_synthesis.as_ref().is_some_and(|adapter| {
                adapter.provider_sha256() == provider_sha256
                    && adapter.offer().realization_properties == realization_properties
                    && adapter
                        .validate_host(
                            &self.advertisement.host_id,
                            &self.advertisement.boot_id,
                            self.advertisement.offer_generation,
                        )
                        .is_ok()
            })
    }

    /// Attach one explicitly selected speaker to this existing Host Boot.
    /// Discovery alone never authorizes playback; the caller must supply a
    /// separate grant to planning, and the adapter rechecks device availability
    /// when the selected Play starts.
    pub fn attach_selected_playback(
        &mut self,
        playback: crate::hosted_audio::HostedPlaybackSelection,
    ) -> Result<(), String> {
        if !self.kernel_resources.is_idle() {
            return Err("selected playback cannot replace active Host reservations".into());
        }
        if self.playback.is_some()
            || playback.boot_id != self.advertisement.boot_id
            || playback.offer_generation != self.advertisement.offer_generation
        {
            return Err("selected playback is stale or already attached".into());
        }
        let offer = conduit_std_offers::audio_play_alsa_hw_offer();
        let pool = playback.pool_id();
        if self
            .advertisement
            .capabilities
            .iter()
            .any(|existing| existing.capability_id == offer.capability_id)
            || self
                .advertisement
                .resources
                .iter()
                .any(|existing| existing.pool_id == pool)
        {
            return Err("selected playback offer or resource already exists".into());
        }
        let mut advertisement = self.advertisement.clone();
        advertisement.capabilities.push(offer);
        advertisement.resources.push(conduit_core::resource_offer(
            pool.as_str(),
            conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS,
            1,
        ));
        advertisement.resources.sort();
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let resources = crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = resources;
        self.playback = Some(playback);
        Ok(())
    }

    /// Attach deterministic proof PCM and an exact WAV artifact resource.
    /// This establishes an artifact effect, not intelligible speech or hearing.
    pub fn attach_deterministic_speech_and_wav_artifact(
        &mut self,
        artifact: crate::hosted_wav_artifact::WavArtifactSelection,
    ) -> Result<(), String> {
        self.attach_spoken_output(Some(artifact), None)
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
        self.attach_spoken_output(Some(artifact), Some(adapter))
    }

    /// Attach an explicitly selected speaker route to a real streaming voice.
    /// The selected playback resource must already belong to this Host Boot;
    /// discovering a device alone never grants authority to play it.
    pub fn attach_espeak_speech_for_selected_playback(
        &mut self,
        adapter: crate::hosted_speech_synthesis::EspeakSpeechAdapter,
    ) -> Result<(), String> {
        adapter
            .validate_host(
                &self.advertisement.host_id,
                &self.advertisement.boot_id,
                self.advertisement.offer_generation,
            )
            .map_err(|error| error.to_string())?;
        let playback = self
            .playback
            .as_ref()
            .ok_or_else(|| "spoken playback requires an exact selected resource".to_string())?;
        if playback.boot_id != self.advertisement.boot_id
            || playback.offer_generation != self.advertisement.offer_generation
        {
            return Err("selected spoken playback is stale for this Host Boot".into());
        }
        self.attach_spoken_output(None, Some(adapter))
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
        artifact: Option<crate::hosted_wav_artifact::WavArtifactSelection>,
        adapter: Option<crate::hosted_speech_synthesis::EspeakSpeechAdapter>,
    ) -> Result<(), String> {
        if !self.kernel_resources.is_idle() {
            return Err("spoken output cannot replace active Host reservations".into());
        }
        if self.wav_artifact.is_some()
            || self.speech_synthesis.is_some()
            || artifact.as_ref().is_some_and(|artifact| {
                artifact.boot_id != self.advertisement.boot_id
                    || artifact.offer_generation != self.advertisement.offer_generation
            })
        {
            return Err("spoken output is stale or already selected".into());
        }
        let mut advertisement = self.advertisement.clone();
        if let Some(artifact) = &artifact {
            advertisement.resources.push(conduit_core::resource_offer(
                artifact.pool_id().as_str(),
                conduit_std_offers::AUDIO_WAV_ARTIFACT_RESOURCE_CLASS,
                1,
            ));
        }
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
        if artifact.is_some() {
            advertisement
                .capabilities
                .push(conduit_std_offers::audio_write_wav_artifact_offer());
        }
        advertisement.resources.sort();
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let resources = crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = resources;
        self.wav_artifact = artifact;
        self.speech_synthesis = adapter;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        hosted_audio::{AlsaPlaybackObservation, HostedPlaybackSelection},
        StdHost,
    };
    use conduit_core::{BootId, OfferGeneration};

    fn selection(boot_id: BootId, generation: OfferGeneration) -> HostedPlaybackSelection {
        HostedPlaybackSelection::from_observation(
            AlsaPlaybackObservation {
                card_index: 1,
                card_id: "SELECTED".into(),
                card_name: "Selected speaker".into(),
                device: 0,
                device_name: "Playback".into(),
                base_identity: "selected-test".into(),
            },
            boot_id,
            generation,
        )
    }

    #[test]
    fn selected_speaker_attaches_to_existing_host_without_inventing_a_boot() {
        let mut host = StdHost::new();
        let before = host.advertisement().clone();
        assert!(host.playback_authority_grant("grant/selected").is_err());
        let selected = selection(before.boot_id.clone(), before.offer_generation);
        let pool = selected.pool_id();
        host.attach_selected_playback(selected.clone()).unwrap();
        let after = host.advertisement();
        assert_eq!(after.host_id, before.host_id);
        assert_eq!(after.boot_id, before.boot_id);
        assert_eq!(after.offer_generation, before.offer_generation);
        assert!(after
            .resources
            .iter()
            .any(|resource| resource.pool_id == pool));
        let grant = host.playback_authority_grant("grant/selected").unwrap();
        assert_eq!(grant.host_id, before.host_id);
        assert_eq!(grant.boot_id, before.boot_id);
        assert_eq!(grant.grant_id.as_str(), "grant/selected");
        let admitted = after.clone();
        assert!(host.attach_selected_playback(selected).is_err());
        assert_eq!(host.advertisement(), &admitted);
    }

    #[test]
    fn stale_speaker_never_changes_the_installed_host_offer() {
        let mut host = StdHost::new();
        let before = host.advertisement().clone();
        let stale_boot = selection(BootId::from("other-boot"), before.offer_generation);
        assert!(host.attach_selected_playback(stale_boot).is_err());
        let stale_generation = selection(
            before.boot_id.clone(),
            OfferGeneration(before.offer_generation.0 + 1),
        );
        assert!(host.attach_selected_playback(stale_generation).is_err());
        assert_eq!(host.advertisement(), &before);
        assert!(host.playback_authority_grant("grant/selected").is_err());
    }

    #[test]
    fn conflicting_resource_cannot_be_adopted_as_selected_playback() {
        let original = StdHost::new();
        let selected = selection(
            original.advertisement().boot_id.clone(),
            original.advertisement().offer_generation,
        );
        let mut conflicting = original.advertisement().clone();
        conflicting.resources.push(conduit_core::resource_offer(
            selected.pool_id().as_str(),
            "conduit.resource/unrelated@1",
            1,
        ));
        let mut host = StdHost::from_advertisement(conflicting.clone()).unwrap();
        assert!(host.attach_selected_playback(selected).is_err());
        assert_eq!(host.advertisement(), &conflicting);
        assert!(host.playback_authority_grant("grant/selected").is_err());
    }
}
