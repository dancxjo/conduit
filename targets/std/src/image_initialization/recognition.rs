//! Initial custody of an already verified Whisper clip provider. No capture or
//! recognition process starts here; execution still needs its ordinary admission.
use super::*;
use crate::hosted_speech_recognition::WhisperSpeechAdapter;
pub(crate) struct InitialWhisper {
    pub(super) entry: BaseProviderEntry,
    pub(super) adapter: WhisperSpeechAdapter,
}
impl InitialWhisper {
    pub(crate) fn prepare(
        config: &StdHostConfig,
        image: &HostImage,
        checked: &conduit_host_make::CheckedHostConfiguration,
        adapter: WhisperSpeechAdapter,
    ) -> Result<Self, String> {
        let implementation = conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION;
        let bases = image
            .manifest
            .base_selections
            .iter()
            .filter(|base| base.kind == "speech/recognizer")
            .collect::<Vec<_>>();
        let budgets = image
            .manifest
            .resource_budgets
            .iter()
            .filter(|budget| budget.class == conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS)
            .collect::<Vec<_>>();
        if image.manifest.source_configuration_id.as_deref() != Some(checked.configuration_id())
            || bases.len() != 1
            || bases[0].driver != implementation
            || !checked
                .configuration()
                .bases
                .iter()
                .any(|base| base.kind == "speech/recognizer" && base.initialization.is_some())
            || budgets.len() != 1
            || budgets[0].slots != 1
            || budgets[0].bytes == 0
            || adapter.limits().maximum_audio_bytes < conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
            || adapter.limits().maximum_text_bytes
                < conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16
        {
            return Err(
                "selected IMAGE differs from its actual initialized Whisper clip provider/budget"
                    .into(),
            );
        }
        let initialization = checked
            .configuration()
            .bases
            .iter()
            .find(|base| base.kind == "speech/recognizer")
            .and_then(|base| base.initialization.as_ref())
            .ok_or("selected recognizer initialization missing")?;
        let text = |key: &str| {
            initialization
                .values
                .get(key)
                .and_then(serde_json::Value::as_str)
        };
        let count = |key: &str| {
            initialization
                .values
                .get(key)
                .and_then(serde_json::Value::as_u64)
        };
        let discovery = adapter.discovery();
        let offer = adapter.clip_offer();
        use sha2::{Digest, Sha256};
        let coverage = offer
            .realization_properties
            .first()
            .map(|property| format!("{:x}", Sha256::digest(property.canonical_value())));
        if text("executable_sha256") != Some(discovery.executable_sha256.as_str())
            || text("model_sha256") != Some(discovery.model_sha256.as_str())
            || count("model_bytes") != Some(discovery.model_bytes)
            || count("threads") != Some(u64::from(adapter.limits().threads))
            || count("timeout_milliseconds").map(u128::from)
                != Some(adapter.limits().timeout.as_millis())
            || coverage.as_deref() != text("language_coverage_sha256")
        {
            return Err("initialized recognizer artifacts/limits/Language coverage differ from checked Source".into());
        }
        Ok(Self {
            entry: BaseProviderEntry {
                base_id: bases[0].id.as_str().into(),
                provider_instance_id: adapter.provider_identity().into(),
                provider_generation: config.offer_generation.0,
                implementation_id: implementation.into(),
                mechanism_family: "speech/recognizer".into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    &budgets[0].id,
                    conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
                    1,
                )],
            },
            adapter,
        })
    }
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::*;
    use crate::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits};
    use conduit_host_make::*;
    use conduit_plot::rust_binding::NativeRustBinding;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    #[test]
    fn actual_initialized_recognizer_enters_original_image_registry_without_capture_or_spawn() {
        let root = std::env::temp_dir().join(format!(
            "conduit-initial-whisper-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let executable = root.join("whisper-cli");
        let model = root.join("model.bin");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\ntouch '{}'\nexit 99\n",
                root.join("spawned").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&model, b"finite fixture model").unwrap();
        let discovery = WhisperDiscovery::inspect(&executable, &model).unwrap();
        let coverage = crate::hosted_language::tests::fixture_coverage(
            &discovery.provider_identity(),
            "en",
            "language/english",
        );
        let encoded = coverage.clone().encode().unwrap();
        fs::write(root.join("coverage.native"), &encoded).unwrap();
        use sha2::{Digest, Sha256};
        let coverage_sha = format!("{:x}", Sha256::digest(&encoded));
        let discovery = discovery.declare_language_coverage(coverage).unwrap();
        let mut configuration = parse_host_configuration_conduit(
            &include_str!("../../profiles/linux-computer.host.conduit")
                .replace("clock/monotonic", "timer/monotonic"),
        )
        .unwrap();
        configuration.bases.push(ConfigurationBase {kind:"speech/recognizer".into(),implementation:Some(conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION.into()),implementations:vec![],initialization:Some(serde_json::from_value(serde_json::json!({"executable":executable,"executable_sha256":discovery.executable_sha256,"model":model,"model_sha256":discovery.model_sha256,"model_bytes":discovery.model_bytes,"language_coverage":root.join("coverage.native"),"language_coverage_sha256":coverage_sha,"threads":1,"timeout_milliseconds":2000})).unwrap())});
        for (kind, implementation) in [
            ("time/tick", conduit_std_offers::TICK_IMPLEMENTATION),
            (
                "speech/recognize-clip",
                conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION,
            ),
        ] {
            configuration.capabilities.push(CapabilityIntent {
                kind: kind.into(),
                pinned_implementation: Some(implementation.into()),
            });
        }
        configuration.resources.extend([
            ResourceBudget {
                id: "timer/main".into(),
                class: TIMER_RESOURCE_CLASS.into(),
                slots: 1,
                bytes: 512,
            },
            ResourceBudget {
                id: "speech/whisper".into(),
                class: conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS.into(),
                slots: 1,
                bytes: 1,
            },
        ]);
        let catalog = conduit_workspace_make::catalog();
        let packages = conduit_workspace_make::package_set();
        let checked = check_host_configuration(configuration, &catalog, &packages).unwrap();
        let image = build_default_host_image(
            checked.profile().clone(),
            &catalog,
            &packages,
            &BuildInputs {
                source_identity: "fixture/initial-recognizer".into(),
                toolchain_available: true,
            },
        )
        .unwrap()
        .0;
        let config = StdHostConfig {
            host_id: "host/recognizer".into(),
            boot_id: "boot/recognizer".into(),
            offer_generation: OfferGeneration(1),
        };
        let adapter = || {
            discovery
                .clone()
                .initialize(WhisperLimits::clip_recognition(1, Duration::from_secs(2)))
                .unwrap()
        };
        let wrong = adapter();
        // A different actual finite provider limit must not borrow checked pins.
        let changed = discovery
            .clone()
            .initialize(WhisperLimits::clip_recognition(2, Duration::from_secs(2)))
            .unwrap();
        assert!(InitialWhisper::prepare(&config, &image, &checked, changed).is_err());
        let initial = InitialWhisper::prepare(&config, &image, &checked, wrong).unwrap();
        let identity = initial.adapter.provider_identity();
        let mut host = StdHost::from_selected_initial_bases(
            config,
            &image,
            &catalog,
            InitialBases {
                recognition: Some(initial),
                ..InitialBases::default()
            },
        )
        .unwrap();
        assert_eq!(host.advertisement.offer_generation, OfferGeneration(1));
        assert_eq!(host.base_registry.entries().len(), 2);
        assert_eq!(
            host.speech_recognition
                .as_ref()
                .unwrap()
                .provider_identity(),
            identity
        );
        assert!(host.microphone.is_none());
        assert_eq!(host.advertisement.capabilities.len(), 2);
        assert!(host
            .attach_whisper_clip_recognizer(adapter())
            .unwrap_err()
            .contains("fresh checked construction"));
        assert!(!root.join("spawned").exists());
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }
}
