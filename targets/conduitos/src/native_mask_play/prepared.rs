//! One finite Mask execution suspended at its actual renderer Host Call.
use super::*;
use crate::native_compositor::ScanoutAcknowledgement;
use conduit_core::{HostBaseId, PortDirection, SignId};
use conduit_face::{ManifestationLifecycle, MaskShow};
use conduit_kernel::scheduler::{HostCallRequest, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, HostCallOutcome, NodeId, SignSink,
};
use conduit_plan_lowering::lowering::LoweredForePort;

pub struct NativeMaskRendererRequest {
    pub(super) face: Face,
    pub(super) prepared_show: MaskShow,
    display_base_id: HostBaseId,
}

impl NativeMaskRendererRequest {
    pub fn face(&self) -> &Face {
        &self.face
    }
    pub fn prepared_show(&self) -> &MaskShow {
        &self.prepared_show
    }
    pub fn display_base_id(&self) -> &HostBaseId {
        &self.display_base_id
    }
}

/// Preparation allocates finite serialized values. No drawing or successful
/// renderer completion occurs here. Dropping any incomplete execution cancels it.
pub struct PreparedNativeMaskPlay {
    pub(super) scheduler: MaskScheduler,
    pub(super) request: HostCallRequest,
    pub(super) renderer: NativeMaskRendererRequest,
    pub(super) show_fore: LoweredForePort,
    pub(super) interaction_node: NodeId,
    pub(super) interaction_fore: LoweredForePort,
    pub(super) show_bytes: Vec<u8>,
    pub(super) receipt: Option<NativeMaskPlayReceipt>,
    pub(super) retired: bool,
    pub(super) read_only: bool,
    pub(super) available_show: Option<MaskShow>,
}

impl PreparedNativeMaskPlay {
    #[cfg(test)]
    pub fn prepare(
        planned: &PlannedMaskPlot,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        surface_id: &str,
        display_base_id: HostBaseId,
    ) -> Result<Self, NativeMaskPlayError> {
        Self::prepare_with_profile(
            planned,
            face,
            play_sequence,
            front_subject,
            surface_id,
            display_base_id,
            NativeMaskStorageProfile::Standard,
        )
    }

    /// The caller explicitly selects initialized storage; a larger Plan cannot
    /// silently upgrade an existing default-profile renderer.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_profile(
        planned: &PlannedMaskPlot,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        surface_id: &str,
        display_base_id: HostBaseId,
        profile: NativeMaskStorageProfile,
    ) -> Result<Self, NativeMaskPlayError> {
        Self::prepare_using(
            planned,
            face,
            play_sequence,
            front_subject,
            surface_id,
            display_base_id,
            profile,
            None,
        )
    }

    /// Consume the exact retained backing; failure retires it rather than silently
    /// allocating a replacement or enlarging its admitted profile.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_storage(
        planned: &PlannedMaskPlot,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        surface_id: &str,
        display_base_id: HostBaseId,
        profile: NativeMaskStorageProfile,
        storage: RetainedNativeMaskStorage,
    ) -> Result<Self, NativeMaskPlayError> {
        Self::prepare_using(
            planned,
            face,
            play_sequence,
            front_subject,
            surface_id,
            display_base_id,
            profile,
            Some(storage),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_using(
        planned: &PlannedMaskPlot,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        surface_id: &str,
        display_base_id: HostBaseId,
        profile: NativeMaskStorageProfile,
        storage: Option<RetainedNativeMaskStorage>,
    ) -> Result<Self, NativeMaskPlayError> {
        Self::prepare_cold_using(
            planned,
            face,
            play_sequence,
            front_subject,
            surface_id,
            display_base_id,
            profile,
            storage,
        )?
        .start()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_cold_with_storage(
        planned: &PlannedMaskPlot,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        surface_id: &str,
        display_base_id: HostBaseId,
        profile: NativeMaskStorageProfile,
        storage: RetainedNativeMaskStorage,
    ) -> Result<ColdNativeMaskInvocation, NativeMaskPlayError> {
        Self::prepare_cold_using(
            planned,
            face,
            play_sequence,
            front_subject,
            surface_id,
            display_base_id,
            profile,
            Some(storage),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_cold_using(
        planned: &PlannedMaskPlot,
        face: &Face,
        play_sequence: u64,
        front_subject: &str,
        surface_id: &str,
        display_base_id: HostBaseId,
        profile: NativeMaskStorageProfile,
        storage: Option<RetainedNativeMaskStorage>,
    ) -> Result<ColdNativeMaskInvocation, NativeMaskPlayError> {
        storage::validate_profile(planned, profile)?;
        face.validate().map_err(|_| NativeMaskPlayError::Face)?;
        if planned.plan.fragments.len() != 1 || display_base_id.as_str().is_empty() {
            return Err(NativeMaskPlayError::Plan);
        }
        let fragment = &planned.plan.fragments[0];
        let active = conduit_core::bind_active_play(
            &planned.plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let prepared_show = MaskShow::prepared(
            planned,
            face,
            active.clone(),
            front_subject.into(),
            surface_id.into(),
            SignId::from("conduitos/mask/render-prepared"),
        )
        .map_err(|_| NativeMaskPlayError::Face)?;
        let face_bytes = serde_json::to_vec(face).map_err(|_| NativeMaskPlayError::Value)?;
        if face_bytes.len() > profile.bytes() as usize {
            return Err(NativeMaskPlayError::Value);
        }
        let show_value_id = show_value_id(planned, face, &active);
        let show_bytes = serde_json::to_vec(&ShowValue {
            schema: "conduit.face/show-value@1",
            mask_plan_id: planned.plan.plan_id.as_str(),
            active_play_id: active.active_play_id.as_str(),
            face_id: face.identity.as_str(),
            face_revision: face.revision,
            show_value_id: &show_value_id,
        })
        .map_err(|_| NativeMaskPlayError::Value)?;
        if show_bytes.len() > profile.bytes() as usize {
            return Err(NativeMaskPlayError::Value);
        }
        let lowered = lower_plan_fragment(fragment).map_err(|_| NativeMaskPlayError::Plan)?;
        let renderer_node = lowered
            .nodes
            .iter()
            .find(|node| node.placement_id == planned.show_placement().placement_id)
            .ok_or(NativeMaskPlayError::Shape)?;
        // Only this exact renderer effect is implemented here. Other advertised
        // effects cannot be acknowledged with renderer bytes.
        let mut renderer_calls = lowered
            .host_calls
            .iter()
            .filter(|call| call.node == renderer_node.node);
        let call = renderer_calls.next().ok_or(NativeMaskPlayError::Shape)?;
        if renderer_calls.next().is_some() {
            return Err(NativeMaskPlayError::Shape);
        }
        if call.node != renderer_node.node
            || call.call != HostCallId(0)
            || call.contract_id.as_str() != "conduit.host/present@1"
            || call.target_kind.as_ref().map(|kind| kind.as_str())
                != Some("face/base/conduitos-surface@1")
        {
            return Err(NativeMaskPlayError::Shape);
        }
        let interaction_node = lowered
            .nodes
            .iter()
            .find(|node| {
                fragment.placements.iter().any(|placement| {
                    placement.placement_id == node.placement_id
                        && placement.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND
                })
            })
            .ok_or(NativeMaskPlayError::Shape)?
            .node;
        let mut interaction_calls = lowered
            .host_calls
            .iter()
            .filter(|call| call.node == interaction_node);
        let interaction_call = interaction_calls.next().ok_or(NativeMaskPlayError::Shape)?;
        if interaction_calls.next().is_some()
            || interaction_call.call != HostCallId(0)
            || interaction_call.contract_id.as_str() != "conduit.host/face-interaction@1"
            || interaction_call
                .target_kind
                .as_ref()
                .map(|kind| kind.as_str())
                != Some(conduit_face::FACE_INTERACTION_VALUE_KIND)
        {
            return Err(NativeMaskPlayError::Shape);
        }
        let fore = |name: &str, direction| {
            lowered
                .fore_ports
                .iter()
                .find(|port| port.front_port_id.as_str() == name && port.direction == direction)
                .cloned()
                .ok_or(NativeMaskPlayError::Shape)
        };
        let face_fore = fore("face", PortDirection::Input)?;
        let show_fore = fore("show", PortDirection::Output)?;
        let interaction_fore = fore("interaction", PortDirection::Output)?;
        let scheduler = match storage {
            Some(storage) => MaskScheduler::retained(storage, planned, profile)?,
            None => MaskScheduler::Ephemeral(alloc::boxed::Box::new(scheduler_for_profile(
                fragment, &lowered, profile,
            )?)),
        };
        let storage_budget = storage::admit_budget(&scheduler, profile)?;
        let renderer_input_bytes = face_bytes.len() as u32;
        let available_show = prepared_show
            .transition(
                ManifestationLifecycle::Available,
                SignId::from("conduitos/mask/render-available"),
            )
            .map_err(|_| NativeMaskPlayError::Face)?;
        available_show
            .validate(face)
            .map_err(|_| NativeMaskPlayError::Face)?;
        let renderer_show_value_bytes = show_bytes.len() as u32;
        Ok(ColdNativeMaskInvocation::new(ColdMaskFields {
            scheduler,
            renderer_node: call.node,
            renderer_call: call.call,
            face_fore,
            face_bytes,
            renderer: NativeMaskRendererRequest {
                face: face.clone(),
                prepared_show,
                display_base_id: display_base_id.clone(),
            },
            show_fore,
            interaction_fore,
            interaction_node,
            show_bytes,
            read_only: fragment.placements.iter().any(|placement| placement.kind_id.as_str() == conduit_face::FACE_INTERACTION_KIND && matches!(placement.execution_profile_id.as_str(), conduit_conduitos_mask_offer::READ_ONLY_INTERACTION_PROFILE | conduit_conduitos_mask_offer::READ_ONLY_COLLECTION_INTERACTION_PROFILE)),
            available_show: Some(available_show),
            receipt: Some(NativeMaskPlayReceipt {
                mask_plan_id: planned.plan.plan_id.clone(),
                active_play_id: active.active_play_id,
                face_id: face.identity.as_str().into(),
                face_revision: face.revision,
                renderer_input_bytes,
                renderer_show_value_bytes,
                show_value_id,
                scanout_frame_sequence: 0,
                scanout_pixels_written: 0,
                display_base_id,
                surface_id: surface_id.into(),
                kernel_signs: 0,
                fore_endpoints: lowered.fore_ports.len() as u16,
                collection_storage: (profile == NativeMaskStorageProfile::Collection)
                    .then_some(storage_budget),
            }),
        }))
    }

    pub fn renderer_request(&self) -> &NativeMaskRendererRequest {
        &self.renderer
    }

    pub fn complete_render(
        mut self,
        ack: &ScanoutAcknowledgement<'_>,
    ) -> Result<NativeMaskInteractionSession, NativeMaskPlayError> {
        let actual = ack.composition();
        let expected = &self.renderer.prepared_show.show;
        if actual.face_id != expected.face_id
            || ack.face_revision() != expected.face_revision
            || actual.manifestation_id != expected.manifestation_id
            || actual.plan_id != expected.plan_id
            || actual.active_play_id != expected.active_play_id
            || actual.play_sequence != expected.play_sequence
            || actual.placement_id != expected.placement_id
            || actual.host_id != expected.host_id
            || actual.boot_id != expected.boot_id
            || actual.offer_generation != expected.offer_generation
            || actual.presenter_implementation_id != expected.presenter_implementation_id
            || actual.presenter_capability_id != expected.presenter_capability_id
            || actual.presenter_artifact_id != expected.presenter_artifact_id
            || actual.front_subject != expected.front_subject
            || actual.surface_id != expected.target_subject
            || actual.display_base_id != self.renderer.display_base_id
            || ack.pixels_written() == 0
        {
            return Err(NativeMaskPlayError::RendererMismatch);
        }
        let value = self
            .scheduler
            .store_host_value(&self.show_bytes)
            .map_err(|_| NativeMaskPlayError::Value)?;
        self.scheduler
            .complete_host_call(
                self.request.node,
                self.request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, self.show_bytes.len() as u32)
                            .map_err(|_| NativeMaskPlayError::Value)?,
                    ),
                    failure: None,
                },
            )
            .map_err(|_| NativeMaskPlayError::HostComplete)?;
        let mut observed_show = false;
        let mut pending_interaction = None;
        for _ in 0..64 {
            if let Some(request) = self.scheduler.next_host_request() {
                if pending_interaction.is_some()
                    || request.node != self.interaction_node
                    || request.call != HostCallId(0)
                    || request.request != self.scheduler.interaction_request()
                    || self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|_| NativeMaskPlayError::Value)?
                        != self.show_bytes
                {
                    return Err(NativeMaskPlayError::HostComplete);
                }
                pending_interaction = Some(request);
            }
            while let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.show_fore.endpoint, self.show_fore.cord)
                .map_err(|_| NativeMaskPlayError::ForeOutput)?
            {
                if observed_show
                    || self
                        .scheduler
                        .host_value(offer.value)
                        .map_err(|_| NativeMaskPlayError::Value)?
                        != self.show_bytes
                {
                    return Err(NativeMaskPlayError::Value);
                }
                self.scheduler
                    .accept_output(self.show_fore.endpoint, self.show_fore.cord, offer.sequence)
                    .map_err(|_| NativeMaskPlayError::ForeOutput)?;
                observed_show = true;
            }
            if observed_show && let Some(request) = pending_interaction.take() {
                self.renderer.prepared_show = self
                    .available_show
                    .take()
                    .ok_or(NativeMaskPlayError::Face)?;
                let receipt = self.receipt.as_mut().ok_or(NativeMaskPlayError::Kernel)?;
                receipt.kernel_signs = self.scheduler.signs().len();
                receipt.scanout_frame_sequence = ack.frame_sequence();
                receipt.scanout_pixels_written = ack.pixels_written();
                return Ok(NativeMaskInteractionSession {
                    play: self,
                    request,
                });
            }
            if !matches!(
                self.scheduler
                    .step()
                    .map_err(|_| NativeMaskPlayError::Kernel)?,
                SchedulerStatus::Progress { .. }
            ) {
                return Err(NativeMaskPlayError::Kernel);
            }
        }
        Err(NativeMaskPlayError::Kernel)
    }

    /// Complete a render-only use by explicitly closing its interaction Fore.
    #[cfg(test)]
    pub fn complete(
        self,
        ack: &ScanoutAcknowledgement<'_>,
    ) -> Result<NativeMaskPlayReceipt, NativeMaskPlayError> {
        self.complete_render(ack)?.close_without_input()
    }

    #[cfg(test)]
    pub fn cancel(mut self) -> Result<(), NativeMaskPlayError> {
        self.scheduler
            .cancel()
            .map_err(|_| NativeMaskPlayError::Kernel)?;
        self.retired = true;
        Ok(())
    }

    #[cfg(test)]
    pub fn fail(mut self) -> NativeMaskPlayError {
        match self.scheduler.cancel() {
            Ok(()) => {
                self.retired = true;
                NativeMaskPlayError::RendererFailed
            }
            Err(_) => NativeMaskPlayError::Kernel,
        }
    }
}

impl Drop for PreparedNativeMaskPlay {
    fn drop(&mut self) {
        if !self.retired {
            let _ = self.scheduler.cancel();
        }
    }
}
