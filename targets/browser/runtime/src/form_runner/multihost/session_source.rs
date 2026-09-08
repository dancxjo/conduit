//! Source-fragment Host input and ordered egress through the ordinary kernel.

use super::*;
use crate::form_runner::protocol::TourButtonTransitionEffect;
use crate::form_runner::protocol::TourTimerEffect;

impl Session {
    pub(in super::super) fn complete_timer(
        &mut self,
        active_play_id: &str,
        request: u32,
    ) -> Result<Output, String> {
        if active_play_id != self.source_active_play_id.as_str() {
            return Err("multi-Host timer completion has a stale Play identity".into());
        }
        if self.role != Role::Source || self.stage != Stage::Timing {
            return Err("multi-Host timer completion arrived in the wrong phase".into());
        }
        let pending = self
            .pending
            .take()
            .ok_or("multi-Host timer request is missing")?;
        if pending.request.request.0 != request {
            self.pending = Some(pending);
            return Err("multi-Host timer completion has a stale request identity".into());
        }
        engine::complete_host_effect(&mut self.scheduler, &pending)?;
        self.source_offer()
    }

    pub(in super::super) fn observe_partial_send(
        &mut self,
        sent_bytes: usize,
    ) -> Result<Output, String> {
        if self.role != Role::Source || self.stage != Stage::Offered {
            return Err("partial Line progress does not match an offered source frame".into());
        }
        self.deliveries
            .get_mut(usize::try_from(self.sequence).map_err(debug_error)?)
            .ok_or("partial Line progress has no correlated delivery tracker")?
            .partially_sent(sent_bytes)
            .map_err(debug_error)?;
        Ok(Output::Waiting {
            schema: "conduit.tour/multi-host-progress@1",
            phase: "partially-sent-awaiting-delivery",
            plan_id: self.fragment.plan_id.as_str().into(),
        })
    }

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
                if self.deliveries.len()
                    >= usize::from(conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS)
                {
                    return Err("multi-Host delivery observation bound exhausted".into());
                }
                if self.deliveries.len() != usize::try_from(offer.sequence).map_err(debug_error)? {
                    return Err("multi-Host delivery correlation sequence is not contiguous".into());
                }
                let correlation = offer.sequence.to_le_bytes();
                self.retain_line_record(conduit_net::RecordTranscriptDirection::Sent, &payload)?;
                let mut delivery = conduit_net::RecordDeliveryTracker::locally_accepted(
                    &correlation,
                    payload.len(),
                )
                .map_err(debug_error)?;
                delivery
                    .framed_queued(offer.sequence)
                    .map_err(debug_error)?;
                self.deliveries.push(delivery);
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
                if engine::transforms::complete_transform(
                    &mut self.scheduler,
                    placement,
                    operation,
                    request,
                )? {
                    continue;
                }
                if matches!(
                    operation.contract_id.as_str(),
                    conduit_core::WAIT_HOST_OPERATION_CONTRACT
                        | conduit_core::MONOTONIC_TIMER_HOST_OPERATION_CONTRACT
                ) {
                    let input = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(debug_error)?;
                    let duration_millis = u64::from_le_bytes(
                        input
                            .try_into()
                            .map_err(|_| "multi-Host timer duration is not an exact u64")?,
                    );
                    let pending = PendingHostEffect {
                        request,
                        effect: BrowserHostEffect::Timer { duration_millis },
                    };
                    let effect = TourTimerEffect {
                        schema: "conduit.tour/timer-effect@1",
                        effect_kind: "timer",
                        active_play_id: self.source_active_play_id.as_str().into(),
                        placement_id: placement.placement_id.as_str().into(),
                        host_id: self.fragment.host_id.as_str().into(),
                        boot_id: self.fragment.boot_id.as_str().into(),
                        request_sequence: request.request.0,
                        duration_millis,
                        source_interaction: Some(self.source_interaction.clone()),
                    };
                    self.pending = Some(pending);
                    self.stage = Stage::Timing;
                    return Ok(Output::Timer {
                        schema: "conduit.tour/multi-host-timer@1",
                        timer: Box::new(effect),
                        plan_projection: Box::new(self.projection.clone()),
                    });
                }
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
                SchedulerStatus::Drained => {
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
