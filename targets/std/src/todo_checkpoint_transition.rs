//! Idle transition between exact Todo checkpoint offers on one std Host Boot.
use crate::{kernel_preparation::KernelResourceLedger, StdHost, StdHostConfig};
use conduit_core::{OfferGeneration, ResourceAccessMode, ResourceContentRequirement};
use std::path::Path;

impl StdHost {
    /// Replace one selected Todo checkpoint generation while retaining this
    /// Host Boot's Play and Sign cursors. Only the exact scoped reference Todo
    /// profile is eligible; selected spoken output remains bound to this Boot.
    /// Other initialized equipment is refused unchanged.
    /// The caller must plan again against the returned advertisement.
    pub fn transition_todo_checkpoint_offer(
        &mut self,
        root: &Path,
        content: ResourceContentRequirement,
    ) -> Result<(), String> {
        if !self.kernel_resources.is_idle() {
            return Err("Todo checkpoint transition requires an idle Host".into());
        }
        let current = self
            .advertisement
            .resources
            .iter()
            .find(|offer| offer.pool_id.as_str() == "std/todo-checkpoint")
            .and_then(|offer| offer.content.as_ref())
            .ok_or("Host has no selected Todo checkpoint residence")?
            .contract
            .clone();
        if content.identity != current.identity {
            return Err("Todo checkpoint transition changed semantic resource identity".into());
        }
        if self.image_identity.is_some()
            || self.midi_input.is_some()
            || self.midi_output.is_some()
            || self.local_model.is_some()
            || self.speech_recognition.is_some()
            || self.microphone.is_some()
            || !self.base_registry.entries().is_empty()
            || self.vector_search.is_some()
            || self.calendar.is_some()
            || self.body_conversation_context.is_some()
            || self.vision.is_some()
            || {
                #[cfg(unix)]
                {
                    self.terminal_attachment.is_some()
                }
                #[cfg(not(unix))]
                {
                    false
                }
            }
        {
            return Err("Todo checkpoint transition refuses selected Host equipment".into());
        }
        let config = StdHostConfig {
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            offer_generation: self.advertisement.offer_generation,
        };
        let old_offer = match current.access {
            ResourceAccessMode::WriteCandidatePublish => {
                conduit_std_offers::todo_checkpoint_offer(current.clone())
            }
            ResourceAccessMode::ReadPublished => {
                conduit_std_offers::todo_checkpoint_read_offer(current.clone())
            }
        }
        .map_err(str::to_string)?;
        let old_resource = self
            .advertisement
            .resources
            .iter()
            .find(|offer| offer.pool_id.as_str() == "std/todo-checkpoint")
            .expect("selected resource exists above")
            .clone();
        if old_resource.class_id.as_str() != "resource/todo-checkpoint@1"
            || old_resource.content.as_ref().map(|value| &value.contract) != Some(&current)
            || !self.advertisement.capabilities.contains(&old_offer)
        {
            return Err("current Todo checkpoint offer differs from selected residence".into());
        }
        let prior_root = self
            .todo_checkpoint_root
            .as_ref()
            .ok_or("Todo checkpoint root is unbound")?;
        let next_generation = OfferGeneration(
            self.advertisement
                .offer_generation
                .0
                .checked_add(1)
                .ok_or("Todo checkpoint offer generation exhausted")?,
        );
        let next_config = StdHostConfig {
            offer_generation: next_generation,
            ..config
        };
        let next_offer = match content.access {
            ResourceAccessMode::WriteCandidatePublish => {
                conduit_std_offers::todo_checkpoint_offer(content.clone())
            }
            ResourceAccessMode::ReadPublished => {
                conduit_std_offers::todo_checkpoint_read_offer(content.clone())
            }
        }
        .map_err(str::to_string)?;
        let next = match content.access {
            ResourceAccessMode::WriteCandidatePublish => {
                Self::new_for_todo_checkpoint_once(next_config, root, content)?
            }
            ResourceAccessMode::ReadPublished => {
                Self::new_for_todo_checkpoint_read(next_config, root, content)?
            }
        };
        let next_root = next
            .todo_checkpoint_root
            .as_ref()
            .expect("constructor selects root");
        if next_root.path != prior_root.path {
            return Err("Todo checkpoint transition changed selected root".into());
        }
        #[cfg(unix)]
        if next_root.device != prior_root.device || next_root.inode != prior_root.inode {
            return Err("Todo checkpoint transition rebound selected root".into());
        }
        // Preserve every separately selected spoken offer/resource while
        // replacing only the exact checkpoint pair. The new pair comes from
        // the same reviewed constructor used at a fresh Boot.
        let mut advertisement = self.advertisement.clone();
        advertisement.offer_generation = next_generation;
        advertisement
            .resources
            .retain(|offer| offer.pool_id != old_resource.pool_id);
        advertisement
            .capabilities
            .retain(|offer| offer != &old_offer);
        if advertisement
            .capabilities
            .iter()
            .any(|offer| offer.capability_id == next_offer.capability_id)
        {
            return Err("next Todo checkpoint offer collides with selected equipment".into());
        }
        advertisement.resources.push(
            next.advertisement
                .resources
                .iter()
                .find(|offer| offer.pool_id == old_resource.pool_id)
                .ok_or("next Todo checkpoint residence is missing")?
                .clone(),
        );
        advertisement.capabilities.push(next_offer);
        advertisement.resources.sort();
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        let ledger = KernelResourceLedger::new(&advertisement)?;
        self.advance_retained_spoken_offer_generation(next_generation)?;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        self.todo_checkpoint_root = next.todo_checkpoint_root;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hosted_audio::{AlsaPlaybackObservation, HostedPlaybackSelection},
        hosted_wav_artifact::WavArtifactSelection,
    };
    use conduit_core::{
        kind_id, BootId, HostId, ResourceRetention, ResourceSemanticIdentity, ResourceSharing,
        ResourceVersionIdentity,
    };

    fn content(access: ResourceAccessMode, version: u8) -> ResourceContentRequirement {
        ResourceContentRequirement {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            version: ResourceVersionIdentity::from_digest([version; 32]),
            content_profile: kind_id("conduit.todo/checkpoint-envelope@1"),
            maximum_bytes: conduit_std_offers::TODO_CHECKPOINT_MAX_BYTES,
            maximum_items: 1,
            retention: ResourceRetention::ExternalDurable,
            sharing: ResourceSharing::SingleWriterPublished,
            access,
            generation_slots: 1,
            reader_leases: 1,
            publication_slots: if access == ResourceAccessMode::WriteCandidatePublish {
                1
            } else {
                0
            },
            sensitive: false,
        }
    }

    #[test]
    fn checkpoint_offer_transition_keeps_selected_speaker_and_artifact_on_same_boot() {
        let root = std::env::temp_dir().join(format!(
            "conduit-todo-spoken-transition-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let artifact_root = root.join("audio");
        std::fs::create_dir_all(&artifact_root).unwrap();
        let config = StdHostConfig {
            host_id: HostId::from("host/todo-spoken-transition"),
            boot_id: BootId::from("boot/todo-spoken-transition"),
            offer_generation: OfferGeneration(1),
        };
        let mut host = StdHost::new_for_todo_checkpoint_once(
            config.clone(),
            &root,
            content(ResourceAccessMode::WriteCandidatePublish, 1),
        )
        .unwrap();
        let speaker = HostedPlaybackSelection::from_observation(
            AlsaPlaybackObservation {
                card_index: 1,
                card_id: "SELECTED".into(),
                card_name: "Selected speaker".into(),
                device: 0,
                device_name: "Playback".into(),
                base_identity: "selected-test".into(),
            },
            config.boot_id.clone(),
            config.offer_generation,
        );
        host.attach_selected_playback(speaker).unwrap();
        host.attach_deterministic_speech_and_wav_artifact(
            WavArtifactSelection::per_play_root(
                &artifact_root,
                config.boot_id.clone(),
                config.offer_generation,
            )
            .unwrap(),
        )
        .unwrap();
        let spoken_resource_ids: Vec<_> = host
            .advertisement()
            .resources
            .iter()
            .filter(|offer| offer.pool_id.as_str() != "std/todo-checkpoint")
            .map(|offer| offer.pool_id.clone())
            .collect();
        let old_generation = host.advertisement().offer_generation;
        host.transition_todo_checkpoint_offer(&root, content(ResourceAccessMode::ReadPublished, 2))
            .unwrap();
        assert!(host.advertisement().offer_generation > old_generation);
        assert_eq!(host.advertisement().boot_id, config.boot_id);
        assert_eq!(
            host.playback.as_ref().unwrap().offer_generation,
            host.advertisement().offer_generation
        );
        assert_eq!(
            host.wav_artifact.as_ref().unwrap().offer_generation,
            host.advertisement().offer_generation
        );
        for pool in &spoken_resource_ids {
            assert!(host
                .advertisement()
                .resources
                .iter()
                .any(|offer| &offer.pool_id == pool));
        }
        let before_refusal = host.advertisement().clone();
        assert!(host
            .transition_todo_checkpoint_offer(
                &root,
                ResourceContentRequirement {
                    identity: ResourceSemanticIdentity::from_digest([9; 32]),
                    ..content(ResourceAccessMode::WriteCandidatePublish, 3)
                },
            )
            .is_err());
        assert_eq!(host.advertisement(), &before_refusal);
        host.transition_todo_checkpoint_offer(
            &root,
            content(ResourceAccessMode::WriteCandidatePublish, 3),
        )
        .unwrap();
        assert_eq!(host.advertisement().boot_id, config.boot_id);
        std::fs::remove_dir_all(root).unwrap();
    }
}
