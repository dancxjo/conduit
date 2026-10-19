//! Canonical zero-Body browser encounter over CURRENT's actual BirthDraft.
//! Transport buffers and Mask-local state do not duplicate owner values.
use super::*;
use crate::owner_face_mask::local::LocalBrowserMask;
use conduit_birth_plot::HostOwnedBirthFaceBasis;
use conduit_core::{BootId, HostId};

#[cfg(test)]
mod abi_tests;
mod control_bindings;
#[cfg(test)]
mod tests;
mod transport;

pub(super) struct Encounter {
    basis: HostOwnedBirthFaceBasis,
    mask: LocalBrowserMask,
    delivered: Option<conduit_face::FaceInteraction>,
    bindings: control_bindings::ControlBindings,
}

impl Encounter {
    fn snapshot(&self) -> serde_json::Value {
        let mut snapshot = self.mask.snapshot();
        snapshot["control_bindings"] = serde_json::json!(self.bindings);
        snapshot
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    generation: u32,
    host_id: HostId,
    boot_id: BootId,
    component: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Acknowledge {
    generation: u32,
    show_id: String,
    face_id: String,
    face_revision: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submit {
    generation: u32,
    interaction: serde_json::Value,
}

impl CurrentDraft {
    fn require_generation(&self, generation: u32) -> Result<(), String> {
        if self.generation != generation || self.draft.is_none() {
            return Err("Stale or absent Crèche draft".into());
        }
        Ok(())
    }
    pub(super) fn close_local_face(&mut self) -> Result<(), String> {
        if let Some(encounter) = self.local_face.as_mut() {
            encounter.mask.cancel()?;
        }
        self.local_face = None;
        Ok(())
    }
    fn prepare_local_face(&mut self, request: Prepare) -> Result<(), String> {
        self.require_generation(request.generation)?;
        let sequence = self
            .mask_sequence
            .checked_add(1)
            .ok_or("Mask sequence exhausted")?;
        let basis = HostOwnedBirthFaceBasis {
            host_id: request.host_id,
            boot_id: request.boot_id,
            encounter_id: format!("browser-creche-{}", self.generation),
        };
        let face = self
            .draft
            .as_ref()
            .ok_or("Crèche absent")?
            .host_owned_face(&basis)
            .map_err(|error| format!("Crèche source Face: {error:?}"))?;
        let bindings = control_bindings::ControlBindings::prepare(&face);
        let (source, root) = if request.component {
            (
                conduit_face::CRECHE_COMPONENT_MASK_SOURCE,
                "creche-mask-single",
            )
        } else {
            (
                crate::installed_browser::dom_mask::MASK_SOURCE,
                "browser-graphical",
            )
        };
        let mask = LocalBrowserMask::prepare(
            basis.host_id.clone(),
            basis.boot_id.clone(),
            face,
            source,
            root,
            sequence,
        )?;
        let candidate = Encounter {
            basis,
            mask,
            delivered: None,
            bindings,
        };
        let value = serde_json::json!({"schema":"conduit.browser/creche-local-face@1",
            "generation":self.generation, "owner_revision":self.draft.as_ref().ok_or("Crèche absent")?.revision(), "birth_requested":false, "encounter":candidate.snapshot()});
        transport::admit(&value)?;
        transport::write(&value)?;
        if let Some(previous) = self.local_face.as_mut() {
            previous.mask.replace()?;
        }
        self.local_face = Some(candidate);
        self.mask_sequence = sequence;
        Ok(())
    }
    fn local_snapshot(&self, birth_requested: bool) -> Result<(), String> {
        let encounter = self.local_face.as_ref().ok_or("Crèche local Mask absent")?;
        transport::write(
            &serde_json::json!({"schema":"conduit.browser/creche-local-face@1",
            "generation":self.generation, "owner_revision":self.draft.as_ref().ok_or("Crèche absent")?.revision(), "birth_requested":birth_requested,
            "encounter":encounter.snapshot(),"delivered_interaction":encounter.delivered}),
        )
    }
    fn acknowledge_local_face(&mut self, request: Acknowledge) -> Result<(), String> {
        self.require_generation(request.generation)?;
        self.local_face
            .as_mut()
            .ok_or("Crèche local Mask absent")?
            .mask
            .acknowledge(&request.show_id, &request.face_id, &request.face_revision)?;
        self.local_snapshot(false)
    }
    fn submit_local_face(&mut self, request: Submit) -> Result<(), String> {
        self.require_generation(request.generation)?;
        let encounter = self.local_face.as_mut().ok_or("Crèche local Mask absent")?;
        let (show, interaction) = encounter.mask.interact(request.interaction)?;
        let outcome = self
            .draft
            .as_mut()
            .ok_or("Crèche absent")?
            .apply_host_owned_face_interaction(&encounter.basis, &show, &interaction)
            .map_err(|error| format!("Crèche owner refused: {error:?}"))?;
        encounter.delivered = Some(interaction);
        self.local_snapshot(matches!(outcome, BirthActionOutcome::Birth(_)))
    }
}
