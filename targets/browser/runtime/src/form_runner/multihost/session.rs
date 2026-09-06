//! One role of the exact two-browser-Host executable-tour Play.

use super::plan::PreparedPlan;
use super::protocol::{self, LineFrame, MultiHostReceipt, Output, PlanProjection};
use crate::form_runner::engine::{self, BrowserHostEffect, DriveStatus, PendingHostEffect};
use crate::form_runner::protocol::{
    decode_manifestation, TourBackEvidence, TourEffect, TourGearEvidence,
};
use crate::source_interaction::SourceInteractionEvidence;
use conduit_core::{
    bind_active_play, bind_presentation, bind_sign, ActivePlayId, PlanFragment,
    PresentationIdentity,
};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_plan_lowering::lowering::{LoweredPlanFragment, RemoteCordDirection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    Source,
    Sink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Offered,
    Accepted,
    Presenting,
    WaitingClose,
    Closing,
    Complete,
    Cancelled,
}

pub(super) struct Session {
    role: Role,
    stage: Stage,
    scheduler: engine::TourScheduler,
    fragment: PlanFragment,
    lowered: LoweredPlanFragment,
    projection: PlanProjection,
    source_active_play_id: ActivePlayId,
    sink_active_play_id: ActivePlayId,
    source_interaction: SourceInteractionEvidence,
    pending: Option<PendingHostEffect>,
    latest_presentation: Option<PresentationIdentity>,
}

impl Session {
    pub(super) fn prepare(
        role: Role,
        exact: PreparedPlan,
        play_sequence: u64,
        source_interaction: SourceInteractionEvidence,
    ) -> Result<(Self, Output), String> {
        let source_fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_host.host_id)
            .cloned()
            .ok_or_else(|| "multi-Host source fragment is missing".to_string())?;
        let sink_fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.sink_host.host_id)
            .cloned()
            .ok_or_else(|| "multi-Host sink fragment is missing".to_string())?;
        let fragment = match role {
            Role::Source => source_fragment.clone(),
            Role::Sink => sink_fragment.clone(),
        };
        let (scheduler, lowered) = engine::prepare_remote_fragment(&fragment)?;
        let direction = lowered
            .remote_endpoints
            .first()
            .map(|remote| remote.direction)
            .ok_or_else(|| "multi-Host fragment has no remote endpoint".to_string())?;
        if !matches!(
            (role, direction),
            (Role::Source, RemoteCordDirection::Egress)
                | (Role::Sink, RemoteCordDirection::Ingress)
        ) {
            return Err("multi-Host fragment has the wrong remote direction".into());
        }
        let source_active = bind_active_play(
            &exact.plan.plan_id,
            &source_fragment.host_id,
            &source_fragment.boot_id,
            play_sequence,
        )
        .active_play_id;
        let sink_active = bind_active_play(
            &exact.plan.plan_id,
            &sink_fragment.host_id,
            &sink_fragment.boot_id,
            play_sequence,
        )
        .active_play_id;
        let projection = protocol::projection(&exact.plan, play_sequence)?;
        if projection.cord.line_id != exact.line.line_id.as_str() {
            return Err("multi-Host Plan projection changed the selected Line".into());
        }
        let stage = match role {
            Role::Source => Stage::Offered,
            Role::Sink => Stage::Accepted,
        };
        let mut session = Self {
            role,
            stage,
            scheduler,
            fragment,
            lowered,
            projection,
            source_active_play_id: source_active,
            sink_active_play_id: sink_active,
            source_interaction,
            pending: None,
            latest_presentation: None,
        };
        let output = match role {
            Role::Source => session.source_offer()?,
            Role::Sink => Output::Waiting {
                schema: "conduit.tour/multi-host-progress@1",
                phase: "waiting-for-value",
                plan_id: session.fragment.plan_id.as_str().into(),
            },
        };
        Ok((session, output))
    }

    pub(super) fn ingest(&mut self, frame: LineFrame) -> Result<Output, String> {
        match (self.role, self.stage, frame.phase.as_str()) {
            (Role::Sink, Stage::Accepted, "value") => self.sink_admit_value(frame),
            (Role::Source, Stage::Offered, "accepted") => self.source_accept(frame),
            (Role::Source, Stage::Accepted, "delivered") => self.source_delivered(frame),
            (Role::Sink, Stage::WaitingClose, "close") => self.sink_close(frame),
            (Role::Source, Stage::Closing, "terminal") => self.source_terminal(frame),
            _ => Err("multi-Host Line frame arrived in the wrong exact lifecycle phase".into()),
        }
    }

    pub(super) fn complete_manifestation(&mut self) -> Result<Output, String> {
        if self.role != Role::Sink || self.stage != Stage::Presenting {
            return Err("multi-Host presentation completion arrived in the wrong phase".into());
        }
        let pending = self
            .pending
            .take()
            .ok_or_else(|| "multi-Host sink has no pending presentation".to_string())?;
        engine::complete_host_effect(&mut self.scheduler, &pending)?;
        self.stage = Stage::WaitingClose;
        Ok(Output::Line {
            schema: "conduit.tour/browser-memory-line-effect@1",
            frame: Box::new(self.frame("delivered", 0, Vec::new())),
            plan_projection: None,
            receipt: None,
        })
    }

    pub(super) fn cancel(&mut self) -> Result<Output, String> {
        if self.stage != Stage::Complete && self.stage != Stage::Cancelled {
            self.scheduler
                .cancel()
                .map_err(|error| format!("cancel multi-Host scheduler: {error:?}"))?;
        }
        self.stage = Stage::Cancelled;
        Ok(Output::Receipt {
            schema: "conduit.tour/multi-host-progress@1",
            receipt: Box::new(self.receipt("cancelled")),
        })
    }

    fn source_offer(&mut self) -> Result<Output, String> {
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
                if offer.sequence != 0 {
                    return Err("two-browser lesson emitted more than one value".into());
                }
                let payload = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(debug_error)?
                    .to_vec();
                return Ok(Output::Line {
                    schema: "conduit.tour/browser-memory-line-effect@1",
                    frame: Box::new(self.frame("value", offer.sequence, payload)),
                    plan_projection: Some(Box::new(self.projection.clone())),
                    receipt: None,
                });
            }
            match self.scheduler.step().map_err(debug_error)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    return Err("multi-Host source became idle before offering its value".into())
                }
                SchedulerStatus::Complete => {
                    return Err("multi-Host source completed before offering its value".into())
                }
                SchedulerStatus::Cancelled => return Err("multi-Host source was cancelled".into()),
            }
        }
    }

    fn sink_admit_value(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "value", true)?;
        let remote = self.remote();
        let (endpoint, cord) = (remote.endpoint, remote.cord);
        let admission = self
            .scheduler
            .admit_remote_input(endpoint, cord, frame.sequence, &frame.payload)
            .map_err(debug_error)?;
        if !matches!(
            admission,
            RemoteIngressOutcome::Accepted { sequence: 0, .. }
        ) {
            return Err("one-slot browser-memory Line refused its first admitted value".into());
        }
        let pending = match engine::drive(&mut self.scheduler, &self.fragment)? {
            DriveStatus::Effect(pending) => pending,
            DriveStatus::Complete => {
                return Err("multi-Host sink completed before presentation".into())
            }
            DriveStatus::Waiting { .. } => {
                return Err("multi-Host sink awaits a pending effect".into())
            }
        };
        if !matches!(pending.effect, BrowserHostEffect::Manifestation(_)) {
            return Err("multi-Host sink requested a non-presentation Host effect".into());
        }
        let manifestation = self.project_manifestation(&pending)?;
        self.pending = Some(pending);
        self.stage = Stage::Presenting;
        Ok(Output::Manifestation {
            schema: "conduit.tour/multi-host-manifestation@1",
            manifestation: Box::new(manifestation),
            accepted_frame: Box::new(self.frame("accepted", 0, Vec::new())),
            plan_projection: Box::new(self.projection.clone()),
        })
    }

    fn source_accept(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "accepted", false)?;
        let remote = self.remote();
        let (endpoint, cord) = (remote.endpoint, remote.cord);
        self.scheduler
            .remote_egress_accept(endpoint, cord, 0)
            .map_err(debug_error)?;
        self.stage = Stage::Accepted;
        Ok(Output::Waiting {
            schema: "conduit.tour/multi-host-progress@1",
            phase: "accepted-awaiting-delivery",
            plan_id: self.fragment.plan_id.as_str().into(),
        })
    }

    fn source_delivered(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "delivered", false)?;
        let (endpoint, cord) = {
            let remote = self.remote();
            (remote.endpoint, remote.cord)
        };
        self.scheduler
            .remote_egress_delivered(endpoint, cord, 0)
            .map_err(debug_error)?;
        self.drive_to_complete()?;
        if !self
            .scheduler
            .remote_egress_terminal(endpoint, cord)
            .map_err(debug_error)?
        {
            return Err("multi-Host source egress is not terminal".into());
        }
        self.stage = Stage::Closing;
        Ok(Output::Line {
            schema: "conduit.tour/browser-memory-line-effect@1",
            frame: Box::new(self.frame("close", 1, Vec::new())),
            plan_projection: None,
            receipt: None,
        })
    }

    fn sink_close(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "close", false)?;
        let remote = self.remote();
        let (endpoint, cord) = (remote.endpoint, remote.cord);
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(debug_error)?;
        self.drive_to_complete()?;
        self.stage = Stage::Complete;
        Ok(Output::Line {
            schema: "conduit.tour/browser-memory-line-effect@1",
            frame: Box::new(self.frame("terminal", 1, Vec::new())),
            plan_projection: None,
            receipt: Some(Box::new(self.receipt("completed"))),
        })
    }

    fn source_terminal(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "terminal", false)?;
        self.stage = Stage::Complete;
        Ok(Output::Receipt {
            schema: "conduit.tour/multi-host-progress@1",
            receipt: Box::new(self.receipt("completed")),
        })
    }

    fn drive_to_complete(&mut self) -> Result<(), String> {
        loop {
            if self.scheduler.next_host_request().is_some() {
                return Err("multi-Host terminal path retained an unexpected Host effect".into());
            }
            match self.scheduler.step().map_err(debug_error)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => return Ok(()),
                SchedulerStatus::Idle => {
                    return Err("multi-Host fragment became idle before terminal truth".into())
                }
                SchedulerStatus::Cancelled => {
                    return Err("multi-Host fragment was cancelled before terminal truth".into())
                }
            }
        }
    }

    fn project_manifestation(&mut self, pending: &PendingHostEffect) -> Result<TourEffect, String> {
        let BrowserHostEffect::Manifestation(manifestation) = &pending.effect else {
            return Err("multi-Host pending effect is not a manifestation".into());
        };
        let placement = self
            .fragment
            .placements
            .get(usize::from(pending.request.node.0))
            .ok_or_else(|| "multi-Host presentation has no planned placement".to_string())?;
        let observation_sequence = pending.request.request.0;
        let presentation = bind_presentation(
            &self.sink_active_play_id,
            &placement.placement_id,
            u64::from(observation_sequence),
        );
        let (unit_millis, segments, text) = decode_manifestation(manifestation)?;
        self.latest_presentation = Some(presentation.clone());
        Ok(TourEffect {
            schema: "conduit.tour/manifestation-effect@3",
            effect_kind: "manifestation",
            source_document_id: self.fragment.source_document_id.as_str().into(),
            checked_form_id: self.fragment.checked_form_id.as_str().into(),
            expanded_form_id: self.fragment.expanded_form_id.as_str().into(),
            plan_id: self.fragment.plan_id.as_str().into(),
            fragment_id: self.fragment.fragment_id.as_str().into(),
            active_play_id: self.sink_active_play_id.as_str().into(),
            presentation_id: presentation.presentation_id.as_str().into(),
            placement_id: placement.placement_id.as_str().into(),
            host_id: self.fragment.host_id.as_str().into(),
            boot_id: self.fragment.boot_id.as_str().into(),
            presentation_kind: manifestation.kind_id.into(),
            observation_sequence,
            realization: "direct",
            expanded_gears: self
                .projection
                .hosts
                .iter()
                .flat_map(|host| &host.gears)
                .map(|gear| TourGearEvidence {
                    gear_id: gear.gear_id.clone(),
                    kind_id: gear.kind_id.clone(),
                    implementation_id: gear.implementation_id.clone(),
                })
                .collect(),
            realization_backs: Vec::<TourBackEvidence>::new(),
            unit_millis,
            segments,
            text,
            source_interaction: Some(self.source_interaction.clone()),
        })
    }

    fn validate_frame(
        &self,
        frame: &LineFrame,
        phase: &str,
        allow_payload: bool,
    ) -> Result<(), String> {
        let expected = self.frame(phase, frame.sequence, frame.payload.clone());
        if frame != &expected
            || frame.sequence > 1
            || (!allow_payload && !frame.payload.is_empty())
            || (allow_payload && frame.payload.is_empty())
        {
            return Err("multi-Host Line frame does not match the exact planned identity".into());
        }
        Ok(())
    }

    fn frame(&self, phase: &str, sequence: u64, payload: Vec<u8>) -> LineFrame {
        let remote = self.remote();
        LineFrame {
            schema: "conduit.tour/browser-memory-line-frame@1".into(),
            phase: phase.into(),
            plan_id: self.fragment.plan_id.as_str().into(),
            source_fragment_id: remote.source_fragment_id.as_str().into(),
            sink_fragment_id: remote.sink_fragment_id.as_str().into(),
            source_host_id: remote.line.binding.source.host_id.as_str().into(),
            source_boot_id: remote.line.binding.source.boot_id.as_str().into(),
            source_active_play_id: self.source_active_play_id.as_str().into(),
            sink_host_id: remote.line.binding.sink.host_id.as_str().into(),
            sink_boot_id: remote.line.binding.sink.boot_id.as_str().into(),
            sink_active_play_id: self.sink_active_play_id.as_str().into(),
            source_endpoint_id: remote.line.binding.source.endpoint_id.as_str().into(),
            sink_endpoint_id: remote.line.binding.sink.endpoint_id.as_str().into(),
            connection_id: remote.connection_id.as_str().into(),
            line_id: remote.line.line_id.as_str().into(),
            link_binding_id: remote.line.binding.binding_id.as_str().into(),
            base_implementation_id: remote.line.binding.base.as_str().into(),
            base_instance_id: remote.line.binding.base_instance_id.as_str().into(),
            sequence,
            value_kind: remote.value_kind.as_str().into(),
            payload,
        }
    }

    fn receipt(&self, disposition: &'static str) -> MultiHostReceipt {
        let active = match self.role {
            Role::Source => &self.source_active_play_id,
            Role::Sink => &self.sink_active_play_id,
        };
        let sign = bind_sign(
            &self.fragment.host_id,
            &self.fragment.boot_id,
            Some(active),
            0,
        );
        MultiHostReceipt {
            schema: "conduit.tour/multi-host-receipt@1",
            disposition,
            plan_id: self.fragment.plan_id.as_str().into(),
            fragment_id: self.fragment.fragment_id.as_str().into(),
            active_play_id: active.as_str().into(),
            host_id: self.fragment.host_id.as_str().into(),
            boot_id: self.fragment.boot_id.as_str().into(),
            terminal_sign_id: sign.sign_id.as_str().into(),
            transferred_values: u32::from(disposition == "completed"),
        }
    }

    fn remote(&self) -> &conduit_plan_lowering::lowering::LoweredRemoteEndpoint {
        &self.lowered.remote_endpoints[0]
    }
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
