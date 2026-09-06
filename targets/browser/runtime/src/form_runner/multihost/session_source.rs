//! Source-fragment Host input and ordered egress through the ordinary kernel.

use super::*;
use crate::form_runner::protocol::TourButtonTransitionEffect;

impl Session {
    pub(in super::super) fn complete_input(
        &mut self,
        active_play_id: &str,
        request: u32,
        bytes: &[u8],
    ) -> Result<Output, String> {
        if active_play_id != self.source_active_play_id.as_str() {
            return Err("multi-Host input completion has a stale Play identity".into());
        }
        if self.role != Role::Source || self.stage != Stage::Input {
            return Err("multi-Host input completion arrived in the wrong phase".into());
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or("multi-Host input request is missing")?;
        if pending.request.request.0 != request {
            return Err("multi-Host input completion has a stale request identity".into());
        }
        engine::complete_host_effect_with_output(&mut self.scheduler, pending, bytes)?;
        self.pending = None;
        self.source_offer()
    }

    pub(super) fn source_offer(&mut self) -> Result<Output, String> {
        let (endpoint, cord) = {
            let remote = self.remote();
            (remote.endpoint, remote.cord)
        };
        loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(endpoint, cord)
                .map_err(debug_error)?
            {
                if offer.sequence != self.sequence {
                    return Err("multi-Host egress changed the next ordered sequence".into());
                }
                let payload = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(debug_error)?
                    .to_vec();
                self.stage = Stage::Offered;
                return Ok(Output::Line {
                    schema: "conduit.tour/browser-memory-line-effect@1",
                    frame: Box::new(self.frame("value", offer.sequence, payload)),
                    plan_projection: Some(Box::new(self.projection.clone())),
                    receipt: None,
                });
            }
            if let Some(request) = self.scheduler.next_host_request() {
                let placement = self
                    .fragment
                    .placements
                    .get(usize::from(request.node.0))
                    .ok_or("multi-Host input has no planned placement")?;
                let operation = placement
                    .host_operations
                    .get(usize::from(request.operation.0))
                    .ok_or("multi-Host input has no planned Host operation")?;
                if operation.contract_id.as_str()
                    != crate::installed_browser::BUTTON_EVENT_OPERATION
                {
                    return Err("multi-Host source Host effect is unsupported".into());
                }
                let pending = PendingHostEffect {
                    request,
                    effect: BrowserHostEffect::ButtonTransition,
                };
                let effect = TourButtonTransitionEffect {
                    schema: "conduit.tour/button-transition-effect@1",
                    effect_kind: "button-transition",
                    active_play_id: self.source_active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.fragment.host_id.as_str().into(),
                    boot_id: self.fragment.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
                    source_interaction: Some(self.source_interaction.clone()),
                };
                self.pending = Some(pending);
                self.stage = Stage::Input;
                return Ok(Output::Input {
                    schema: "conduit.tour/multi-host-input@1",
                    input: Box::new(effect),
                    plan_projection: Box::new(self.projection.clone()),
                });
            }
            match self.scheduler.step().map_err(debug_error)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    return Err("multi-Host source became idle before offering its value".into())
                }
                SchedulerStatus::Complete => {
                    if !self
                        .scheduler
                        .remote_egress_terminal(endpoint, cord)
                        .map_err(debug_error)?
                    {
                        return Err("multi-Host source egress is not terminal".into());
                    }
                    self.stage = Stage::Closing;
                    return Ok(Output::Line {
                        schema: "conduit.tour/browser-memory-line-effect@1",
                        frame: Box::new(self.frame("close", self.sequence, Vec::new())),
                        plan_projection: None,
                        receipt: None,
                    });
                }
                SchedulerStatus::Cancelled => return Err("multi-Host source was cancelled".into()),
            }
        }
    }
}
