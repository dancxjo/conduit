//! One role of the exact two-browser-Host executable-tour Play.

#[path = "session_source.rs"]
mod source;

use super::plan::PreparedPlan;
use super::protocol::{
    self, LineFrame, MultiHostReceipt, Output, PlanProjection, RecordDeliveryProjection,
    RecordTranscriptEntryProjection, RecordTranscriptProjection,
};
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

#[derive(Clone, Copy)]
pub(super) enum TransportTermination {
    Unavailable,
    Disconnected,
    TimedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Input,
    Offered,
    Accepted,
    Presenting,
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
    sequence: u64,
    transferred_values: u32,
    deliveries: Vec<conduit_net::RecordDeliveryTracker>,
    transcript: Option<SessionTranscript>,
}

struct SessionTranscript {
    history: conduit_net::BoundedRecordTranscript,
    framed_type: Vec<u8>,
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
        let transcript = if &lowered.remote_endpoints[0].value_kind
            == conduit_net::framed_typed_record_type()
                .profile()
                .map_err(debug_error)?
                .value_kind()
        {
            Some(SessionTranscript {
                history: conduit_net::BoundedRecordTranscript::new(
                    16,
                    conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES,
                    16 * conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES,
                    0,
                )
                .map_err(debug_error)?,
                framed_type: conduit_net::framed_typed_record_type()
                    .canonical_bytes()
                    .map_err(debug_error)?,
            })
        } else {
            None
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
            sequence: 0,
            transferred_values: 0,
            deliveries: Vec::with_capacity(
                conduit_net::MAXIMUM_RECORD_DELIVERY_OBSERVATIONS.into(),
            ),
            transcript,
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
            (Role::Sink, Stage::Accepted, "close") => self.sink_close(frame),
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
        self.stage = Stage::Accepted;
        let output = Output::Line {
            schema: "conduit.tour/browser-memory-line-effect@1",
            frame: Box::new(self.frame("delivered", self.sequence, Vec::new())),
            plan_projection: None,
            receipt: None,
        };
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Line sequence exhausted")?;
        self.transferred_values = self
            .transferred_values
            .checked_add(1)
            .ok_or("Line count exhausted")?;
        Ok(output)
    }

    pub(super) fn cancel(&mut self) -> Result<Output, String> {
        let newly_cancelled = self.stage != Stage::Complete && self.stage != Stage::Cancelled;
        if newly_cancelled {
            self.scheduler
                .cancel()
                .map_err(|error| format!("cancel multi-Host scheduler: {error:?}"))?;
        }
        if newly_cancelled {
            if let Some(transcript) = &mut self.transcript {
                transcript
                    .history
                    .terminal(conduit_net::RecordTranscriptTerminal::Cancelled)
                    .map_err(debug_error)?;
            }
        }
        self.stage = Stage::Cancelled;
        Ok(Output::Receipt {
            schema: "conduit.tour/multi-host-progress@1",
            receipt: Box::new(self.receipt("cancelled")),
        })
    }

    pub(super) fn terminate_transport(
        &mut self,
        termination: TransportTermination,
        code: u16,
    ) -> Result<Output, String> {
        if self.stage == Stage::Complete || self.stage == Stage::Cancelled {
            return Err("transport termination arrived after terminal truth".into());
        }
        self.scheduler
            .cancel()
            .map_err(|error| format!("cancel terminated multi-Host scheduler: {error:?}"))?;
        if self.role == Role::Source {
            for delivery in &mut self.deliveries {
                if !delivery.is_terminal() {
                    match termination {
                        TransportTermination::Unavailable => delivery.transport_unavailable(code),
                        TransportTermination::Disconnected => delivery.disconnected(code),
                        TransportTermination::TimedOut => delivery.timed_out(code),
                    }
                    .map_err(debug_error)?;
                }
            }
        }
        let (disposition, terminal) = match termination {
            TransportTermination::Unavailable => (
                "transport-unavailable",
                conduit_net::RecordTranscriptTerminal::TransportUnavailable,
            ),
            TransportTermination::Disconnected => (
                "disconnected",
                conduit_net::RecordTranscriptTerminal::Disconnected,
            ),
            TransportTermination::TimedOut => {
                ("timed-out", conduit_net::RecordTranscriptTerminal::TimedOut)
            }
        };
        self.retain_transcript_terminal(terminal)?;
        self.stage = Stage::Complete;
        Ok(Output::Receipt {
            schema: "conduit.tour/multi-host-progress@1",
            receipt: Box::new(self.receipt(disposition)),
        })
    }

    fn sink_admit_value(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "value", true)?;
        self.retain_line_record(
            conduit_net::RecordTranscriptDirection::Received,
            &frame.payload,
        )?;
        let remote = self.remote();
        let (endpoint, cord) = (remote.endpoint, remote.cord);
        let admission = self
            .scheduler
            .admit_remote_input(endpoint, cord, frame.sequence, &frame.payload)
            .map_err(debug_error)?;
        if !matches!(
            admission,
            RemoteIngressOutcome::Accepted { sequence, .. } if sequence == self.sequence
        ) {
            return Err("one-slot browser-memory Line refused its next admitted value".into());
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
            accepted_frame: Box::new(self.frame("accepted", self.sequence, Vec::new())),
            plan_projection: Box::new(self.projection.clone()),
        })
    }

    fn source_accept(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "accepted", false)?;
        let remote = self.remote();
        let (endpoint, cord) = (remote.endpoint, remote.cord);
        self.scheduler
            .remote_egress_accept(endpoint, cord, self.sequence)
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
            .remote_egress_delivered(endpoint, cord, self.sequence)
            .map_err(debug_error)?;
        let receipt = self.sink_active_play_id.as_str().as_bytes();
        self.deliveries
            .get_mut(usize::try_from(self.sequence).map_err(debug_error)?)
            .ok_or("delivered Line value has no correlated delivery tracker")?
            .remote_accepted(receipt)
            .map_err(debug_error)?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Line sequence exhausted")?;
        self.transferred_values = self
            .transferred_values
            .checked_add(1)
            .ok_or("Line count exhausted")?;
        self.source_offer()
    }

    fn sink_close(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "close", false)?;
        let remote = self.remote();
        let (endpoint, cord) = (remote.endpoint, remote.cord);
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(debug_error)?;
        self.drive_to_complete()?;
        self.retain_transcript_terminal(conduit_net::RecordTranscriptTerminal::Completed)?;
        self.stage = Stage::Complete;
        Ok(Output::Line {
            schema: "conduit.tour/browser-memory-line-effect@1",
            frame: Box::new(self.frame("terminal", self.sequence, Vec::new())),
            plan_projection: None,
            receipt: Some(Box::new(self.receipt("completed"))),
        })
    }

    fn source_terminal(&mut self, frame: LineFrame) -> Result<Output, String> {
        self.validate_frame(&frame, "terminal", false)?;
        self.stage = Stage::Complete;
        self.retain_transcript_terminal(conduit_net::RecordTranscriptTerminal::Completed)?;
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
            || frame.sequence != self.sequence
            || frame.payload.len()
                > self.remote().line.binding.limits.maximum_payload_bytes as usize
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
            transferred_values: self.transferred_values,
            deliveries: self
                .deliveries
                .iter()
                .enumerate()
                .map(|(sequence, tracker)| delivery_projection(sequence as u64, tracker))
                .collect(),
            transcript: self.transcript.as_ref().map(transcript_projection),
        }
    }

    fn remote(&self) -> &conduit_plan_lowering::lowering::LoweredRemoteEndpoint {
        &self.lowered.remote_endpoints[0]
    }

    fn retain_line_record(
        &mut self,
        direction: conduit_net::RecordTranscriptDirection,
        canonical: &[u8],
    ) -> Result<(), String> {
        let Some(transcript) = &mut self.transcript else {
            return Ok(());
        };
        let frame = exact_leaf(canonical, &transcript.framed_type)?;
        transcript
            .history
            .record(direction, frame)
            .map(|_| ())
            .map_err(debug_error)
    }

    fn retain_transcript_terminal(
        &mut self,
        terminal: conduit_net::RecordTranscriptTerminal,
    ) -> Result<(), String> {
        if let Some(transcript) = &mut self.transcript {
            transcript.history.terminal(terminal).map_err(debug_error)?;
        }
        Ok(())
    }
}

fn exact_leaf<'a>(canonical: &'a [u8], value_type: &[u8]) -> Result<&'a [u8], String> {
    let node = canonical
        .strip_prefix(value_type)
        .ok_or("framed Line value has the wrong exact type")?;
    if node.first() != Some(&0) || node.len() < 5 {
        return Err("framed Line value has malformed canonical shape".into());
    }
    let length = usize::try_from(u32::from_le_bytes(
        node[1..5]
            .try_into()
            .map_err(|_| "framed Line leaf length is truncated")?,
    ))
    .map_err(debug_error)?;
    (node.len() == 5 + length)
        .then_some(&node[5..])
        .ok_or_else(|| "framed Line leaf length is not exact".into())
}

fn transcript_projection(transcript: &SessionTranscript) -> RecordTranscriptProjection {
    let entries = (0..transcript.history.len())
        .filter_map(|index| transcript.history.entry(index))
        .map(|entry| {
            let (event, frame_bytes, terminal_code) = match entry.event {
                conduit_net::RecordTranscriptEventRef::Record { direction, frame } => (
                    match direction {
                        conduit_net::RecordTranscriptDirection::Sent => "sent-record",
                        conduit_net::RecordTranscriptDirection::Received => "received-record",
                    },
                    frame.len(),
                    None,
                ),
                conduit_net::RecordTranscriptEventRef::Terminal(terminal) => match terminal {
                    conduit_net::RecordTranscriptTerminal::Completed => ("completed", 0, None),
                    conduit_net::RecordTranscriptTerminal::Cancelled => ("cancelled", 0, None),
                    conduit_net::RecordTranscriptTerminal::TransportUnavailable => {
                        ("transport-unavailable", 0, None)
                    }
                    conduit_net::RecordTranscriptTerminal::Disconnected => {
                        ("disconnected", 0, None)
                    }
                    conduit_net::RecordTranscriptTerminal::TimedOut => ("timed-out", 0, None),
                    conduit_net::RecordTranscriptTerminal::Refused(code) => {
                        ("refused", 0, Some(code))
                    }
                    conduit_net::RecordTranscriptTerminal::Failed(code) => {
                        ("failed", 0, Some(code))
                    }
                },
            };
            RecordTranscriptEntryProjection {
                sequence: entry.sequence,
                event,
                frame_bytes,
                terminal_code,
            }
        })
        .collect();
    RecordTranscriptProjection {
        retained_items: transcript.history.len(),
        retained_bytes: transcript.history.retained_bytes(),
        retention_gap: transcript.history.retention_gap(),
        entries,
    }
}

fn delivery_projection(
    sequence: u64,
    tracker: &conduit_net::RecordDeliveryTracker,
) -> RecordDeliveryProjection {
    use conduit_net::RecordDeliveryStateRef::*;
    let (state, queue_sequence, sent_bytes, receipt, code) = match tracker.state() {
        LocallyAccepted => ("locally-accepted", None, None, None, None),
        FramedQueued { queue_sequence } => {
            ("framed-queued", Some(queue_sequence), None, None, None)
        }
        PartiallySent { sent_bytes, .. } => ("partially-sent", None, Some(sent_bytes), None, None),
        RemoteAccepted { receipt } => ("remote-accepted", None, None, Some(receipt), None),
        TransportUnavailable { code } => ("transport-unavailable", None, None, None, Some(code)),
        Disconnected { code } => ("disconnected", None, None, None, Some(code)),
        TimedOut { code } => ("timed-out", None, None, None, Some(code)),
        Refused { code } => ("refused", None, None, None, Some(code)),
        Failed { code } => ("failed", None, None, None, Some(code)),
    };
    RecordDeliveryProjection {
        sequence,
        correlation_hex: hex(tracker.correlation()),
        frame_bytes: tracker.frame_bytes(),
        state,
        queue_sequence,
        sent_bytes,
        remote_receipt_hex: receipt.map(hex),
        failure_code: code,
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
