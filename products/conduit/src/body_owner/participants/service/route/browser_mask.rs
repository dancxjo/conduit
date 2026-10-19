//! Browser Mask Source selection, sealing and transactional route replacement.
use super::*;
use conduit_std_host::browser_admission::BrowserComponentRecipe;

fn source(
    look: conduit_face::ComponentMaskLook,
    recipe: BrowserComponentRecipe,
) -> (&'static str, &'static str) {
    let ordinary = match recipe {
        BrowserComponentRecipe::RequiredItem => {
            return (
                conduit_face::TODO_REQUIRED_ITEM_COMPONENT_SOURCE,
                conduit_face::todo_required_item_mask_root(look),
            )
        }
        BrowserComponentRecipe::Flat => conduit_face::TodoComponentRecipe::Flat,
        BrowserComponentRecipe::Grouped => conduit_face::TodoComponentRecipe::Grouped,
    };
    (
        conduit_face::TODO_COMPONENT_MASK_SOURCE,
        conduit_face::todo_component_mask_root(look, ordinary),
    )
}

impl Owner {
    /// Called only by the worker holding the accepted browser socket. The
    /// serialized owner verifies its exact observed binding again and retains
    /// the selected route until replacement or browser leave.
    pub(crate) fn browser_mask_route(
        &mut self,
        window_id: &str,
        credential: &MembershipCredential,
        binding: &LinkBindingId,
        evidence: Option<&BrowserCarrierLineEvidence>,
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let mut window = self.pending_browser.take().ok_or("window-not-active")?;
        let result = (|| -> Result<RemoteOwnerMaskRouteSeal, String> {
            window.check(window_id).map_err(|_| "window-not-active")?;
            let WindowState::Active {
                credential: active,
                observation,
                line_authorization,
                line_evidence,
                route,
                acknowledged_show,
                ..
            } = &mut window.state
            else {
                return Err("window-not-active".into());
            };
            if active != credential || observation.observed_binding_id != *binding {
                return Err("browser-route-carrier-mismatch".into());
            }
            let current = self.session.evidence().membership.parts.iter().any(|part| {
                part.part_id == credential.part_id
                    && part.current.as_ref().is_some_and(|host| {
                        host.host_id == credential.host_id
                            && host.boot_id == credential.boot_id
                            && host.offer_generation == observation.advertisement.offer_generation
                    })
            });
            let admitted = self.admissions.as_ref().is_some_and(|manager| {
                manager
                    .receipts
                    .iter()
                    .rev()
                    .find(|receipt| receipt.credential.part_id == credential.part_id)
                    .is_some_and(|receipt| receipt.credential == *credential)
            });
            if !current || !admitted || credential.body_id != self.session.evidence().body_id {
                return Err("browser-route-part-unavailable".into());
            }
            let browser = &observation.advertisement;
            let (mask_source, mask_root, mask_offer) = match window.component_mask_selection {
                Some((look, recipe)) => {
                    let (source, root) = source(look, recipe);
                    (
                        source,
                        root,
                        conduit_browser_mask_offer::owner_component::owner_component_offer()?,
                    )
                }
                None => (
                    conduit_browser_mask_offer::MASK_SOURCE,
                    "browser-graphical",
                    conduit_browser_mask_offer::offer(),
                ),
            };
            if !browser.capabilities.contains(&mask_offer) {
                return Err("browser-route-mask-back-unavailable".into());
            }
            let owner = self.host.advertisement();
            let evidence = evidence.ok_or("browser-line-evidence-missing")?;
            validate_carrier_evidence(
                evidence,
                line_authorization,
                window_id,
                binding,
                credential,
                owner,
                browser,
            )?;
            let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
                browser,
                owner,
                &evidence.face,
                &evidence.returned,
                &evidence.interaction,
                mask_source,
                mask_root,
            )?;
            let face = self.local_face_snapshot()?;
            let selected = RemoteOwnerMaskRouteSeal::seal_current_with_interaction(
                &self.session,
                &face,
                owner,
                browser,
                &planned,
                &evidence.face,
                &evidence.returned,
                &evidence.interaction,
            )
            .map_err(|error| format!("browser-route-refused:{error:?}"))?;
            *route = Some(Box::new(selected.clone()));
            *line_evidence = Some(Box::new(evidence.clone()));
            *acknowledged_show = None;
            Ok(selected)
        })();
        self.pending_browser = Some(window);
        let selected = result?;
        self.admit_browser_face_route(&selected)?;
        Ok(selected)
    }

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
