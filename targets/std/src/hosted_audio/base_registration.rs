//! Non-authorizing provenance for an explicitly checked playback provider.
use super::HostedPlaybackSelection;
use conduit_core::{BaseEnforcementClass, BaseLifecycle, BaseProviderEntry};
use std::fmt::Write;

impl HostedPlaybackSelection {
    pub(crate) fn checked_base_entry(&self) -> Option<BaseProviderEntry> {
        let digest = self.executable_sha256?;
        let mut hex = String::with_capacity(64);
        for byte in digest {
            write!(&mut hex, "{byte:02x}").expect("write provider identity to String");
        }
        let endpoint = format!(
            "std/alsa-output/{}/card-{}/device-{}",
            self.observation.base_identity, self.observation.card_id, self.observation.device
        );
        Some(BaseProviderEntry {
            base_id: endpoint.clone().into(),
            provider_instance_id: format!("{endpoint}/{hex}").into(),
            provider_generation: self.offer_generation.0,
            implementation_id: conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION.into(),
            mechanism_family: "std.base/alsa-playback@1".into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![conduit_std_offers::audio_play_alsa_hw_offer()],
            resources: vec![conduit_core::resource_offer(
                self.pool_id().as_str(),
                conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS,
                1,
            )],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn checked_speaker_and_microphone_attach_in_either_order_without_duplicate_providers() {
        use crate::hosted_microphone::{AlsaMicrophoneDiscovery, MicrophoneLimits};
        use std::{os::unix::fs::PermissionsExt, time::Duration};
        for microphone_first in [true, false] {
            let root = std::env::temp_dir().join(format!(
                "conduit-duplex-base-{}-{microphone_first}",
                std::process::id()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let executable = root.join("arecord");
            std::fs::write(&executable,"#!/bin/sh\nif [ \"$2\" = -l ]; then printf 'card 999: Fixture [Fixture], device 0: Capture [Capture]\\n'; exit 0; fi\nexit 97\n").unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            let discovery = AlsaMicrophoneDiscovery::inspect(&executable).unwrap();
            let observation = discovery.observations[0].clone();
            let microphone = discovery
                .initialize(
                    &observation,
                    MicrophoneLimits {
                        capture_milliseconds: 1,
                        timeout: Duration::from_secs(2),
                    },
                )
                .unwrap();
            let config = crate::StdHostConfig {
                host_id: "host/duplex-fixture".into(),
                boot_id: "boot/duplex-fixture".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            };
            let mut host = crate::StdHost::new_with_config(config.clone());
            let mut microphone = Some(microphone);
            if microphone_first {
                host.attach_microphone(microphone.take().unwrap()).unwrap();
            }
            let playback = HostedPlaybackSelection::from_observation(
                crate::hosted_audio::AlsaPlaybackObservation {
                    card_index: observation.card_index,
                    card_id: observation.card_id,
                    card_name: observation.card_name,
                    device: observation.device,
                    device_name: "Output".into(),
                    base_identity: observation.base_identity,
                },
                config.boot_id,
                host.advertisement().offer_generation,
            )
            .with_executable_sha256("ab".repeat(32))
            .unwrap();
            host.attach_selected_playback(playback).unwrap();
            if !microphone_first {
                host.attach_microphone(microphone.take().unwrap()).unwrap();
            }
            let ad = host.advertisement();
            assert_eq!(ad.bases.len(), 2);
            let mut ids = ad
                .capabilities
                .iter()
                .map(|c| c.capability_id.clone())
                .collect::<Vec<_>>();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), ad.capabilities.len());
            let mut pools = ad
                .resources
                .iter()
                .map(|r| r.pool_id.clone())
                .collect::<Vec<_>>();
            pools.sort();
            pools.dedup();
            assert_eq!(pools.len(), ad.resources.len());
            assert_eq!(
                host.playback.as_ref().unwrap().offer_generation,
                ad.offer_generation
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn checked_playback_publishes_one_exact_base_without_granting_authority() {
        let selected = HostedPlaybackSelection::from_observation(
            crate::hosted_audio::AlsaPlaybackObservation {
                card_index: 999,
                card_id: "Fixture".into(),
                card_name: "Fixture".into(),
                device: 0,
                device_name: "Fixture".into(),
                base_identity: "fixture/speaker".into(),
            },
            "boot/base-fixture".into(),
            conduit_core::OfferGeneration(1),
        );
        assert!(selected.checked_base_entry().is_none());
        let selected = selected.with_executable_sha256("ab".repeat(32)).unwrap();
        let mut host = crate::StdHost::new_with_config(crate::StdHostConfig {
            host_id: "host/base-fixture".into(),
            boot_id: selected.boot_id.clone(),
            offer_generation: selected.offer_generation,
        });
        host.attach_selected_playback(selected.clone()).unwrap();
        let ad = host.advertisement();
        assert_eq!(ad.bases.len(), 1);
        let base = &ad.bases[0];
        assert_eq!(base.lifecycle, BaseLifecycle::Ready);
        assert!(base
            .provider_instance_id
            .as_str()
            .ends_with(&"ab".repeat(32)));
        assert_eq!(base.resource_pool_ids, vec![selected.pool_id()]);
        let offers = ad
            .capabilities
            .iter()
            .filter(|c| {
                c.implementation.implementation_id.as_str()
                    == conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION
            })
            .collect::<Vec<_>>();
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].authority_requirements.len(), 1);
        assert_eq!(base.capability_ids, vec![offers[0].capability_id.clone()]);
        let before = host.advertisement().clone();
        assert!(host.attach_selected_playback(selected).is_err());
        assert_eq!(host.advertisement(), &before);
    }
}
