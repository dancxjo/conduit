//! One initialized std Host composition for the complete conversation path.
//!
//! Provider-specific discovery stays outside this module. The Host advertises
//! the Voice profile only after ASR, model, TTS, and current Body context have
//! all crossed their existing validation boundaries.

use crate::{
    hosted_local_model::HostedLocalModelAdapter, hosted_speech::PiperSpeechAdapter,
    hosted_speech_recognition::WhisperSpeechAdapter, StdHost, StdHostComposition, StdHostConfig,
};
use conduit_body::BodyConversationContext;

pub struct VoiceHostProviders {
    pub recognition: WhisperSpeechAdapter,
    pub model: Box<dyn HostedLocalModelAdapter>,
    pub synthesis: PiperSpeechAdapter,
}

impl StdHost {
    /// Construct one Host that can realize every provider Gear in the reviewed
    /// live-conversation Form. Partial provider sets use the existing narrower
    /// constructors and must not claim this complete profile.
    pub fn new_with_voice_conversation(
        config: StdHostConfig,
        composition: StdHostComposition,
        providers: VoiceHostProviders,
        context: &BodyConversationContext,
    ) -> Result<Self, String> {
        validate_recognition(&providers.recognition)?;
        validate_synthesis(&providers.synthesis)?;

        let mut host = Self::new_with_local_model(config, composition, providers.model)?;
        host.advertisement.resources.extend([
            conduit_core::resource_offer(
                "std/whisper-process",
                conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
                1,
            ),
            crate::hosted_speech::process_resource_offer(),
        ]);
        host.advertisement.capabilities.retain(|offer| {
            !matches!(
                offer.implementation.implementation_id.as_str(),
                conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION
                    | conduit_std_offers::DETERMINISTIC_STREAMING_SPEECH_IMPLEMENTATION
            )
        });
        host.advertisement
            .capabilities
            .push(conduit_std_offers::whisper_speech_offer());
        host.advertisement
            .capabilities
            .push(conduit_std_offers::piper_speech_offer());
        host.advertisement
            .capabilities
            .push(conduit_std_offers::piper_streaming_speech_offer());
        host.advertisement
            .capabilities
            .push(conduit_std_offers::audio_convert_pcm_profile_offer());
        host.advertisement.resources.sort();
        host.advertisement.resources.dedup();
        host.advertisement
            .capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        if host
            .advertisement
            .capabilities
            .windows(2)
            .any(|pair| pair[0].capability_id == pair[1].capability_id)
        {
            return Err("Voice Host composition produced duplicate capability identity".into());
        }
        host.speech_recognition = Some(providers.recognition);
        host.speech_synthesis = Some(providers.synthesis);
        host.install_body_conversation_context(context)?;
        if let Some(pair) = host
            .advertisement
            .capabilities
            .windows(2)
            .find(|pair| pair[0].capability_id == pair[1].capability_id)
        {
            return Err(format!(
                "Voice Host composition duplicated capability {}",
                pair[0].capability_id.as_str()
            ));
        }
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
        return Err("initialized Whisper adapter does not satisfy the Voice profile".into());
    }
    Ok(())
}

fn validate_synthesis(adapter: &PiperSpeechAdapter) -> Result<(), String> {
    if adapter.discovery().sample_rate_hz != 22_050
        || adapter.limits().maximum_text_bytes
            < conduit_tongues::MAXIMUM_SPEAKABLE_SEGMENT_BYTES as u32
        || adapter.limits().maximum_frames < conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2)
        || adapter.limits().maximum_blocks < conduit_std_offers::PIPER_MAXIMUM_BLOCKS
    {
        return Err("initialized Piper adapter does not satisfy the Voice profile".into());
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use conduit_ai::{
        LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelIdentity,
        LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits, LocalModelOffer,
    };
    use conduit_core::{BootId, HostId, OfferGeneration, PlannedGear};
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

    struct FakeModel {
        offer: LocalModelOffer,
    }

    impl HostedLocalModelAdapter for FakeModel {
        fn offer(&self) -> &LocalModelOffer {
            &self.offer
        }

        fn execute(
            &mut self,
            _placement: &PlannedGear,
            _input: &[u8],
            _output: &mut Vec<u8>,
        ) -> crate::hosted_local_model::LocalModelAdapterTerminal {
            crate::hosted_local_model::LocalModelAdapterTerminal::Failed
        }
    }

    struct ProviderFixture {
        root: PathBuf,
        whisper_executable: PathBuf,
        whisper_model: PathBuf,
        piper_executable: PathBuf,
        piper_model: PathBuf,
        piper_config: PathBuf,
    }

    impl ProviderFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "conduit-voice-host-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir(&root).unwrap();
            let whisper_executable = root.join("whisper-cli");
            let whisper_model = root.join("whisper.bin");
            let piper_executable = root.join("piper");
            let piper_model = root.join("voice.onnx");
            let piper_config = root.join("voice.onnx.json");
            fs::write(&whisper_executable, "#!/bin/sh\nexit 0\n").unwrap();
            fs::write(&piper_executable, "#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(&whisper_executable, fs::Permissions::from_mode(0o700)).unwrap();
            fs::set_permissions(&piper_executable, fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(&whisper_model, b"bounded whisper model").unwrap();
            fs::write(&piper_model, b"bounded piper model").unwrap();
            fs::write(&piper_config, br#"{"audio":{"sample_rate":22050}}"#).unwrap();
            Self {
                root,
                whisper_executable,
                whisper_model,
                piper_executable,
                piper_model,
                piper_config,
            }
        }

        fn providers(&self) -> VoiceHostProviders {
            let recognition = crate::hosted_speech_recognition::WhisperDiscovery::inspect(
                &self.whisper_executable,
                &self.whisper_model,
            )
            .unwrap()
            .initialize(crate::hosted_speech_recognition::WhisperLimits {
                maximum_audio_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
                maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
                threads: 1,
                timeout: Duration::from_secs(1),
            })
            .unwrap();
            let synthesis = crate::hosted_speech::PiperDiscovery::inspect(
                &self.piper_executable,
                &self.piper_model,
                &self.piper_config,
                None,
            )
            .unwrap()
            .initialize(crate::hosted_speech::PiperLimits {
                maximum_text_bytes: conduit_tongues::MAXIMUM_SPEAKABLE_SEGMENT_BYTES as u32,
                maximum_frames: conduit_tongues::MAXIMUM_PCM_BYTES.div_ceil(2),
                maximum_blocks: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
                timeout: Duration::from_secs(1),
            })
            .unwrap();
            VoiceHostProviders {
                recognition,
                model: Box::new(FakeModel {
                    offer: local_model_offer(),
                }),
                synthesis,
            }
        }
    }

    impl Drop for ProviderFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn complete_initialized_voice_profile_plans_the_unchanged_live_conversation() {
        let fixture = ProviderFixture::new();
        let host = StdHost::new_with_voice_conversation(
            StdHostConfig {
                host_id: HostId::from("host/voice-composition"),
                boot_id: BootId::from("boot/voice-composition"),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::minimal(),
            fixture.providers(),
            &context(),
        )
        .unwrap();
        let source = include_str!("../../../forms/live-conversation/main.conduit");
        let (startup, profile) = conversation_catalogs();
        let checked = conduit_form::check_syntax_document(
            &conduit_form::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        let expanded = conduit_form::expand_canonical_form_for_authoring(
            &checked,
            "live-conversation",
            &profile,
        )
        .unwrap()
        .expanded;
        conduit_planner::default_expanded_placements(&expanded, &[host.advertisement().clone()])
            .expect("one complete initialized Voice Host places every provider Gear");
    }

    fn conversation_catalogs() -> (conduit_form::StartupCatalog, conduit_form::ProfileCatalog) {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        startup
            .insert_value_kind_alias(
                "PcmFrames",
                conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
            )
            .unwrap();
        conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profile).unwrap();
        conduit_tongues::install_speech_catalogs(&mut startup, &mut profile).unwrap();
        conduit_chat::install_body_chat_catalog(&mut startup, &mut profile).unwrap();
        conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
        conduit_ai::install_model_text_catalog(&mut startup, &mut profile).unwrap();
        (startup, profile)
    }

    fn context() -> BodyConversationContext {
        use conduit_body::{Body, BodyConversationContextBasis};
        use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
        let body = Body::born(
            SourceDocumentId::from("source/voice-host-test"),
            CheckedFormId::from("checked/voice-host-test"),
            1,
            SignId::from("sign/voice-host-test/born"),
        )
        .unwrap();
        let (_, wake) = body
            .wake(2, SignId::from("sign/voice-host-test/wake"))
            .unwrap();
        BodyConversationContext {
            schema: "conduit.body/conversation-context-value@2".into(),
            display_name: "Orifinia Dawnheart".into(),
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            wake_sequence: 2,
            basis: BodyConversationContextBasis {
                body_id: wake.body_id,
                wake_id: wake.wake_id,
                wake_sequence: 2,
                revision: 1,
            },
            hosts: vec![],
            active_forms: vec!["form/live-conversation".into()],
            current_plan_id: None,
            active_play_id: None,
            lines: vec![],
            recent_sign_ids: vec![],
        }
    }

    fn local_model_offer() -> LocalModelOffer {
        LocalModelOffer {
            identity: LocalModelIdentity {
                runtime_name: "fixture".into(),
                runtime_version: "1".into(),
                runtime_build_identity: "fixture/build-1".into(),
                model_name: "fixture-model".into(),
                model_content_identity: "sha256-fixture".into(),
                architecture: "fixture".into(),
                parameter_profile: "tiny".into(),
                quantization: "exact".into(),
            },
            limits: LocalModelLimits {
                work: LlmWorkBounds {
                    maximum_input_bytes: 4_096,
                    maximum_context_items: 1,
                    maximum_output_bytes: 4_096,
                    maximum_work_units: 4_096,
                    maximum_history_items: 0,
                },
                model_bytes: 1,
                admitted_memory_mib: 1,
                compute: conduit_ai::LocalModelComputeNeed {
                    minimum_lanes: 1,
                    preferred_lanes: 1,
                    maximum_lanes: 1,
                    minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
                },
                maximum_in_flight: 1,
                maximum_queue_items: 4,
                maximum_queue_bytes: 16_384,
                cancellation_supported: true,
                cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
            },
            supported_profiles: vec![
                LocalModelKindProfile::Generate,
                LocalModelKindProfile::GenerateFlow,
            ],
            initialized: true,
            lifecycle: LocalModelLifecycleState::Ready,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
        }
    }
}
