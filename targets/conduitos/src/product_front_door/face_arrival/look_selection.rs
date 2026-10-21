//! Local preference keys select checked roots; they never dispatch a Body action.
use super::*;
use conduit_face::{ComponentVisualSelection, CrecheComponentRecipe, VisualDensity, VisualPalette};
use conduit_human::KeyTransition;
use sha2::{Digest, Sha256};

impl FaceArrival {
    pub(super) fn select_local_look(
        &mut self,
        event: KeyEvent,
        door: &FrontDoor,
        display: &mut impl PixelTarget,
    ) -> Result<bool, &'static str> {
        if self.owner_face.is_some()
            || self.preparation.owner_host.is_some()
            || !door.creche_open()
            || event.transition() != KeyTransition::Pressed
            || event.modifiers_after().bits() != 0
            || !matches!(event.usage(), 63 | 64)
        {
            return Ok(false);
        }
        let current = &self
            .mask
            .visual_expression()
            .ok_or("creche-look-absent")?
            .expression;
        let mut selection = ComponentVisualSelection {
            density: current.density,
            palette: current.palette,
        };
        if event.usage() == 63 {
            selection.density = match selection.density {
                VisualDensity::Compact => VisualDensity::Spacious,
                VisualDensity::Spacious => VisualDensity::Compact,
            };
        } else {
            selection.palette = match selection.palette {
                VisualPalette::Phosphor => VisualPalette::Editorial,
                VisualPalette::Editorial => VisualPalette::Phosphor,
            };
        }
        let face = door
            .creche_face(&self.basis)
            .map_err(|error| error.as_str())?;
        let old_show = self.mask.show().cloned().ok_or("creche-look-show-absent")?;
        if old_show.show.face_id != face.identity || old_show.show.face_revision != face.revision {
            return Err("creche-look-face-stale");
        }
        let context = &self.preparation;
        let mut replacement = NativeFaceMask::prepare_creche_selected(
            context.host_id.clone(),
            context.boot_id.clone(),
            context.generation,
            &context.build_id,
            context.display_base_id.clone(),
            "conduitos/creche/face",
            &context.provider,
            CrecheComponentRecipe::ResponsiveColumns,
            selection,
        )
        .map_err(|error| error.as_str())?;
        replacement
            .set_visual_user_constraints(
                context.forced_colors,
                true,
                context.minimum_text_scale_percent,
            )
            .map_err(|error| error.as_str())?;
        // Preparation finishes before the old Show retires. Unsubmitted local
        // drafts/focus move only across the identical Face, never across Shows.
        replacement
            .preserve_encounter_from(&mut self.mask)
            .map_err(|error| error.as_str())?;
        self.mask.suspend().map_err(|error| error.as_str())?;
        self.mask = replacement;
        let composition = self.present(face.clone(), display)?;
        let configuration = self
            .mask
            .component_configuration()
            .ok_or("creche-look-configuration-absent")?;
        let fresh_show = self.mask.show().ok_or("creche-look-show-absent")?;
        let evidence = serde_json::json!({
            "schema":"conduit.conduitos/component-realization@1",
            "selection_entrance":"local-creche",
            "face_id":face.identity,
            "face_revision":face.revision,
            "configuration":{
                "component":configuration.component,
                "invocation_path":configuration.invocation_path,
                "root":selection.creche_root(CrecheComponentRecipe::ResponsiveColumns),
                "recipe_canonical_sha256":alloc::format!("{:x}", Sha256::digest(configuration.recipe.canonical_value())),
                "recipe_canonical_bytes":configuration.recipe.canonical_value().len(),
                "implementation_id":configuration.implementation_id,
                "execution_profile_id":configuration.execution_profile_id,
                "placement_id":configuration.placement_id,
            },
            "render":{"commands":composition.display.commands,
                "pixels_written":composition.display.pixels_written},
            "expression":self.mask.visual_expression(),
            "fallback":self.mask.component_fallback(),
            "previous_show_id":old_show.show_id,
            "show_id":fresh_show.show_id,
            "mask_plan_id":fresh_show.show.plan_id,
            "active_play_id":fresh_show.show.active_play_id,
            "manifestation_id":fresh_show.show.manifestation_id,
            "host_id":fresh_show.show.host_id,
            "boot_id":fresh_show.show.boot_id,
            "offer_generation":fresh_show.show.offer_generation,
            "owner_show_acknowledged":false,
            "local_scanout_acknowledged":true,
        });
        super::super::native_action_evidence::emit_with_limit(
            b"CONDUIT_NATIVE_COMPONENT_REALIZATION ",
            &evidence,
            65_536,
        )?;
        self.record_focus()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
