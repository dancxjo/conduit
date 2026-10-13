//! One direct spoken child of the installed owner's face Plan.
//!
//! The Body stays lulled. The same selected Host Boot prepares and executes
//! this ordinary Mask Plot; no replay Host, second Body, or separate BodyPlan
//! can stand in for the owner wardrobe route.

use super::{face_wardrobe::OwnerFaceWardrobe, face_wardrobe_runtime::wardrobe_error, Owner};
use conduit_core::{port_id, ConnectionTrack, SignId};
use conduit_face::{Face, FaceRole, LocalOwnerMaskRouteSeal, MaskShow};
use conduit_std_host::{
    direct_spoken_mask_runtime::DirectSpokenMaskPreparation, ExternalForeInput,
};

impl Owner {
    /// Explicitly wear and select the direct child. A current different Show
    /// is released through wardrobe actions; its worn policy is restored after
    /// the direct route becomes selected. All revisions are checked in this
    /// one owner control turn.
    #[cfg(test)]
    pub(crate) fn select_direct_spoken_route(&mut self) -> Result<LocalOwnerMaskRouteSeal, String> {
        self.select_direct_spoken_look(None)
    }

    pub(crate) fn select_direct_spoken_look(
        &mut self,
        look: Option<conduit_std_host::direct_spoken_template_mask::DirectSpokenTemplateLook>,
    ) -> Result<LocalOwnerMaskRouteSeal, String> {
        let seal = self.admit_direct_spoken_look(look)?;
        let face = self.local_face_snapshot()?;
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let llm_speech = Self::current_llm_spoken_route(
            &self.host,
            self.llm_spoken_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_face_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            Some(&seal),
            llm_speech,
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        let wardrobe = self
            .face_wardrobe
            .as_mut()
            .ok_or("owner face wardrobe is not admitted")?;
        wardrobe
            .select_plot(
                &self.session,
                &face,
                &current,
                &seal.planned_mask.mask.plot_identity,
                &seal.route_plan_id,
            )
            .map_err(wardrobe_error)?;
        Ok(seal)
    }

    /// Admit the provider-held direct child before any Start. The owner Plan
    /// records this exact Mask Plan, current Boot offer, and selected grants.
    pub(crate) fn admit_direct_spoken_look(
        &mut self,
        look: Option<conduit_std_host::direct_spoken_template_mask::DirectSpokenTemplateLook>,
    ) -> Result<LocalOwnerMaskRouteSeal, String> {
        if !self.selected_speech_host_is_idle()
            || !self.host.current().spoken_mask_artifact_route_is_current()
        {
            return Err(
                "direct spoken Mask needs an idle current voice and retained artifact capacity"
                    .into(),
            );
        }
        let face = self.local_face_snapshot()?;
        let template = look
            .map(|look| {
                self.host
                    .current()
                    .prepare_direct_spoken_template_mask(&face, look)
            })
            .transpose()?;
        let ordinary = if template.is_none() {
            Some(self.host.current().prepare_direct_spoken_mask(&face)?)
        } else {
            None
        };
        let prepared = template
            .as_ref()
            .map(|template| template.mask())
            .or(ordinary.as_ref())
            .ok_or("direct spoken selection is missing")?;
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
        let llm_speech = Self::current_llm_spoken_route(
            &self.host,
            self.llm_spoken_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_face_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            Some(&seal),
            llm_speech,
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        if let Some(wardrobe) = &mut self.face_wardrobe {
            wardrobe
                .admit_or_replace(&self.session, &face, &current)
                .map_err(wardrobe_error)?;
        } else {
            let plot = seal.planned_mask.mask.plot_identity.clone();
            self.face_wardrobe = Some(
                OwnerFaceWardrobe::seal(
                    &self.session,
                    &face,
                    &current,
                    vec![plot.clone()],
                    vec![plot],
                )
                .map_err(wardrobe_error)?,
            );
        }
        self.direct_spoken_template = template;
        self.direct_spoken_route = Some(seal.clone());
        Ok(seal)
    }

    fn selected_direct_spoken_seal(&mut self) -> Result<LocalOwnerMaskRouteSeal, String> {
        let face = self.local_face_snapshot()?;
        let seal = Self::current_direct_spoken_route(
            &self.host,
            self.direct_spoken_route.as_ref(),
            self.direct_spoken_template.as_ref(),
            &self.session,
            &face,
        )?
        .ok_or("direct spoken route has no current Host/provider witness")?
        .clone();
        if !self.host.current().spoken_mask_artifact_route_is_current() {
            return Err("direct spoken artifact retained capacity is unavailable".into());
        }
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let llm_speech = Self::current_llm_spoken_route(
            &self.host,
            self.llm_spoken_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_face_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            Some(&seal),
            llm_speech,
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        let wardrobe = self
            .face_wardrobe
            .as_mut()
            .ok_or("owner face wardrobe is not admitted")?;
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

    /// Continue only the currently acknowledged direct Show. Reading chooses
    /// content from this Face; it never acknowledges or creates another Show.
    pub(crate) fn prepare_selected_direct_spoken_read(
        &mut self,
    ) -> Result<(LocalOwnerMaskRouteSeal, Face, MaskShow), String> {
        let seal = self.selected_direct_spoken_seal()?;
        let show = self.selected_owner_show()?;
        if show.mask_plot != seal.planned_mask.mask.plot_identity {
            return Err("selected Show is not the direct spoken Mask".into());
        }
        Ok((seal, self.local_face_snapshot()?, show))
    }

    pub(crate) fn validate_selected_direct_spoken_read(
        &mut self,
        seal: &LocalOwnerMaskRouteSeal,
        show: &MaskShow,
    ) -> Result<(), String> {
        let (current_seal, _, current_show) = self.prepare_selected_direct_spoken_read()?;
        if current_seal != *seal || current_show != *show {
            return Err("direct spoken read source changed".into());
        }
        Ok(())
    }

    /// Prepare the exact selected route before handing the sole Host to a
    /// cancellable worker. Preparation has no effect and mints no Show.
    pub(crate) fn prepare_selected_direct_spoken_start(
        &mut self,
    ) -> Result<DirectSpokenStart, String> {
        let seal = self.selected_direct_spoken_seal()?;
        let face = self.local_face_snapshot()?;
        if let Some(selected) = self.direct_spoken_template.as_ref() {
            selected.validate_current(self.host.current(), &face)?;
            let prepared = self
                .host
                .current()
                .prepare_direct_spoken_template_mask(&face, selected.look())?;
            if prepared.mask().planned_mask != seal.planned_mask
                || prepared.mask().authority_grants != seal.authority_grants
                || prepared.wording() != selected.wording()
            {
                return Err("checked spoken selection changed before Start".into());
            }
            let front_subject = face
                .subjects
                .iter()
                .find(|subject| subject.role == FaceRole::Body)
                .map(|subject| subject.identity.clone())
                .ok_or("direct spoken Face has no Body subject")?;
            let (fragment, preparation, reading) = prepared.into_preparation(
                self.host.current(),
                &face,
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
            return Ok(DirectSpokenStart {
                seal,
                face,
                fragment,
                preparation,
                input,
                template_reading: Some(reading),
            });
        }
        let prepared = self.host.current().prepare_direct_spoken_mask(&face)?;
        if prepared.planned_mask != seal.planned_mask
            || prepared.authority_grants != seal.authority_grants
        {
            return Err("direct spoken Mask Plan or grants changed before Start".into());
        }
        let front_subject = face
            .subjects
            .iter()
            .find(|subject| subject.role == FaceRole::Body)
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
            template_reading: None,
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
            self.direct_spoken_template.as_ref(),
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
        let llm_speech = Self::current_llm_spoken_route(
            &self.host,
            self.llm_spoken_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_face_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            Some(seal),
            llm_speech,
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        self.face_wardrobe
            .as_mut()
            .ok_or("owner face wardrobe is not admitted")?
            .acknowledge_selected_show(&self.session, &face, &current, &seal.route_plan_id, show)
            .map_err(wardrobe_error)
    }
}

/// Exact pre-Play inputs transferred to the worker with the owner's Host.
pub(crate) struct DirectSpokenStart {
    pub(crate) seal: LocalOwnerMaskRouteSeal,
    pub(crate) face: Face,
    pub(crate) fragment: conduit_core::PlanFragment,
    pub(crate) preparation: DirectSpokenMaskPreparation,
    pub(crate) input: ExternalForeInput,
    pub(crate) template_reading:
        Option<conduit_std_host::spoken_template_reading::PreparedSpokenTemplateReading>,
}
