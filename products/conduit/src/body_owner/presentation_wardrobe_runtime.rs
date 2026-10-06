//! Installed owner orchestration of its already sealed presentation wardrobe.
use super::{
    presentation_wardrobe::{OwnerPresentationWardrobe, OwnerPresentationWardrobeError},
    Owner,
};
use conduit_presentation::{
    CurrentOwnerPresentationRoute, LocalOwnerMaskRouteSeal, MaskShow, MaskWardrobeAction,
    RemoteOwnerMaskRouteSeal,
};
use serde_json::{json, Value};

impl Owner {
    pub(crate) fn current_presentation_routes<'a>(
        owner_offer: &'a conduit_core::HostAdvertisement,
        browser: Option<&'a super::participants::BrowserWindow>,
        local: Option<&'a LocalOwnerMaskRouteSeal>,
    ) -> Vec<CurrentOwnerPresentationRoute<'a>> {
        let mut current = Vec::with_capacity(2);
        if let Some(seal) = local {
            current.push(CurrentOwnerPresentationRoute::Local { seal, owner_offer });
        }
        if let Some((seal, mask_host_offer, face_line, return_line, interaction_line)) =
            browser.and_then(super::participants::BrowserWindow::current_mask_route)
        {
            current.push(CurrentOwnerPresentationRoute::Remote {
                seal,
                owner_offer,
                mask_host_offer,
                face_line,
                return_line,
                interaction_line: Some(interaction_line),
            });
        }
        current
    }

    pub(crate) fn admit_browser_presentation_route(
        &mut self,
        seal: &RemoteOwnerMaskRouteSeal,
    ) -> Result<(), String> {
        let face = self.local_face_snapshot()?;
        let current = Self::current_presentation_routes(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            None,
        );
        if !current.iter().any(|route| {
            matches!(route,
                CurrentOwnerPresentationRoute::Remote { seal: found, .. }
                    if found.route_plan_id == seal.route_plan_id
            )
        }) {
            return Err("browser presentation route is no longer current".into());
        }
        if let Some(wardrobe) = &mut self.presentation_wardrobe {
            wardrobe
                .admit_or_replace(&self.session, &face, &current)
                .map_err(wardrobe_error)?;
        } else {
            let mask = seal.planned_mask.mask.plot_identity.clone();
            self.presentation_wardrobe = Some(
                OwnerPresentationWardrobe::seal(
                    &self.session,
                    &face,
                    &current,
                    vec![mask.clone()],
                    vec![mask],
                )
                .map_err(wardrobe_error)?,
            );
        }
        Ok(())
    }

    pub(crate) fn acknowledge_selected_browser_show(
        &mut self,
        seal: &RemoteOwnerMaskRouteSeal,
        show: &MaskShow,
    ) -> Result<(), String> {
        let face = self.local_face_snapshot()?;
        let current = Self::current_presentation_routes(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            None,
        );
        self.presentation_wardrobe
            .as_mut()
            .ok_or("owner presentation wardrobe is not admitted")?
            .acknowledge_selected_show(&self.session, &face, &current, &seal.route_plan_id, show)
            .map_err(wardrobe_error)
    }

    /// The installed owner keeps the Body-lifetime wardrobe. The attached
    /// terminal supplies only its already sealed local route and actual Show.
    pub(crate) fn acknowledge_attached_terminal_show(
        &mut self,
        seal: &LocalOwnerMaskRouteSeal,
        show: &MaskShow,
    ) -> Result<(), String> {
        self.validate_attached_terminal_route(seal, show)?;
        let face = self.local_face_snapshot()?;
        let current = Self::current_presentation_routes(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            Some(seal),
        );
        if let Some(wardrobe) = &mut self.presentation_wardrobe {
            wardrobe
                .admit_or_replace(&self.session, &face, &current)
                .map_err(wardrobe_error)?;
        } else {
            let mask = seal.planned_mask.mask.plot_identity.clone();
            self.presentation_wardrobe = Some(
                OwnerPresentationWardrobe::seal(
                    &self.session,
                    &face,
                    &current,
                    vec![mask.clone()],
                    vec![mask],
                )
                .map_err(wardrobe_error)?,
            );
        }
        let wardrobe = self
            .presentation_wardrobe
            .as_mut()
            .expect("just admitted owner wardrobe");
        if wardrobe
            .control()
            .selected
            .as_ref()
            .is_some_and(|selected| {
                selected.route_id == format!("route/{}", seal.route_plan_id.as_str())
            })
        {
            wardrobe
                .acknowledge_selected_show(
                    &self.session,
                    &face,
                    &current,
                    &seal.route_plan_id,
                    show,
                )
                .map_err(wardrobe_error)?;
        }
        Ok(())
    }

    pub(crate) fn attached_terminal_wardrobe_report(
        &mut self,
        seal: &LocalOwnerMaskRouteSeal,
        show: &MaskShow,
        basis_revision: u64,
        action: Option<MaskWardrobeAction>,
    ) -> Result<Value, String> {
        self.validate_attached_terminal_route(seal, show)?;
        let face = self.local_face_snapshot()?;
        let current = Self::current_presentation_routes(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            Some(seal),
        );
        let wardrobe = self
            .presentation_wardrobe
            .as_mut()
            .ok_or("owner presentation wardrobe is not admitted")?;
        let (reconciliation, transition) = if let Some(action) = action {
            let transition = wardrobe
                .apply(&self.session, &face, &current, basis_revision, action)
                .map_err(wardrobe_error)?;
            (transition.reconciliation.clone(), Some(transition))
        } else {
            (
                wardrobe
                    .reconcile(&self.session, &face, &current)
                    .map_err(wardrobe_error)?,
                None,
            )
        };
        let admitted = wardrobe
            .plan()
            .admit_current_routes(&self.session, &face, &current)
            .map_err(|error| format!("owner wardrobe routes: {error:?}"))?;
        let show_id = match wardrobe.selected_show(&self.session, &face, &current) {
            Ok(selected_show) if selected_show == show => Some(show.show_id.as_str()),
            Ok(_) | Err(OwnerPresentationWardrobeError::ShowNotAcknowledged) => None,
            Err(error) => return Err(wardrobe_error(error)),
        };
        let selected = wardrobe.control().selected.as_ref();
        Ok(json!({
            "schema": "conduit.body/attached-terminal-wardrobe@1",
            "scope": "owner-body",
            "durable": false,
            "body_id": seal.body_id,
            "face_id": seal.face_id,
            "face_revision": seal.face_revision,
            "host_id": seal.owner_offer.host_id,
            "boot_id": seal.owner_offer.boot_id,
            "offer_generation": seal.owner_offer.offer_generation.0,
            "route_plan_id": seal.route_plan_id,
            "owner_plan_id": wardrobe.plan().plan_id,
            "wardrobe": wardrobe.control().scoped_wardrobe.wardrobe,
            "admitted_routes": admitted.routes(),
            "selected": selected,
            "show_id": show_id,
            "fresh_show_required": selected.is_some() && show_id.is_none(),
            "reconciliation": reconciliation,
            "transition": transition,
            "unadmitted_masks": "other Mask routes require current sealed Host and Line witnesses",
        }))
    }

    pub(crate) fn validate_selected_terminal_show(
        &mut self,
        seal: &LocalOwnerMaskRouteSeal,
        show: &MaskShow,
    ) -> Result<(), String> {
        let face = self.local_face_snapshot()?;
        let current = Self::current_presentation_routes(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            Some(seal),
        );
        let wardrobe = self
            .presentation_wardrobe
            .as_mut()
            .ok_or("owner presentation wardrobe is not admitted")?;
        let selected = wardrobe
            .selected_show(&self.session, &face, &current)
            .map_err(wardrobe_error)?;
        if selected != show {
            return Err("attached terminal Show differs from selected owner Show".into());
        }
        Ok(())
    }

    pub(crate) fn forget_attached_terminal_show(&mut self, seal: &LocalOwnerMaskRouteSeal) {
        if let Some(wardrobe) = &mut self.presentation_wardrobe {
            wardrobe.forget_show_for(&seal.route_plan_id);
        }
    }
}

pub(crate) fn wardrobe_error(error: OwnerPresentationWardrobeError) -> String {
    format!("owner presentation wardrobe refused: {error:?}")
}
