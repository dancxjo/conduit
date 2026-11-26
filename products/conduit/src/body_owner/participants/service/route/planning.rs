//! Select one exact browser Mask from current membership, offers and carrier.
use super::*;

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
                navigation_face,
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
            let evidence = evidence.ok_or("browser-line-evidence-missing")?;
            let owner = self.host.advertisement();
            validate_carrier_evidence(
                evidence,
                line_authorization,
                window_id,
                binding,
                credential,
                owner,
                browser,
            )?;
            let face = self.local_face_snapshot()?;
            let full_line = &conduit_host_browser_make::OWNER_FACE_FULL_LINE;
            let full = window.component_mask_selection.is_none()
                && evidence.face.binding.limits.maximum_payload_bytes
                    == full_line.maximum_payload_bytes
                && browser
                    .capabilities
                    .contains(&conduit_browser_mask_offer::full_profile::offer())
                && conduit_browser_mask_offer::full_profile::eligible(&face).is_ok();
            let (mask_source, mask_root, mask_offer) = match window.component_mask_selection {
                Some((look, recipe)) => {
                    let (source, root) = recipe.source_root(look);
                    (
                        source,
                        root,
                        conduit_browser_mask_offer::owner_component::owner_component_offer()?,
                    )
                }
                None if full => (
                    conduit_browser_mask_offer::MASK_SOURCE,
                    "browser-graphical",
                    conduit_browser_mask_offer::full_profile::offer(),
                ),
                None => (
                    conduit_browser_mask_offer::MASK_SOURCE,
                    "browser-graphical",
                    conduit_browser_mask_offer::offer(),
                ),
            };
            if !browser.capabilities.contains(&mask_offer) {
                return Err("browser-route-mask-back-unavailable".into());
            }
            // Select one exact advertised Back for this Kind. The seal below
            // still binds and validates the complete actual Host advertisement.
            let mut planning_browser = browser.clone();
            planning_browser.capabilities.retain(|candidate| {
                candidate.kind_id != mask_offer.kind_id || candidate == &mask_offer
            });
            let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
                &planning_browser,
                owner,
                &evidence.face,
                &evidence.returned,
                &evidence.interaction,
                mask_source,
                mask_root,
            )?;
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
            *navigation_face = None;
            Ok(selected)
        })();
        self.pending_browser = Some(window);
        let selected = result?;
        self.admit_browser_face_route(&selected)?;
        Ok(selected)
    }
}
