//! Checked owner appearance requests retain the exact local encounter.
use super::*;
use conduit_face::{
    ComponentMaskLook, ComponentVisualSelection, TodoComponentRecipe, VisualDensity, VisualPalette,
};

impl FaceArrival {
    pub(in crate::product_front_door) fn select_owner_required_item(
        &mut self,
        identities: BootIdentities,
        prior: &crate::native_guest_face::NativeGuestFace,
        route: &mut NativeOwnerReturnRoute,
        display: &mut impl PixelTarget,
    ) -> Result<crate::native_guest_face::NativeGuestFace, OwnerSelectionError> {
        let look = self
            .preparation
            .selected
            .map_or(ComponentMaskLook::Compact, |(look, _)| look);
        self.select_owner_component(
            identities,
            prior,
            route,
            look,
            TodoComponentRecipe::RequiredItem,
            display,
        )
    }

    pub(in crate::product_front_door) fn toggle_owner_axis(
        &mut self,
        identities: BootIdentities,
        prior: &crate::native_guest_face::NativeGuestFace,
        route: &mut NativeOwnerReturnRoute,
        usage: u8,
        display: &mut impl PixelTarget,
    ) -> Result<crate::native_guest_face::NativeGuestFace, OwnerSelectionError> {
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
