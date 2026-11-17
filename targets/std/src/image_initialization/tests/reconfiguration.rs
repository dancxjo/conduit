//! Valid reference setup must not replace a selected IMAGE's admission ledger.
use super::*;

fn refuses_without_reconfiguration(
    selected: &mut StdHost,
    operation: impl FnOnce(&mut StdHost) -> Result<(), String>,
) {
    let advertisement = selected.advertisement.clone();
    let providers = selected.base_registry.entries().to_vec();
    let identity = selected.image_identity().cloned();
    // The private ledger's Debug includes IMAGE bounds, pool usage and retirement
    // reservations without adding a production inspection API.
    let ledger = format!("{:?}", selected.kernel_resources);
    assert!(selected.kernel_resources.is_idle());
    let planned = plan(selected);
    let before = selected
        .kernel_resources
        .prepare_and_reserve_plans(&selected.advertisement, &[(&planned, false)])
        .unwrap_err();
    assert!(before.contains("complete ordinary managed-memory accounting"));
    assert_eq!(format!("{:?}", selected.kernel_resources), ledger);

    let error = operation(selected).unwrap_err();
    assert!(error.contains("fresh checked construction"), "{error}");
    assert_eq!(selected.advertisement, advertisement);
    assert_eq!(selected.base_registry.entries(), providers);
    assert_eq!(selected.image_identity(), identity.as_ref());
    assert_eq!(format!("{:?}", selected.kernel_resources), ledger);
    let after = selected
        .kernel_resources
        .prepare_and_reserve_plans(&selected.advertisement, &[(&planned, false)])
        .unwrap_err();
    assert_eq!(after, before);
    assert_eq!(format!("{:?}", selected.kernel_resources), ledger);
    assert!(selected.kernel_resources.is_idle());
}

#[test]
fn valid_finite_sampling_cannot_replace_selected_image_bounds() {
    let value = CheckedValueContract::new(
        kind_id("value/bytes"),
        conduit_std_offers::CURRENT_SAMPLE_FINITE_MAXIMUM_BYTES,
        vec![],
    )
    .unwrap();
    let trigger = CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
    let expected = conduit_std_offers::current_sample_finite_offer(&value, &trigger).unwrap();
    let image = image(SOURCE);
    let mut selected = host(&image, "boot/sampling").unwrap();
    refuses_without_reconfiguration(&mut selected, |host| {
        host.install_finite_current_sampling(&value, &trigger)
    });

    let mut reference = StdHost::new();
    reference
        .install_finite_current_sampling(&value, &trigger)
        .unwrap();
    assert!(reference.advertisement.capabilities.contains(&expected));
    assert!(reference.image_identity().is_none());
    assert!(reference.kernel_resources.is_idle());
}

#[test]
fn valid_boolean_filter_cannot_replace_selected_image_bounds() {
    let input = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let predicate =
        StructuredInfoValue::leaf(input.clone(), InfoBool::TRUE.encode().to_vec()).unwrap();
    let program = conduit_plot::PortableExpressionProgram::from_checked_value(input, &predicate);
    let temporal = PortTemporal::Flow { closes: true };
    let expected = conduit_std_offers::pure_filter_std_offer(&program, temporal).unwrap();
    let image = image(SOURCE);
    let mut selected = host(&image, "boot/filter").unwrap();
    refuses_without_reconfiguration(&mut selected, |host| {
        host.install_pure_filter(&program, temporal)
    });

    let mut reference = StdHost::new();
    reference.install_pure_filter(&program, temporal).unwrap();
    assert!(reference.advertisement.capabilities.contains(&expected));
    assert!(reference.image_identity().is_none());
    assert!(reference.kernel_resources.is_idle());
}

#[cfg(all(feature = "local-model-proof", unix))]
mod whisper {
    use super::*;
    use crate::hosted_speech_recognition::{WhisperDiscovery, WhisperLimits, WhisperSpeechAdapter};
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

    struct Artifacts(PathBuf);
    impl Artifacts {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "conduit-image-reconfiguration-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let executable = root.join("whisper-cli");
            // Initialization hashes real files; no inference process is launched.
            fs::write(&executable, "#!/bin/sh\nexit 97\n").unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(root.join("model.bin"), b"bounded model").unwrap();
            Self(root)
        }

        fn adapter(&self) -> WhisperSpeechAdapter {
            let discovery =
                WhisperDiscovery::inspect(self.0.join("whisper-cli"), self.0.join("model.bin"))
                    .unwrap();
            let coverage = crate::hosted_language::tests::fixture_coverage(
                &discovery.provider_identity(),
                "en",
                "language/english",
            );
            discovery
                .declare_language_coverage(coverage)
                .unwrap()
                .initialize(WhisperLimits::clip_recognition(1, Duration::from_secs(2)))
                .unwrap()
        }
    }
    impl Drop for Artifacts {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn clip() -> Vec<u8> {
        let frame = conduit_audio::PcmFrameHeader::new(
            conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
            16_000,
            conduit_audio::PcmChannelLayout::Mono,
            2,
            1,
            0,
            false,
        )
        .unwrap()
        .encode_frame(&[1, 0, 2, 0])
        .unwrap();
        conduit_audio::encode_pcm_clip(&[&frame]).unwrap()
    }

    #[test]
    fn initialized_whisper_attachment_cannot_replace_selected_image_bounds() {
        let artifacts = Artifacts::new();
        let image = image(SOURCE);
        let mut selected = host(&image, "boot/whisper").unwrap();
        let adapter = artifacts.adapter();
        refuses_without_reconfiguration(&mut selected, |host| {
            host.attach_whisper_clip_proof(adapter, clip())
        });
        assert!(selected.speech_recognition.is_none());

        let adapter = artifacts.adapter();
        let expected_offer = adapter.clip_offer();
        let expected_provider = adapter.discovery().provider_identity();
        let mut reference = StdHost::new();
        reference
            .attach_whisper_clip_proof(adapter, clip())
            .unwrap();
        assert!(reference
            .advertisement
            .capabilities
            .contains(&expected_offer));
        let installed = reference.speech_recognition.as_ref().unwrap();
        assert_eq!(installed.discovery().provider_identity(), expected_provider);
        assert_eq!(installed.proof_pcm_clip(), Some(clip().as_slice()));
        assert!(reference.kernel_resources.is_idle());
    }

    #[test]
    fn proof_clip_source_requires_fresh_image_construction_before_adapter_lookup() {
        let artifacts = Artifacts::new();
        let image = image(SOURCE);
        let mut selected = host(&image, "boot/clip").unwrap();
        // The selected timer IMAGE has no recognizer. Its IMAGE fence must win
        // over the ordinary missing-recognizer refusal, without installing one.
        refuses_without_reconfiguration(&mut selected, |host| {
            host.attach_proof_pcm_clip_source(clip())
        });
        assert!(selected.speech_recognition.is_none());

        let mut reference = StdHost::new_with_whisper_clip_speech_recognition(
            crate::StdHostConfig {
                host_id: "fixture/whisper-reference".into(),
                boot_id: "boot/clip-reference".into(),
                offer_generation: OfferGeneration(1),
            },
            crate::StdHostComposition::reference(),
            artifacts.adapter(),
        )
        .unwrap();
        let provider = reference
            .speech_recognition
            .as_ref()
            .unwrap()
            .discovery()
            .provider_identity();
        reference.attach_proof_pcm_clip_source(clip()).unwrap();
        let installed = reference.speech_recognition.as_ref().unwrap();
        assert_eq!(installed.discovery().provider_identity(), provider);
        assert_eq!(installed.proof_pcm_clip(), Some(clip().as_slice()));
        assert!(reference.image_identity().is_none());
        assert!(reference.kernel_resources.is_idle());
    }
}
