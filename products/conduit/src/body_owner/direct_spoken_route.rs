//! One direct spoken child of the installed owner's presentation Plan.
//!
//! The Body stays lulled. The same selected Host Boot prepares and executes
//! this ordinary Mask Plot; no replay Host, second Body, or separate BodyPlan
//! can stand in for the owner wardrobe route.

use super::{
    presentation_wardrobe::OwnerPresentationWardrobe,
    presentation_wardrobe_runtime::wardrobe_error, Owner,
};
use conduit_core::{port_id, ConnectionTrack, SignId};
use conduit_presentation::{LocalOwnerMaskRouteSeal, MaskShow, Presentation, PresentationRole};
use conduit_std_host::{
    direct_spoken_mask_runtime::DirectSpokenMaskPreparation, ExternalForeInput,
};

impl Owner {
    /// Admit the provider-held direct child before any Start. The owner Plan
    /// records this exact Mask Plan, current Boot offer, and selected grants.
    pub(crate) fn admit_direct_spoken_route(&mut self) -> Result<LocalOwnerMaskRouteSeal, String> {
        if !self.selected_speech_host_is_idle()
            || !self.host.current().spoken_mask_artifact_route_is_current()
        {
            return Err("direct spoken Mask needs an idle current voice and free artifact".into());
        }
        let face = self.local_face_snapshot()?;
        let prepared = self.host.current().prepare_direct_spoken_mask(&face)?;
        let seal = LocalOwnerMaskRouteSeal::seal_lulled_with_grants(
            &self.session,
            &face,
            self.host.advertisement(),
            &prepared.planned_mask,
            &prepared.authority_grants,
        )
        .map_err(|error| format!("seal owner direct spoken route: {error:?}"))?;
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            Some(&seal),
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        if let Some(wardrobe) = &mut self.presentation_wardrobe {
            wardrobe
                .admit_or_replace(&self.session, &face, &current)
                .map_err(wardrobe_error)?;
        } else {
            let plot = seal.planned_mask.mask.plot_identity.clone();
            self.presentation_wardrobe = Some(
                OwnerPresentationWardrobe::seal(
                    &self.session,
                    &face,
                    &current,
                    vec![plot.clone()],
                    vec![plot],
                )
                .map_err(wardrobe_error)?,
            );
        }
        self.direct_spoken_route = Some(seal.clone());
        Ok(seal)
    }

    fn selected_direct_spoken_seal(&mut self) -> Result<LocalOwnerMaskRouteSeal, String> {
        let face = self.local_face_snapshot()?;
        let seal = Self::current_direct_spoken_route(
            &self.host,
            self.direct_spoken_route.as_ref(),
            &self.session,
            &face,
        )?
        .ok_or("direct spoken route has no current Host/provider witness")?
        .clone();
        if !self.host.current().spoken_mask_artifact_route_is_current() {
            return Err("direct spoken artifact destination already published".into());
        }
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            Some(&seal),
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        let wardrobe = self
            .presentation_wardrobe
            .as_mut()
            .ok_or("owner presentation wardrobe is not admitted")?;
        wardrobe
            .reconcile(&self.session, &face, &current)
            .map_err(wardrobe_error)?;
        let selected = wardrobe
            .control()
            .selected
            .as_ref()
            .ok_or("direct spoken Mask is not selected")?;
        if selected.route_id != format!("route/{}", seal.route_plan_id.as_str())
            || selected.mask_plot != seal.planned_mask.mask.plot_identity
        {
            return Err("direct spoken Mask is not the selected owner route".into());
        }
        Ok(seal)
    }

    /// Prepare the exact selected route before handing the sole Host to a
    /// cancellable worker. Preparation has no effect and mints no Show.
    pub(crate) fn prepare_selected_direct_spoken_start(
        &mut self,
    ) -> Result<DirectSpokenStart, String> {
        let seal = self.selected_direct_spoken_seal()?;
        let face = self.local_face_snapshot()?;
        let prepared = self.host.current().prepare_direct_spoken_mask(&face)?;
        if prepared.planned_mask != seal.planned_mask
            || prepared.authority_grants != seal.authority_grants
        {
            return Err("direct spoken Mask Plan or grants changed before Start".into());
        }
        let front_subject = face
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Body)
            .map(|subject| subject.identity.clone())
            .ok_or("direct spoken Face has no Body subject")?;
        let preparation = DirectSpokenMaskPreparation::new(
            face.clone(),
            prepared.planned_mask,
            front_subject,
            "artifact/owner-direct-spoken".into(),
            SignId::from(format!("sign/{}/prepared", seal.route_plan_id.as_str())),
            SignId::from(format!("sign/{}/available", seal.route_plan_id.as_str())),
        )?;
        let input = ExternalForeInput {
            front_port_id: port_id("face"),
            track: ConnectionTrack::Payload,
            bytes: preparation.encoded_face().to_vec(),
        };
        Ok(DirectSpokenStart {
            seal,
            face,
            fragment: prepared.plan.fragments[0].clone(),
            preparation,
            input,
        })
    }

    pub(crate) fn acknowledge_selected_direct_spoken_show(
        &mut self,
        seal: &LocalOwnerMaskRouteSeal,
        show: &MaskShow,
    ) -> Result<(), String> {
        let face = self.local_face_snapshot()?;
        let current_seal = Self::current_direct_spoken_route(
            &self.host,
            self.direct_spoken_route.as_ref(),
            &self.session,
            &face,
        )?
        .ok_or("direct spoken provider disappeared before Show acknowledgement")?;
        if current_seal != seal {
            return Err("direct spoken child route changed before Show acknowledgement".into());
        }
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            Some(seal),
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        self.presentation_wardrobe
            .as_mut()
            .ok_or("owner presentation wardrobe is not admitted")?
            .acknowledge_selected_show(&self.session, &face, &current, &seal.route_plan_id, show)
            .map_err(wardrobe_error)
    }
}

/// Exact pre-Play inputs transferred to the worker with the owner's Host.
pub(crate) struct DirectSpokenStart {
    pub(crate) seal: LocalOwnerMaskRouteSeal,
    pub(crate) face: Presentation,
    pub(crate) fragment: conduit_core::PlanFragment,
    pub(crate) preparation: DirectSpokenMaskPreparation,
    pub(crate) input: ExternalForeInput,
}
