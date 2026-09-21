//! Live one-value kernel/session delivery into the planned HTML renderer Host.

mod sink;
mod wire;

use sink::Sink;
use wire::*;

use crate::{RendererSnapshot, SnapshotError};
use conduit_core::{bind_active_play, BootId, HostId, PlanFragment, SignId};
use conduit_kernel::scheduler::{
    FixedScheduler, HostCallRequest, RemoteIngressOutcome, SchedulerStatus, StepInputBytes, StepIo,
    StepOperation, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, CordId, Failure, FailureCode, FixedHostCallBindings, FixedRoutes,
    HostCallDisposition, HostCallId, HostCallOutcome, HostedSignLog, HostedValueStore, PortId,
    RemoteEndpointId, RequestId, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection,
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_presentation::{Presentation, MAX_RENDERER_VALUE_BYTES};
use conduit_std_host::websocket::{NativeWebSocketLine, NativeWebSocketListener};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};
use patchbay_model::{
    cross_host_renderer_plan, RendererAdapterIdentity, RendererExecution,
    CROSS_HOST_MAXIMUM_FRAME_BYTES, PRESENTATION_PROJECT_KIND,
};
use std::net::TcpStream;
use tungstenite::client::connect_with_config;
use tungstenite::protocol::{Message, WebSocket, WebSocketConfig};
use tungstenite::stream::MaybeTlsStream;

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const SIGN_ITEMS: u16 = 64;
const FRAME_BYTES: usize = CROSS_HOST_MAXIMUM_FRAME_BYTES as usize;

type SourceScheduler =
    FixedScheduler<ProjectBack, HostedValueStore, HostedSignLog, 1, 1, PORTS, 1, PORTS, 1>;

#[derive(Debug)]
pub enum CrossHostRendererError {
    Plan(String),
    Kernel(String),
    Session(String),
    Line(String),
    Presentation(String),
    Snapshot(SnapshotError),
    Worker,
}

impl core::fmt::Display for CrossHostRendererError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "cross-host renderer failed: {self:?}")
    }
}

impl std::error::Error for CrossHostRendererError {}

impl From<SnapshotError> for CrossHostRendererError {
    fn from(value: SnapshotError) -> Self {
        Self::Snapshot(value)
    }
}

struct ProjectBack {
    value: ValueRef,
    emitted: bool,
}

impl StepOperation<PORTS> for ProjectBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.send(PortId(0), self.value).is_err() {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 1,
            });
        }
        self.emitted = true;
        StepOutcome::Complete
    }
}

struct RenderBack {
    pending: Option<RequestId>,
}

impl StepOperation<PORTS> for RenderBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.pending.is_none() {
            if let Some(value) = io.input(PortId(0)) {
                let request = RequestId(1);
                if io.consume(PortId(0)).is_err()
                    || io
                        .request_host_call(
                            request,
                            HostCallId(0),
                            BoundedValueRef::new(value, MAX_RENDERER_VALUE_BYTES)
                                .expect("planned Presentation value uses its admitted byte bound"),
                        )
                        .is_err()
                {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::InvalidLifecycle,
                        detail: 2,
                    });
                }
                self.pending = Some(request);
                return StepOutcome::Progress;
            }
            return StepOutcome::Await;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending == Some(request)
                && outcome.disposition == HostCallDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none()
                && io.consume_host_completion().is_ok()
            {
                self.pending = None;
                return StepOutcome::Complete;
            }
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 3,
            });
        }
        StepOutcome::Await
    }
}

struct Source {
    scheduler: SourceScheduler,
    binding: SessionBinding,
    session: SessionMachine,
    endpoint: RemoteEndpointId,
    cord: CordId,
}

pub fn cross_host_demonstration_snapshot() -> Result<RendererSnapshot, CrossHostRendererError> {
    let (presentation, parts) = patchbay_model::portable_demonstration_with_parts_and_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )
    .map_err(CrossHostRendererError::Presentation)?;
    let renderer_identity = RendererAdapterIdentity {
        host_id: HostId::from("patchbay-html/host"),
        boot_id: BootId::from("patchbay-html/boot"),
        target_subject: "patchbay-html/document-0".into(),
    };
    let exact = cross_host_renderer_plan(
        HostId::from("patchbay-presentation/host"),
        BootId::from("patchbay-presentation/boot"),
        renderer_identity.clone(),
    )
    .map_err(CrossHostRendererError::Plan)?;
    let source_fragment = fragment_for(&exact.plan, &exact.source_advertisement.host_id)?.clone();
    let sink_fragment = fragment_for(&exact.plan, &exact.renderer_advertisement.host_id)?.clone();
    let source = Source::prepare(presentation, &source_fragment, &sink_fragment)?;
    let sink = Sink::prepare(exact.plan, &sink_fragment, &source_fragment)?;
    let listener = NativeWebSocketListener::bind_loopback(CROSS_HOST_MAXIMUM_FRAME_BYTES)
        .map_err(|error| CrossHostRendererError::Line(format!("{error:?}")))?;
    let url = listener
        .url()
        .map_err(|error| CrossHostRendererError::Line(format!("{error:?}")))?;
    let worker = std::thread::spawn(move || source.run(listener));
    let result = sink.run(&url, renderer_identity);
    let source_result = worker.join().map_err(|_| CrossHostRendererError::Worker)?;
    source_result?;
    let mut snapshot = result?;
    snapshot.attach_parts(parts)?;
    let navigation =
        patchbay_model::PatchbayNavigationProjection::for_embodied(&snapshot.presentation)
            .map_err(CrossHostRendererError::Presentation)?;
    snapshot.attach_navigation(navigation)?;
    Ok(snapshot)
}

fn fragment_for<'a>(
    plan: &'a conduit_core::Plan,
    host: &HostId,
) -> Result<&'a PlanFragment, CrossHostRendererError> {
    plan.fragments
        .iter()
        .find(|fragment| &fragment.host_id == host)
        .ok_or_else(|| CrossHostRendererError::Plan("planned host fragment missing".into()))
}

fn lowered_remote(
    fragment: &PlanFragment,
    direction: RemoteCordDirection,
) -> Result<
    (
        LoweredPlanFragment,
        SessionBinding,
        RemoteEndpointId,
        CordId,
    ),
    CrossHostRendererError,
> {
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| CrossHostRendererError::Plan(format!("{error:?}")))?;
    if lowered.nodes.len() != 1
        || lowered.cords.len() != 1
        || lowered.remote_endpoints.len() != 1
        || lowered.remote_endpoints[0].direction != direction
    {
        return Err(CrossHostRendererError::Plan(
            "renderer fragment is not one exact remote endpoint".into(),
        ));
    }
    let remote = &lowered.remote_endpoints[0];
    let endpoint = remote.endpoint;
    let cord = remote.cord;
    let connection = fragment
        .connections
        .iter()
        .find(|connection| connection.connection_id == remote.connection_id)
        .ok_or_else(|| CrossHostRendererError::Plan("planned connection missing".into()))?;
    let binding = SessionBinding::from_planned_connection(
        fragment.plan_id.clone(),
        remote.source_fragment_id.clone(),
        remote.sink_fragment_id.clone(),
        connection,
    )
    .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
    Ok((lowered, binding, endpoint, cord))
}

fn sign_log() -> Result<HostedSignLog, CrossHostRendererError> {
    let bytes = u32::from(SIGN_ITEMS)
        .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
        .ok_or_else(|| CrossHostRendererError::Kernel("Sign budget overflow".into()))?;
    let remote_bytes = conduit_kernel::remote_sign_storage_bytes(SIGN_ITEMS)
        .ok_or_else(|| CrossHostRendererError::Kernel("remote Sign budget overflow".into()))?;
    HostedSignLog::new_with_remote_storage(SIGN_ITEMS, bytes, SIGN_ITEMS, remote_bytes)
        .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))
}

fn decode_presentation(bytes: &[u8]) -> Result<Presentation, CrossHostRendererError> {
    if bytes.len() > MAX_RENDERER_VALUE_BYTES as usize {
        return Err(CrossHostRendererError::Presentation(
            "Presentation Info exceeds its planned byte bound".into(),
        ));
    }
    let presentation: Presentation = serde_json::from_slice(bytes)
        .map_err(|error| CrossHostRendererError::Presentation(error.to_string()))?;
    presentation
        .validate()
        .map_err(|error| CrossHostRendererError::Presentation(error.to_string()))?;
    Ok(presentation)
}

impl Source {
    fn prepare(
        presentation: Presentation,
        fragment: &PlanFragment,
        _sink: &PlanFragment,
    ) -> Result<Self, CrossHostRendererError> {
        if fragment.placements[0].kind_id.as_str() != PRESENTATION_PROJECT_KIND {
            return Err(CrossHostRendererError::Plan(
                "source placement is not the Presentation projector".into(),
            ));
        }
        let (lowered, binding, endpoint, cord) =
            lowered_remote(fragment, RemoteCordDirection::Egress)?;
        let encoded = serde_json::to_vec(&presentation)
            .map_err(|error| CrossHostRendererError::Presentation(error.to_string()))?;
        if encoded.len() > MAX_RENDERER_VALUE_BYTES as usize {
            return Err(CrossHostRendererError::Presentation(
                "encoded Presentation exceeds planned Info bound".into(),
            ));
        }
        let mut values =
            HostedValueStore::new(1, MAX_RENDERER_VALUE_BYTES, MAX_RENDERER_VALUE_BYTES)
                .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let value = values
            .store(&encoded)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let mut routes = FixedRoutes::<PORTS, 1>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        }
        routes
            .seal()
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let scheduler = SourceScheduler::new(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| CrossHostRendererError::Kernel("source node table width".into()))?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| CrossHostRendererError::Kernel("source Cord table width".into()))?,
            routes,
            [ProjectBack {
                value,
                emitted: false,
            }],
            values,
            sign_log()?,
        )
        .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let session = SessionMachine::new(binding.clone(), SessionRole::Source)
            .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        if active_play.active_play_id != binding.source_active_play_id {
            return Err(CrossHostRendererError::Session(
                "source Play identity disagrees with the planned session".into(),
            ));
        }
        Ok(Self {
            scheduler,
            binding,
            session,
            endpoint,
            cord,
        })
    }

    fn run(mut self, listener: NativeWebSocketListener) -> Result<(), CrossHostRendererError> {
        let mut line = listener
            .accept()
            .map_err(|error| CrossHostRendererError::Line(format!("{error:?}")))?;
        let mut input = vec![0; FRAME_BYTES];
        let mut output = vec![0; FRAME_BYTES];
        expect_message(
            receive_native(&mut line, &mut self.session, &mut input)?,
            |message| matches!(message, SessionMessage::Hello(_)),
            "Hello",
        )?;
        let hello_binding = self.binding.clone();
        let hello = hello_binding.hello_frame().message;
        send_native(
            &mut line,
            &mut self.session,
            &self.binding,
            hello,
            &mut output,
        )?;
        expect_message(
            receive_native(&mut line, &mut self.session, &mut input)?,
            |message| matches!(message, SessionMessage::Ready),
            "Ready",
        )?;
        send_native(
            &mut line,
            &mut self.session,
            &self.binding,
            SessionMessage::Ready,
            &mut output,
        )?;
        self.scheduler
            .step()
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let offer = self
            .scheduler
            .remote_egress_offer(self.endpoint, self.cord)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?
            .ok_or_else(|| CrossHostRendererError::Kernel("source omitted Presentation".into()))?;
        let payload = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?
            .to_vec();
        send_native(
            &mut line,
            &mut self.session,
            &self.binding,
            SessionMessage::Offered {
                sequence: offer.sequence,
                payload: &payload,
            },
            &mut output,
        )?;
        expect_sequence(
            receive_native(&mut line, &mut self.session, &mut input)?,
            offer.sequence,
            true,
        )?;
        self.scheduler
            .remote_egress_accept(self.endpoint, self.cord, offer.sequence)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        expect_sequence(
            receive_native(&mut line, &mut self.session, &mut input)?,
            offer.sequence,
            false,
        )?;
        self.scheduler
            .remote_egress_delivered(self.endpoint, self.cord, offer.sequence)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let status = self
            .scheduler
            .step()
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        if status != SchedulerStatus::Drained
            || self.scheduler.values().used_items() != 0
            || self
                .scheduler
                .cord_usage(self.cord)
                .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?
                != (0, 0)
        {
            return Err(CrossHostRendererError::Kernel(
                "source kernel did not finish with empty admitted storage".into(),
            ));
        }
        if !self
            .scheduler
            .remote_egress_terminal(self.endpoint, self.cord)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?
        {
            return Err(CrossHostRendererError::Kernel(
                "source Cord did not reach terminal".into(),
            ));
        }
        send_native(
            &mut line,
            &mut self.session,
            &self.binding,
            SessionMessage::InputClosed { final_sequence: 1 },
            &mut output,
        )?;
        expect_message(
            receive_native(&mut line, &mut self.session, &mut input)?,
            |message| {
                matches!(
                    message,
                    SessionMessage::Terminal {
                        disposition: SessionTerminalDisposition::Completed,
                        final_sequence: 1
                    }
                )
            },
            "completed terminal",
        )?;
        send_native(
            &mut line,
            &mut self.session,
            &self.binding,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: 1,
            },
            &mut output,
        )
    }
}

#[cfg(test)]
mod tests;
