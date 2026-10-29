use crate::hosted_speech_recognition::{
    tests::{discover, fixture},
    WhisperLimits,
};
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

#[test]
fn recognition_microphone_and_checked_speaker_attach_in_every_order() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let (root, executable, model) = fixture("#!/bin/sh\nexit 99\n");
        let whisper = discover(&executable, &model)
            .initialize(WhisperLimits::clip_recognition(1, Duration::from_secs(2)))
            .unwrap();
        let identity = whisper.provider_identity();
        let mic_exe = root.join("arecord");
        fs::write(&mic_exe,"#!/bin/sh\nif [ \"$2\" = -l ]; then printf 'card 999: Fixture [Fixture], device 0: Capture [Capture]\\n'; exit 0; fi\nexit 97\n").unwrap();
        fs::set_permissions(&mic_exe, fs::Permissions::from_mode(0o700)).unwrap();
        let discovery =
            crate::hosted_microphone::AlsaMicrophoneDiscovery::inspect(&mic_exe).unwrap();
        let observation = discovery.observations[0].clone();
        let mic = discovery
            .initialize(
                &observation,
                crate::hosted_microphone::MicrophoneLimits {
                    capture_milliseconds: 1,
                    timeout: Duration::from_secs(2),
                },
            )
            .unwrap();
        let mut host = crate::StdHost::new_with_config(crate::StdHostConfig {
            host_id: "host/recognition-order".into(),
            boot_id: "boot/recognition-order".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        });
        let mut mic = Some(mic);
        let mut whisper = Some(whisper);
        for step in order {
            match step {
                0 => host.attach_microphone(mic.take().unwrap()).unwrap(),
                1 => host
                    .attach_whisper_clip_recognizer(whisper.take().unwrap())
                    .unwrap(),
                _ => {
                    let playback = crate::hosted_audio::HostedPlaybackSelection::from_observation(
                        crate::hosted_audio::AlsaPlaybackObservation {
                            card_index: 999,
                            card_id: "Fixture".into(),
                            card_name: "Fixture".into(),
                            device: 0,
                            device_name: "Output".into(),
                            base_identity: "fixture/output".into(),
                        },
                        host.advertisement().boot_id.clone(),
                        host.advertisement().offer_generation,
                    )
                    .with_executable_sha256("ab".repeat(32))
                    .unwrap();
                    host.attach_selected_playback(playback).unwrap();
                }
            }
        }
        let ad = host.advertisement();
        assert_eq!(ad.offer_generation, conduit_core::OfferGeneration(3));
        assert_eq!(ad.bases.len(), 3);
        let recognized = ad
            .bases
            .iter()
            .find(|b| {
                b.implementation_id.as_str()
                    == conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION
            })
            .unwrap();
        assert_eq!(recognized.lifecycle, conduit_core::BaseLifecycle::Ready);
        assert_eq!(recognized.provider_instance_id.as_str(), identity);
        assert_eq!(
            host.playback.as_ref().unwrap().offer_generation,
            ad.offer_generation
        );
        let pools: std::collections::BTreeSet<_> =
            ad.resources.iter().map(|p| &p.pool_id).collect();
        assert_eq!(pools.len(), ad.resources.len());
        let caps: std::collections::BTreeSet<_> =
            ad.capabilities.iter().map(|c| &c.capability_id).collect();
        assert_eq!(caps.len(), ad.capabilities.len());
        drop(host);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn recognition_attachment_refuses_active_reservations_and_duplicates_without_mutation() {
    let (root, executable, model) = fixture("#!/bin/sh\nexit 99\n");
    let adapter = || {
        discover(&executable, &model)
            .initialize(WhisperLimits::clip_recognition(1, Duration::from_secs(2)))
            .unwrap()
    };
    let mut host = crate::StdHost::new();
    let before = host.advertisement().clone();
    let cap = before
        .capabilities
        .iter()
        .find(|c| c.resource_requirements.is_empty())
        .unwrap();
    let reserved = host
        .kernel_resources
        .reserve_pool_member(
            &before,
            "plan/recognition-reservation-fixture".into(),
            &cap.capability_id,
            &[],
        )
        .unwrap();
    assert!(host.attach_whisper_clip_recognizer(adapter()).is_err());
    assert_eq!(host.advertisement(), &before);
    host.kernel_resources.release(reserved).unwrap();
    host.attach_whisper_clip_recognizer(adapter()).unwrap();
    let attached = host.advertisement().clone();
    assert!(host.attach_whisper_clip_recognizer(adapter()).is_err());
    assert_eq!(host.advertisement(), &attached);
    drop(host);
    fs::remove_dir_all(root).unwrap();
}
