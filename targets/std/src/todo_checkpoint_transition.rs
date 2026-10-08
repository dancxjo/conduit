//! Idle transition between exact Todo checkpoint offers on one std Host Boot.
use crate::{
    composition, kernel_preparation::KernelResourceLedger, StdHost, StdHostComposition,
    StdHostConfig,
};
use conduit_core::{OfferGeneration, ResourceAccessMode, ResourceContentRequirement};
use std::path::Path;

impl StdHost {
    /// Replace one selected Todo checkpoint generation while retaining this
    /// Host Boot's Play and Sign cursors. Only the exact scoped reference Todo
    /// profile is eligible; other initialized equipment is refused unchanged.
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
            || self.playback.is_some()
            || self.wav_artifact.is_some()
            || self.speech_synthesis.is_some()
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
                conduit_std_offers::todo_checkpoint_offer(current)
            }
            ResourceAccessMode::ReadPublished => {
                conduit_std_offers::todo_checkpoint_read_offer(current)
            }
        }
        .map_err(str::to_string)?;
        let mut expected = composition::build_advertisement(
            config.clone(),
            StdHostComposition::reference(),
            None,
            None,
            None,
            false,
        );
        let old_resource = self
            .advertisement
            .resources
            .iter()
            .find(|offer| offer.pool_id.as_str() == "std/todo-checkpoint")
            .expect("selected resource exists above")
            .clone();
        expected.resources.push(old_resource);
        expected.capabilities.push(old_offer);
        expected.resources.sort();
        expected
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        if self.advertisement != expected {
            return Err("Todo checkpoint transition refuses other selected Host equipment".into());
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
        // Build before publication; on any refusal the old Host stays intact.
        let ledger = KernelResourceLedger::new(&next.advertisement)?;
        self.advertisement = next.advertisement;
        self.kernel_resources = ledger;
        self.todo_checkpoint_root = next.todo_checkpoint_root;
        Ok(())
    }
}
