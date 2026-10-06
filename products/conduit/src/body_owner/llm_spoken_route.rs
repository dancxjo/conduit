//! One LLM-assisted spoken child of the installed owner's Body wardrobe.
//! The accepted wording comes from the current Face through the reviewed
//! semantic validator; synthesis and the artifact have separate receipts.

use super::{
    presentation_wardrobe::OwnerPresentationWardrobe,
    presentation_wardrobe_runtime::wardrobe_error, Owner,
};
use conduit_core::{port_id, ConnectionTrack, SignId};
use conduit_presentation::{
    GenerativePresenterBounds, GenerativePresenterRequest, LocalOwnerMaskRouteSeal, MaskShow,
    Presentation, PresentationRole,
};
use conduit_std_host::{spoken_mask_runtime::SpokenMaskPreparation, ExternalForeInput};

impl Owner {
    pub(crate) fn admit_llm_spoken_route(&mut self) -> Result<LocalOwnerMaskRouteSeal, String> {
        if !self.selected_speech_host_is_idle()
            || !self.host.current().spoken_mask_artifact_route_is_current()
        {
            return Err(
                "LLM spoken Mask needs an idle current voice and retained artifact capacity".into(),
            );
        }
        let face = self.local_face_snapshot()?;
        let prepared = self.host.current().prepare_llm_spoken_mask(&face)?;
        let seal = LocalOwnerMaskRouteSeal::seal_lulled_with_grants(
            &self.session,
            &face,
            self.host.advertisement(),
            &prepared.planned_mask,
            &prepared.authority_grants,
        )
        .map_err(|error| format!("seal owner LLM spoken route: {error:?}"))?;
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let direct = Self::current_direct_spoken_route(
            &self.host,
            self.direct_spoken_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            direct,
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
        self.llm_spoken_route = Some(seal.clone());
        Ok(seal)
    }

    pub(crate) fn select_llm_spoken_route(&mut self) -> Result<LocalOwnerMaskRouteSeal, String> {
        let seal = self.admit_llm_spoken_route()?;
        let face = self.local_face_snapshot()?;
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let direct = Self::current_direct_spoken_route(
            &self.host,
            self.direct_spoken_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            direct,
            Some(&seal),
            &self.session,
            &face,
            super::super::super::current_time_millis()?,
        );
        self.presentation_wardrobe
            .as_mut()
            .ok_or("owner presentation wardrobe is not admitted")?
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

    fn selected_llm_spoken_seal(&mut self) -> Result<LocalOwnerMaskRouteSeal, String> {
        let face = self.local_face_snapshot()?;
        let seal = Self::current_llm_spoken_route(
            &self.host,
            self.llm_spoken_route.as_ref(),
            &self.session,
            &face,
        )?
        .ok_or("LLM spoken route has no current model/voice witness")?
        .clone();
        if !self.host.current().spoken_mask_artifact_route_is_current() {
            return Err("LLM spoken artifact retained capacity is unavailable".into());
        }
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let direct = Self::current_direct_spoken_route(
            &self.host,
            self.direct_spoken_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            direct,
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
        if wardrobe.control().selected.as_ref().is_none_or(|selected| {
            selected.route_id != format!("route/{}", seal.route_plan_id.as_str())
                || selected.mask_plot != seal.planned_mask.mask.plot_identity
        }) {
            return Err("LLM spoken Mask is not the selected owner route".into());
        }
        Ok(seal)
    }

    pub(crate) fn prepare_selected_llm_spoken_start(&mut self) -> Result<LlmSpokenStart, String> {
        let seal = self.selected_llm_spoken_seal()?;
        let face = self.local_face_snapshot()?;
        let prepared = self.host.current().prepare_llm_spoken_mask(&face)?;
        if prepared.planned_mask != seal.planned_mask
            || prepared.authority_grants != seal.authority_grants
        {
            return Err("LLM spoken Mask Plan or grants changed before Start".into());
        }
        let front_subject = face
            .subjects
            .iter()
            .find(|subject| subject.role == PresentationRole::Body)
            .map(|subject| subject.identity.clone())
            .ok_or("LLM spoken Face has no Body subject")?;
        let request = GenerativePresenterRequest::from_presentation(
            format!("request/{}/wording", seal.route_plan_id.as_str()),
            conduit_std_host::hosted_local_model::finite_face_wording_presenter_policy(),
            face.clone(),
            None,
            GenerativePresenterBounds::reviewed_default(),
        )
        .map_err(|error| format!("prepare LLM Presenter request: {error:?}"))?;
        let preparation = SpokenMaskPreparation {
            request,
            presentation: face.clone(),
            planned_mask: prepared.planned_mask,
            front_subject,
            target_subject: "artifact/owner-llm-spoken".into(),
            prepared_sign: SignId::from(format!("sign/{}/prepared", seal.route_plan_id.as_str())),
            available_sign: SignId::from(format!("sign/{}/available", seal.route_plan_id.as_str())),
        };
        let input = ExternalForeInput {
            front_port_id: port_id("face"),
            track: ConnectionTrack::Payload,
            bytes: serde_json::to_vec(&face)
                .map_err(|error| format!("encode owner LLM Face: {error}"))?,
        };
        Ok(LlmSpokenStart {
            seal,
            face,
            fragment: prepared.plan.fragments[0].clone(),
            preparation,
            input,
        })
    }

    pub(crate) fn acknowledge_selected_llm_spoken_show(
        &mut self,
        seal: &LocalOwnerMaskRouteSeal,
        show: &MaskShow,
    ) -> Result<(), String> {
        let face = self.local_face_snapshot()?;
        let current_seal = Self::current_llm_spoken_route(
            &self.host,
            self.llm_spoken_route.as_ref(),
            &self.session,
            &face,
        )?
        .ok_or("LLM spoken provider disappeared before Show acknowledgement")?;
        if current_seal != seal {
            return Err("LLM spoken child route changed before Show acknowledgement".into());
        }
        let terminal = Self::current_attached_terminal_route(
            &self.host,
            self.attached_terminal_route.as_ref(),
            &self.session,
            &face,
        )?;
        let direct = Self::current_direct_spoken_route(
            &self.host,
            self.direct_spoken_route.as_ref(),
            &self.session,
            &face,
        )?;
        let current = Self::current_presentation_routes_with_native_and_speech(
            self.host.advertisement(),
            self.pending_browser.as_ref(),
            terminal,
            self.pending_native_mask.as_ref(),
            direct,
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

pub(crate) struct LlmSpokenStart {
    pub(crate) seal: LocalOwnerMaskRouteSeal,
    pub(crate) face: Presentation,
    pub(crate) fragment: conduit_core::PlanFragment,
    pub(crate) preparation: SpokenMaskPreparation,
    pub(crate) input: ExternalForeInput,
}
