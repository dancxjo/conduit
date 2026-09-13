use super::AlsaMicrophoneAdapter;

impl crate::StdHost {
    pub fn new_with_microphone(
        config: crate::StdHostConfig,
        composition: crate::StdHostComposition,
        adapter: AlsaMicrophoneAdapter,
    ) -> Result<Self, String> {
        let mut host = Self::new_with_composition(config, composition);
        let mut advertisement = host.advertisement.clone();
        advertisement.resources.push(conduit_core::resource_offer(
            adapter.resource_pool_id().as_str(),
            conduit_std_offers::MICROPHONE_CAPTURE_RESOURCE_CLASS,
            1,
        ));
        advertisement
            .capabilities
            .push(conduit_std_offers::microphone_clip_offer());
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        host.advertisement = advertisement;
        host.microphone = Some(adapter);
        host.kernel_resources = kernel_resources;
        Ok(host)
    }

    pub fn attach_whisper_clip_recognizer(
        &mut self,
        adapter: crate::hosted_speech_recognition::WhisperSpeechAdapter,
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
        let mut advertisement = self.advertisement.clone();
        advertisement.resources.push(conduit_core::resource_offer(
            "std/whisper-process",
            conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
            1,
        ));
        advertisement
            .capabilities
            .push(conduit_std_offers::whisper_clip_speech_offer());
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.speech_recognition = Some(adapter);
        self.kernel_resources = kernel_resources;
        Ok(())
    }

    pub fn microphone_authority_grant(
        &self,
        grant_id: &str,
    ) -> Result<conduit_core::AuthorityGrant, String> {
        if self.microphone.is_none() {
            return Err("std Host has no selected microphone resource".into());
        }
        let capability = self
            .advertisement
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION
            })
            .ok_or_else(|| "selected microphone capability is not advertised".to_string())?;
        let requirement = capability
            .authority_requirements
            .first()
            .ok_or_else(|| "microphone capability has no authority contract".to_string())?;
        Ok(conduit_core::AuthorityGrant {
            grant_id: conduit_core::AuthorityGrantId::from(grant_id),
            contract_id: requirement.contract_id.clone(),
            host_operation_contract_id: requirement.host_operation_contract_id.clone(),
            subject_kind: requirement.subject_kind.clone(),
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            capability_id: capability.capability_id.clone(),
        })
    }
}
