//! Cold invocation ownership, separately admitted before the first kernel Step.
use super::prepared::NativeMaskRendererRequest;
use super::*;
use conduit_face::MaskShow;
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::{HostCallId, NodeId};
use conduit_plan_lowering::lowering::LoweredForePort;

pub(super) struct ColdMaskFields {
    pub scheduler: MaskScheduler,
    pub renderer_node: NodeId,
    pub renderer_call: HostCallId,
    pub face_fore: LoweredForePort,
    pub face_bytes: Vec<u8>,
    pub renderer: NativeMaskRendererRequest,
    pub show_fore: LoweredForePort,
    pub interaction_fore: LoweredForePort,
    pub interaction_node: NodeId,
    pub show_bytes: Vec<u8>,
    pub read_only: bool,
    pub available_show: Option<MaskShow>,
    pub receipt: Option<NativeMaskPlayReceipt>,
}
/// Contains actual serialized/Show/Face owners, with no Step or pending Host
/// Call yet. A future Available Show is private and cannot acknowledge output.
pub struct ColdNativeMaskInvocation {
    pub(super) parts: Option<ColdMaskFields>,
}
impl ColdNativeMaskInvocation {
    pub(super) fn new(parts: ColdMaskFields) -> Self {
        Self { parts: Some(parts) }
    }
    pub fn renderer_request(&self) -> Result<&NativeMaskRendererRequest, NativeMaskPlayError> {
        self.parts
            .as_ref()
            .map(|parts| &parts.renderer)
            .ok_or(NativeMaskPlayError::Kernel)
    }
    #[cfg(test)]
    pub(super) fn idle_for_test(&self) -> bool {
        use conduit_kernel::{SignSink, ValueStorage};
        self.parts.as_ref().is_some_and(|parts| {
            parts.scheduler.signs().is_empty() && parts.scheduler.values().used_items() == 0
        })
    }
    pub fn start(mut self) -> Result<PreparedNativeMaskPlay, NativeMaskPlayError> {
        let parts = self.parts.as_mut().ok_or(NativeMaskPlayError::Kernel)?;
        let input_sequence = parts.scheduler.input_sequence();
        if parts
            .scheduler
            .admit_remote_input(
                parts.face_fore.endpoint,
                parts.face_fore.cord,
                input_sequence,
                &parts.face_bytes,
            )
            .map_err(|_| NativeMaskPlayError::ForeAdmit)?
            != (RemoteIngressOutcome::Accepted {
                sequence: input_sequence,
            })
        {
            return Err(NativeMaskPlayError::ForeAdmit);
        }
        parts
            .scheduler
            .close_remote_input(parts.face_fore.endpoint, parts.face_fore.cord)
            .map_err(|_| NativeMaskPlayError::ForeClose)?;
        let mut pending = None;
        for _ in 0..64 {
            if let Some(request) = parts.scheduler.next_host_request() {
                if request.node != parts.renderer_node
                    || request.call != parts.renderer_call
                    || request.request != parts.scheduler.renderer_request()
                    || parts
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|_| NativeMaskPlayError::Value)?
                        != parts.face_bytes
                    || parts.scheduler.next_host_request().is_some()
                {
                    parts
                        .scheduler
                        .cancel()
                        .map_err(|_| NativeMaskPlayError::Kernel)?;
                    return Err(NativeMaskPlayError::HostComplete);
                }
                pending = Some(request);
                break;
            }
            if !matches!(
                parts
                    .scheduler
                    .step()
                    .map_err(|_| NativeMaskPlayError::Kernel)?,
                SchedulerStatus::Progress { .. }
            ) {
                return Err(NativeMaskPlayError::Kernel);
            }
        }
        let request = pending.ok_or(NativeMaskPlayError::Kernel)?;
        let parts = self.parts.take().ok_or(NativeMaskPlayError::Kernel)?;
        Ok(PreparedNativeMaskPlay {
            scheduler: parts.scheduler,
            request,
            renderer: parts.renderer,
            show_fore: parts.show_fore,
            interaction_fore: parts.interaction_fore,
            interaction_node: parts.interaction_node,
            show_bytes: parts.show_bytes,
            read_only: parts.read_only,
            available_show: parts.available_show,
            receipt: parts.receipt,
            retired: false,
        })
    }
}
impl Drop for ColdNativeMaskInvocation {
    fn drop(&mut self) {
        if let Some(parts) = self.parts.as_mut() {
            let _ = parts.scheduler.cancel();
        }
    }
}
