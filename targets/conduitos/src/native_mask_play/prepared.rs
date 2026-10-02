//! One finite Mask execution suspended at its actual renderer Host Call.
use super::*;
use crate::native_compositor::ScanoutAcknowledgement;
use conduit_core::{HostBaseId, PortDirection, SignId};
use conduit_kernel::scheduler::{
    HostCallRequest, RemoteIngressOutcome, RemoteTerminalDisposition, SchedulerStatus,
};
use conduit_kernel::{HostCallOutcome, SignSink};
use conduit_plan_lowering::lowering::LoweredForePort;
use conduit_presentation::MaskShow;

pub struct NativeMaskRendererRequest {
    presentation: Presentation,
    prepared_show: MaskShow,
    display_base_id: HostBaseId,
}

impl NativeMaskRendererRequest {
    pub fn presentation(&self) -> &Presentation {
        &self.presentation
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
    scheduler: Scheduler,
    request: HostCallRequest,
    renderer: NativeMaskRendererRequest,
    show_fore: LoweredForePort,
    interaction_fore: LoweredForePort,
    show_bytes: Vec<u8>,
    receipt: Option<NativeMaskPlayReceipt>,
    retired: bool,
}

impl PreparedNativeMaskPlay {
    pub fn prepare(
        planned: &PlannedMaskPlot,
        presentation: &Presentation,
        play_sequence: u64,
        front_subject: &str,
        surface_id: &str,
        display_base_id: HostBaseId,
    ) -> Result<Self, NativeMaskPlayError> {
        presentation
            .validate()
            .map_err(|_| NativeMaskPlayError::Presentation)?;
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
            presentation,
            active.clone(),
            front_subject.into(),
            surface_id.into(),
            SignId::from("conduitos/mask/render-prepared"),
        )
        .map_err(|_| NativeMaskPlayError::Presentation)?;
        let presentation_bytes =
            serde_json::to_vec(presentation).map_err(|_| NativeMaskPlayError::Value)?;
        if presentation_bytes.len() > MAX_MASK_VALUE_BYTES {
            return Err(NativeMaskPlayError::Value);
        }
        let show_value_id = show_value_id(planned, presentation, &active);
        let show_bytes = serde_json::to_vec(&ShowValue {
            schema: "conduit.presentation/show-value@1",
            mask_plan_id: planned.plan.plan_id.as_str(),
            active_play_id: active.active_play_id.as_str(),
            presentation_id: presentation.identity.as_str(),
            presentation_revision: presentation.revision,
            show_value_id: &show_value_id,
        })
        .map_err(|_| NativeMaskPlayError::Value)?;
        if show_bytes.len() > MAX_MASK_VALUE_BYTES {
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
                != Some("presentation/base/conduitos-surface@1")
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
        let mut scheduler = scheduler(fragment, &lowered)?;
        if scheduler
            .admit_remote_input(face_fore.endpoint, face_fore.cord, 0, &presentation_bytes)
            .map_err(|_| NativeMaskPlayError::ForeAdmit)?
            != (RemoteIngressOutcome::Accepted { sequence: 0 })
        {
            return Err(NativeMaskPlayError::ForeAdmit);
        }
        scheduler
            .close_remote_input(face_fore.endpoint, face_fore.cord)
            .map_err(|_| NativeMaskPlayError::ForeClose)?;
        let mut pending = None;
        for _ in 0..64 {
            if let Some(request) = scheduler.next_host_request() {
                if request.node != call.node
                    || request.call != call.call
                    || request.request != RequestId(0)
                    || scheduler
                        .host_value(request.input.value)
                        .map_err(|_| NativeMaskPlayError::Value)?
                        != presentation_bytes
                    || scheduler.next_host_request().is_some()
                {
                    scheduler
                        .cancel()
                        .map_err(|_| NativeMaskPlayError::Kernel)?;
                    return Err(NativeMaskPlayError::HostComplete);
                }
                pending = Some(request);
                break;
            }
            if !matches!(
                scheduler.step().map_err(|_| NativeMaskPlayError::Kernel)?,
                SchedulerStatus::Progress { .. }
            ) {
                return Err(NativeMaskPlayError::Kernel);
            }
        }
        let request = pending.ok_or(NativeMaskPlayError::Kernel)?;
        Ok(Self {
            scheduler,
            request,
            renderer: NativeMaskRendererRequest {
                presentation: presentation.clone(),
                prepared_show,
                display_base_id: display_base_id.clone(),
            },
            show_fore,
            interaction_fore,
            show_bytes,
            retired: false,
            receipt: Some(NativeMaskPlayReceipt {
                mask_plan_id: planned.plan.plan_id.clone(),
                active_play_id: active.active_play_id,
                presentation_id: presentation.identity.as_str().into(),
                presentation_revision: presentation.revision,
                show_value_id,
                scanout_frame_sequence: 0,
                scanout_pixels_written: 0,
                display_base_id,
                surface_id: surface_id.into(),
                kernel_signs: 0,
                fore_endpoints: lowered.fore_ports.len() as u16,
            }),
        })
    }

    pub fn renderer_request(&self) -> &NativeMaskRendererRequest {
        &self.renderer
    }

    pub fn complete(
        mut self,
        ack: &ScanoutAcknowledgement<'_>,
    ) -> Result<NativeMaskPlayReceipt, NativeMaskPlayError> {
        let actual = ack.composition();
        let expected = &self.renderer.prepared_show.show;
        if actual.presentation_id != expected.presentation_id
            || ack.presentation_revision() != expected.presentation_revision
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
        for _ in 0..64 {
            if self.scheduler.next_host_request().is_some() {
                return Err(NativeMaskPlayError::HostComplete);
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
                    .remote_egress_accept(
                        self.show_fore.endpoint,
                        self.show_fore.cord,
                        offer.sequence,
                    )
                    .and_then(|_| {
                        self.scheduler.remote_egress_delivered(
                            self.show_fore.endpoint,
                            self.show_fore.cord,
                            offer.sequence,
                        )
                    })
                    .map_err(|_| NativeMaskPlayError::ForeOutput)?;
                observed_show = true;
            }
            match self
                .scheduler
                .step()
                .map_err(|_| NativeMaskPlayError::Kernel)?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Drained if observed_show => {
                    if self
                        .scheduler
                        .remote_egress_terminal_disposition(
                            self.interaction_fore.endpoint,
                            self.interaction_fore.cord,
                        )
                        .map_err(|_| NativeMaskPlayError::ForeOutput)?
                        != Some(RemoteTerminalDisposition::NormalClose)
                    {
                        return Err(NativeMaskPlayError::ForeOutput);
                    }
                    let mut receipt = self.receipt.take().ok_or(NativeMaskPlayError::Kernel)?;
                    receipt.kernel_signs = self.scheduler.signs().len();
                    receipt.scanout_frame_sequence = ack.frame_sequence();
                    receipt.scanout_pixels_written = ack.pixels_written();
                    self.retired = true;
                    return Ok(receipt);
                }
                _ => return Err(NativeMaskPlayError::Kernel),
            }
        }
        Err(NativeMaskPlayError::Kernel)
    }

    pub fn cancel(mut self) -> Result<(), NativeMaskPlayError> {
        self.scheduler
            .cancel()
            .map_err(|_| NativeMaskPlayError::Kernel)?;
        self.retired = true;
        Ok(())
    }

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
