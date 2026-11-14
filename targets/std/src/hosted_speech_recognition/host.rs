//! Transactional attachment of an initialized recognizer before publication.
impl crate::StdHost {
    pub fn attach_whisper_clip_recognizer(
        &mut self,
        adapter: super::WhisperSpeechAdapter,
    ) -> Result<(), String> {
        self.kernel_resources.require_reference_reconfiguration()?;
        if !self.kernel_resources.is_idle() {
            return Err("recognizer cannot replace active Host reservations".into());
        }
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
        let mut advertisement = self.advertisement.clone();
        let next = advertisement
            .offer_generation
            .0
            .checked_add(1)
            .ok_or("std Host offer generation exhausted")?;
        let base_id = conduit_core::HostBaseId::from("std/speech-recognition/whisper-clip");
        let mut registry = self.base_registry.clone();
        registry
            .register(conduit_core::BaseProviderEntry {
                base_id: base_id.clone(),
                provider_instance_id: adapter.provider_identity().into(),
                provider_generation: next,
                implementation_id: conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION.into(),
                mechanism_family: "std.base/whisper-process@1".into(),
                enforcement_class: conduit_core::BaseEnforcementClass::Cooperative,
                lifecycle: conduit_core::BaseLifecycle::Ready,
                capabilities: vec![adapter.clip_offer()],
                resources: vec![conduit_core::resource_offer(
                    "std/whisper-process",
                    conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
                    1,
                )],
            })
            .map_err(|e| format!("recognition Base registration: {e:?}"))?;
        registry
            .project_base_ready_into(&base_id, &mut advertisement)
            .map_err(|e| format!("recognition Base advertisement: {e:?}"))?;
        advertisement.offer_generation = conduit_core::OfferGeneration(next);
        advertisement.resources.sort();
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let resources = self.kernel_resources.replacement_for(&advertisement)?;
        self.advance_retained_spoken_offer_generation(advertisement.offer_generation)?;
        self.advertisement = advertisement;
        self.speech_recognition = Some(adapter);
        self.base_registry = registry;
        self.kernel_resources = resources;
        Ok(())
    }
}

#[cfg(all(test, unix))]
#[path = "host/tests.rs"]
mod tests;
