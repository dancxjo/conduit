//! Authenticated owner Mask/Look replacement with exact named focus retention.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::product_front_door) enum OwnerSelectionError {
    ProvisionalInput,
    Route(&'static str),
}
impl OwnerSelectionError {
    pub(in crate::product_front_door) fn code(self) -> &'static str {
        match self {
            Self::ProvisionalInput => "owner-look-provisional-input",
            Self::Route(code) => code,
        }
    }
}
impl From<&'static str> for OwnerSelectionError {
    fn from(code: &'static str) -> Self {
        Self::Route(code)
    }
}

impl FaceArrival {
    pub(in crate::product_front_door) fn ensure_look_replacement_ready(
        &self,
    ) -> Result<(), OwnerSelectionError> {
        if self
            .mask
            .scene()
            .is_some_and(|scene| scene.has_pending_input())
        {
            return Err(OwnerSelectionError::ProvisionalInput);
        }
        Ok(())
    }

    /// Mask-local appearance replacement through the current authenticated owner.
    /// The owner returns the identical Face and a newly sealed planned Mask.
    pub(in crate::product_front_door) fn select_owner_component(
        &mut self,
        identities: BootIdentities,
        prior: &crate::native_guest_face::NativeGuestFace,
        route: &mut NativeOwnerReturnRoute,
        look: conduit_face::ComponentMaskLook,
        recipe: conduit_face::TodoComponentRecipe,
        display: &mut impl PixelTarget,
    ) -> Result<crate::native_guest_face::NativeGuestFace, OwnerSelectionError> {
        let preferences = (
            self.preparation.forced_colors,
            self.preparation.minimum_text_scale_percent,
        );
        self.select_owner_component_with_preferences(
            identities,
            prior,
            route,
            (look, recipe),
            preferences,
            display,
        )
    }

    fn select_owner_component_with_preferences(
        &mut self,
        identities: BootIdentities,
        prior: &crate::native_guest_face::NativeGuestFace,
        route: &mut NativeOwnerReturnRoute,
        selection: (
            conduit_face::ComponentMaskLook,
            conduit_face::TodoComponentRecipe,
        ),
        preferences: (bool, u16),
        display: &mut impl PixelTarget,
    ) -> Result<crate::native_guest_face::NativeGuestFace, OwnerSelectionError> {
        let (look, recipe) = selection;
        self.ensure_look_replacement_ready()?;
        let prior_focus = self.mask.scene().and_then(|scene| scene.focused_request());
        let old_show = self.mask.show().cloned().ok_or("owner-face-show-absent")?;
        let fresh = route.select_component(identities, prior, &old_show, look, recipe)?;
        self.replace_owner_mask(&fresh, preferences, display)?;
        if let Some(request) = prior_focus {
            self.mask
                .focus_named(&request, display)
                .map_err(|error| error.as_str())?;
        }
        self.acknowledge_owner_show(route, identities)?;
        self.record_focus()?;
        self.preparation.selected = Some((look, recipe));
        Ok(fresh)
    }

    fn replace_owner_mask(
        &mut self,
        fresh: &crate::native_guest_face::NativeGuestFace,
        preferences: (bool, u16),
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
            .set_visual_user_constraints(preferences.0, true, preferences.1)
            .map_err(|error| error.as_str())?;
        replacement
            .preserve_encounter_from(&mut self.mask)
            .map_err(|error| error.as_str())?;
        self.mask.suspend().map_err(|error| error.as_str())?;
        self.mask = replacement;
        // This is the local swap boundary. Failures before it retain the old
        // preferences; later presentation/acknowledgement failure is not rollback.
        self.preparation.forced_colors = preferences.0;
        self.preparation.minimum_text_scale_percent = preferences.1;
        self.preparation.owner_host = fresh.route().map(|route| route.owner_host.clone());
        self.preparation.route_plan_id = fresh.route().map(|route| route.route_plan_id.clone());
        // Keep owner Face identity/revision and independent Mask publication counters.
        self.present_owner_face(fresh.face().clone(), fresh.interactions_admitted(), display)?;
        Ok(())
    }

    pub(in crate::product_front_door) fn toggle_owner_text_scale(
        &mut self,
        prior: &crate::native_guest_face::NativeGuestFace,
        route: &mut NativeOwnerReturnRoute,
        identities: BootIdentities,
        display: &mut impl PixelTarget,
    ) -> Result<crate::native_guest_face::NativeGuestFace, OwnerSelectionError> {
        self.ensure_look_replacement_ready()?;
        let scale = if self.preparation.minimum_text_scale_percent == 100 {
            200
        } else {
            100
        };
        let (look, recipe) = self.preparation.selected.unwrap_or((
            conduit_face::ComponentMaskLook::Compact,
            conduit_face::TodoComponentRecipe::Flat,
        ));
        self.select_owner_component_with_preferences(
            identities,
            prior,
            route,
            (look, recipe),
            (self.preparation.forced_colors, scale),
            display,
        )
    }

    pub(in crate::product_front_door) fn toggle_owner_forced_colors(
        &mut self,
        prior: &crate::native_guest_face::NativeGuestFace,
        route: &mut NativeOwnerReturnRoute,
        identities: BootIdentities,
        display: &mut impl PixelTarget,
    ) -> Result<crate::native_guest_face::NativeGuestFace, OwnerSelectionError> {
        self.ensure_look_replacement_ready()?;
        let forced_colors = !self.preparation.forced_colors;
        let (look, recipe) = self.preparation.selected.unwrap_or((
            conduit_face::ComponentMaskLook::Compact,
            conduit_face::TodoComponentRecipe::Flat,
        ));
        self.select_owner_component_with_preferences(
            identities,
            prior,
            route,
            (look, recipe),
            (forced_colors, self.preparation.minimum_text_scale_percent),
            display,
        )
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
