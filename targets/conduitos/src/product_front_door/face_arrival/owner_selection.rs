//! Checked owner appearance requests retain the exact local encounter.
use super::*;
use conduit_face::{
    ComponentMaskLook, ComponentVisualSelection, TodoComponentRecipe, VisualDensity, VisualPalette,
};

impl FaceArrival {
    pub(super) fn replace_owner_mask(
        &mut self,
        fresh: &crate::native_guest_face::NativeGuestFace,
        display: &mut impl PixelTarget,
    ) -> Result<(), &'static str> {
        let context = &self.preparation;
        let mut replacement = NativeFaceMask::prepare_with_owner_route(
            context.host_id.clone(),
            context.boot_id.clone(),
            context.generation,
            &context.build_id,
            context.display_base_id.clone(),
            "conduitos/creche/face",
            &context.provider,
            fresh.route(),
        )
        .map_err(|error| error.as_str())?;
        replacement
            .set_visual_user_constraints(
                context.forced_colors,
                true,
                context.minimum_text_scale_percent,
            )
            .map_err(|error| error.as_str())?;
        replacement
            .preserve_encounter_from(&mut self.mask)
            .map_err(|error| error.as_str())?;
        self.mask.suspend().map_err(|error| error.as_str())?;
        self.mask = replacement;
        self.preparation.owner_host = fresh.route().map(|route| route.owner_host.clone());
        self.preparation.route_plan_id = fresh.route().map(|route| route.route_plan_id.clone());
        // Keep owner Face identity/revision and independent Mask publication counters.
        self.present_owner_face(fresh.face().clone(), fresh.interactions_admitted(), display)?;
        Ok(())
    }

    pub(in crate::product_front_door) fn toggle_owner_axis(
        &mut self,
        identities: BootIdentities,
        prior: &crate::native_guest_face::NativeGuestFace,
        route: &mut NativeOwnerReturnRoute,
        usage: u8,
        display: &mut impl PixelTarget,
    ) -> Result<crate::native_guest_face::NativeGuestFace, &'static str> {
        let (look, recipe) = axis_selection(self.preparation.selected, usage)?;
        self.select_owner_component(identities, prior, route, look, recipe, display)
    }
}

fn axis_selection(
    selected: Option<(ComponentMaskLook, TodoComponentRecipe)>,
    usage: u8,
) -> Result<(ComponentMaskLook, TodoComponentRecipe), &'static str> {
    let (look, recipe) =
        selected.unwrap_or((ComponentMaskLook::Compact, TodoComponentRecipe::Flat));
    let mut expression = ComponentVisualSelection::from(look);
    match usage {
        63 => {
            expression.density = match expression.density {
                VisualDensity::Compact => VisualDensity::Spacious,
                VisualDensity::Spacious => VisualDensity::Compact,
            }
        }
        64 => {
            expression.palette = match expression.palette {
                VisualPalette::Phosphor => VisualPalette::Editorial,
                VisualPalette::Editorial => VisualPalette::Phosphor,
            }
        }
        _ => return Err("native-owner-look-axis-invalid"),
    }
    Ok((expression.into(), recipe))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_owner_axes_preserve_the_other_axis_and_checked_recipe() {
        for recipe in [TodoComponentRecipe::Flat, TodoComponentRecipe::Grouped] {
            let mut selected = Some((ComponentMaskLook::Compact, recipe));
            for (usage, look) in [
                (64, ComponentMaskLook::CompactEditorial),
                (63, ComponentMaskLook::Spacious),
                (64, ComponentMaskLook::SpaciousPhosphor),
                (63, ComponentMaskLook::Compact),
            ] {
                let result = axis_selection(selected, usage).unwrap();
                assert_eq!(result, (look, recipe));
                selected = Some(result);
            }
        }
        assert!(axis_selection(None, 65).is_err());
    }
}
