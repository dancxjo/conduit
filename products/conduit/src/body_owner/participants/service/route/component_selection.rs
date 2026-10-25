//! Checked component selection on the existing acknowledged browser route.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

#[derive(Clone)]
pub(crate) enum BrowserMaskSelection {
    Preset(
        conduit_face::ComponentMaskLook,
        conduit_std_host::browser_admission::BrowserComponentRecipe,
    ),
    History(Box<conduit_face::LookInfo>),
}

impl Owner {
    /// Mask-local configuration keeps the active Body Play and exact Face.
    /// The caller must hold the authenticated current carrier and acknowledged Show.
    pub(crate) fn select_browser_component_mask(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        look: conduit_face::ComponentMaskLook,
        recipe: conduit_std_host::browser_admission::BrowserComponentRecipe,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.select_browser_presentation(
            window_id,
            binding,
            request,
            show,
            BrowserMaskSelection::Preset(look, recipe),
        )
    }

    pub(crate) fn select_browser_history_mask(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        look: &[u8],
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.validate_browser_mask_show(window_id, binding, request, show)?;
        if look.is_empty() || look.len() > conduit_face::MAX_LOOK_BYTES {
            return Err("browser-history-look-size".into());
        }
        let look =
            conduit_face::LookInfo::decode(look).map_err(|_| "browser-history-look-invalid")?;
        self.select_browser_presentation(
            window_id,
            binding,
            request,
            show,
            BrowserMaskSelection::History(Box::new(look)),
        )
    }

    fn select_browser_presentation(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        selection: BrowserMaskSelection,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        self.validate_browser_mask_show(window_id, binding, request, show)?;
        let previous_wardrobe = self.face_wardrobe.clone();
        let window = self.pending_browser.as_mut().ok_or("window-not-active")?;
        let WindowState::Active {
            credential,
            line_evidence: Some(lines),
            route,
            acknowledged_show,
            ..
        } = &window.state
        else {
            return Err("browser-component-carrier-unavailable".into());
        };
        let credential = credential.clone();
        let lines = (**lines).clone();
        let previous_route = route.clone();
        let previous_show = acknowledged_show.clone();
        let previous_selection = window.component_mask_selection.clone();
        window.component_mask_selection = Some(selection);
        let result = self
            .browser_mask_route(window_id, &credential, binding, Some(&lines))
            .and_then(|route| {
                self.select_component_face_route(&route)?;
                Ok(route)
            });
        match result {
            Ok(route) => Ok(route),
            Err(error) => {
                self.face_wardrobe = previous_wardrobe;
                if let Some(window) = self.pending_browser.as_mut() {
                    window.component_mask_selection = previous_selection;
                    if let WindowState::Active {
                        route,
                        acknowledged_show,
                        ..
                    } = &mut window.state
                    {
                        *route = previous_route;
                        *acknowledged_show = previous_show;
                    }
                }
                Err(error)
            }
        }
    }
}

pub(super) fn plan(
    selection: Option<&BrowserMaskSelection>,
    browser: &conduit_core::HostAdvertisement,
    owner: &conduit_core::HostAdvertisement,
    evidence: &BrowserCarrierLineEvidence,
    face: &conduit_face::Face,
) -> Result<conduit_face::PlannedMaskPlot, String> {
    let (source, root, offer) = match selection {
        Some(BrowserMaskSelection::Preset(look, recipe)) => {
            let (source, root) = recipe.source_root(*look)?;
            (source, root, conduit_browser_mask_offer::configured_offer())
        }
        Some(BrowserMaskSelection::History(_)) => (
            conduit_face::HISTORY_COMPONENT_MASK_SOURCE,
            conduit_face::HISTORY_COMPONENT_MASK_ROOT,
            conduit_browser_mask_offer::configured_offer(),
        ),
        None => (
            conduit_browser_mask_offer::MASK_SOURCE,
            "browser-graphical",
            conduit_browser_mask_offer::offer(),
        ),
    };
    if !browser.capabilities.contains(&offer) {
        return Err("browser-route-mask-back-unavailable".into());
    }
    if let Some(BrowserMaskSelection::History(look)) = selection {
        let planned =
            conduit_browser_mask_offer::planned_owner_face_show_interaction_mask_with_look(
                browser,
                owner,
                &evidence.face,
                &evidence.returned,
                &evidence.interaction,
                source,
                root,
                look,
            )?;
        let configuration = conduit_browser_mask_offer::configured_mask_configuration_with_look(
            browser,
            &planned,
            Some(look),
        )?
        .ok_or("browser-history-configuration-missing")?;
        let recipe = conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .map_err(|_| "browser-history-recipe-invalid")?;
        conduit_face::prepare_component_recipe(recipe, face)
            .map_err(|_| "browser-history-face-unavailable")?;
        Ok(planned)
    } else {
        conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
            browser,
            owner,
            &evidence.face,
            &evidence.returned,
            &evidence.interaction,
            source,
            root,
        )
    }
}

#[cfg(test)]
#[path = "component_selection/tests.rs"]
pub(super) mod tests;
