//! Checked component selection on the existing acknowledged browser route.
use super::*;

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
        let previous_selection = window.component_mask_selection;
        window.component_mask_selection = Some((look, recipe));
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
