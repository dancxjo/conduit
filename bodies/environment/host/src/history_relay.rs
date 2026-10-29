//! Two exact planned directional Cords over an existing protected Line.
//! Delivery is not recorder commitment or permission to retire producer evidence.
use crate::{checked, Result};
use conduit_core::{ConnectionId, Plan};
use conduit_environment_runtime::history_exchange::HistoryReadContract;
use conduit_protected_line::{ProtectedCarrier, ProtectedFrameCarrier};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole,
};

const FRAME_BYTES: usize = 4096;

pub(crate) struct HistoryRelay<C> {
    line: ProtectedCarrier<C>,
    request: SessionMachine,
    response: SessionMachine,
    frame: [u8; FRAME_BYTES],
    collector: bool,
}
impl<C: ProtectedFrameCarrier> HistoryRelay<C> {
    /// The composition root supplies a current admitted Plan, never peer-provided
    /// connection names. Membership and observation origin remain independently
    /// verified by the recorder binding and signed batch collector.
    pub(crate) fn from_plan(
        line: ProtectedCarrier<C>,
        plan: &Plan,
        request_id: &ConnectionId,
        response_id: &ConnectionId,
        collector: bool,
    ) -> Result<Self> {
        if !conduit_core::verify_plan(plan) || request_id == response_id {
            return Err("history requires one verified Plan and two exact Cords".into());
        }
        let contract = checked(HistoryReadContract::prepare())?;
        let request = binding(plan, request_id)?;
        let response = binding(plan, response_id)?;
        if request.value_kind != *checked(contract.request_type().profile())?.value_kind()
            || response.value_kind != *checked(contract.result_type().profile())?.value_kind()
            || request.source != response.sink
            || request.sink != response.source
            || request.limits.maximum_payload_bytes > 2304
            || response.limits.maximum_payload_bytes > 2304
        {
            return Err("history Cords differ from checked request/result contract".into());
        }
        let evidence = line.session().evidence();
        let protected = evidence.binding;
        let initiator = &protected.initiator;
        let responder = &protected.responder;
        let local = match evidence.role {
            conduit_protected_line::Role::Initiator => initiator,
            conduit_protected_line::Role::Responder => responder,
        };
        let planned_local = if collector {
            &request.source
        } else {
            &request.sink
        };
        if local.host_id != planned_local.host_id.as_str()
            || local.boot_id != planned_local.boot_id.as_str()
        {
            return Err("history local role differs from the protected Host/Boot".into());
        }
        let endpoints_match =
            |host: &str, boot: &str, endpoint: &conduit_protected_line::EndpointBinding| {
                host == endpoint.host_id && boot == endpoint.boot_id
            };
        if !((endpoints_match(
            request.source.host_id.as_str(),
            request.source.boot_id.as_str(),
            initiator,
        ) && endpoints_match(
            request.sink.host_id.as_str(),
            request.sink.boot_id.as_str(),
            responder,
        )) || (endpoints_match(
            request.source.host_id.as_str(),
            request.source.boot_id.as_str(),
            responder,
        ) && endpoints_match(
            request.sink.host_id.as_str(),
            request.sink.boot_id.as_str(),
            initiator,
        ))) {
            return Err("protected Line endpoints differ from planned history Hosts/Boots".into());
        }
        Ok(Self {
            line,
            request: checked(SessionMachine::new(
                request,
                if collector {
                    SessionRole::Source
                } else {
                    SessionRole::Sink
                },
            ))?,
            response: checked(SessionMachine::new(
                response,
                if collector {
                    SessionRole::Sink
                } else {
                    SessionRole::Source
                },
            ))?,
            frame: [0; FRAME_BYTES],
            collector,
        })
    }
    pub(crate) fn activate(&mut self) -> Result<()> {
        handshake(
            &mut self.line,
            &mut self.request,
            &mut self.frame,
            self.collector,
        )?;
        handshake(
            &mut self.line,
            &mut self.response,
            &mut self.frame,
            !self.collector,
        )
    }
    /// Execute one cursor request and return one canonical result. This method
    /// never signs custody and never acknowledges durable recorder State.
    pub(crate) fn read(&mut self, canonical_request: &[u8]) -> Result<Vec<u8>> {
        if !self.collector {
            return Err("history source cannot issue collector reads".into());
        }
        let request_sequence = self.request.next_sequence();
        send(
            &mut self.line,
            &mut self.request,
            &mut self.frame,
            SessionMessage::Offered {
                sequence: request_sequence,
                payload: canonical_request,
            },
        )?;
        if !matches!(receive(&mut self.line, &mut self.request)?, OwnedMessage::Accepted(s) if s == request_sequence)
        {
            return Err("history cursor not accepted".into());
        }
        let (response_sequence, payload) = receive_offered(&mut self.line, &mut self.response)?;
        send(
            &mut self.line,
            &mut self.response,
            &mut self.frame,
            SessionMessage::Accepted {
                sequence: response_sequence,
            },
        )?;
        send(
            &mut self.line,
            &mut self.response,
            &mut self.frame,
            SessionMessage::Delivered {
                sequence: response_sequence,
            },
        )?;
        if !matches!(receive(&mut self.line, &mut self.request)?, OwnedMessage::Delivered(s) if s == request_sequence)
        {
            return Err("history cursor not consumed".into());
        }
        Ok(payload)
    }
    /// Dispatch one admitted request. Defer request delivery until the response
    /// is delivered: neither direction can burst more than two control frames
    /// into the admitted two-frame relay queue. No retry or extra queue is used.
    pub(crate) fn serve_one(
        &mut self,
        dispatch: impl FnOnce(&[u8]) -> Result<Vec<u8>>,
    ) -> Result<()> {
        if self.collector {
            return Err("collector cannot dispatch history provider".into());
        }
        let (request_sequence, request) = receive_offered(&mut self.line, &mut self.request)?;
        send(
            &mut self.line,
            &mut self.request,
            &mut self.frame,
            SessionMessage::Accepted {
                sequence: request_sequence,
            },
        )?;
        let response = dispatch(&request)?;
        offer(
            &mut self.line,
            &mut self.response,
            &mut self.frame,
            &response,
        )?;
        send(
            &mut self.line,
            &mut self.request,
            &mut self.frame,
            SessionMessage::Delivered {
                sequence: request_sequence,
            },
        )
    }
    pub(crate) fn close(&mut self) -> Result<()> {
        checked(self.line.close())
    }
}
fn binding(plan: &Plan, id: &ConnectionId) -> Result<SessionBinding> {
    let mut matches = plan
        .fragments
        .iter()
        .flat_map(|f| &f.connections)
        .filter(|c| &c.connection_id == id);
    let connection = matches.next().ok_or("missing planned history Cord")?;
    // Remote connections may be repeated in both fragment projections. They
    // must describe the exact same immutable Cord.
    if matches.any(|other| other != connection) {
        return Err("conflicting history Cord projections".into());
    }
    let fragment = |placement: &conduit_core::PlacementId| {
        plan.fragments
            .iter()
            .find(|f| f.placements.iter().any(|g| &g.placement_id == placement))
            .map(|f| f.fragment_id.clone())
            .ok_or("history Cord placement absent")
    };
    checked(SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        fragment(&connection.source_placement_id)?,
        fragment(&connection.sink_placement_id)?,
        connection,
    ))
}
fn send<C: ProtectedFrameCarrier>(
    line: &mut ProtectedCarrier<C>,
    machine: &mut SessionMachine,
    frame: &mut [u8; FRAME_BYTES],
    message: SessionMessage<'_>,
) -> Result<()> {
    let binding = machine.binding().clone();
    checked(machine.admit_outbound(binding.frame(message)))?;
    let length = checked(encode_session_frame_into(
        binding.frame(message),
        frame,
        2304,
        FRAME_BYTES as u32,
    ))?;
    checked(line.send(&frame[..length]))
}
fn receive<C: ProtectedFrameCarrier>(
    line: &mut ProtectedCarrier<C>,
    machine: &mut SessionMachine,
) -> Result<OwnedMessage> {
    let bytes = checked(line.receive())?;
    let frame = checked(decode_session_frame(bytes, 2304, FRAME_BYTES as u32))?;
    checked(machine.admit_inbound(frame))?;
    Ok(match frame.message {
        SessionMessage::Hello(_) => OwnedMessage::Hello,
        SessionMessage::Ready => OwnedMessage::Ready,
        SessionMessage::Offered { sequence, payload } => {
            let mut bounded = [0; 2304];
            if payload.len() > bounded.len() {
                return Err("history payload extent".into());
            }
            bounded[..payload.len()].copy_from_slice(payload);
            OwnedMessage::Offered(sequence, bounded, payload.len())
        }
        SessionMessage::Accepted { sequence } => OwnedMessage::Accepted(sequence),
        SessionMessage::Delivered { sequence } => OwnedMessage::Delivered(sequence),
        _ => return Err("unexpected history Cord lifecycle message".into()),
    })
}
// Fixed scratch bounds keep frame validation independent of peer heap demand.
#[allow(clippy::large_enum_variant)]
enum OwnedMessage {
    Hello,
    Ready,
    Offered(u64, [u8; 2304], usize),
    Accepted(u64),
    Delivered(u64),
}
fn handshake<C: ProtectedFrameCarrier>(
    line: &mut ProtectedCarrier<C>,
    machine: &mut SessionMachine,
    frame: &mut [u8; FRAME_BYTES],
    source: bool,
) -> Result<()> {
    if source {
        // Hello borrows the binding; clone to keep the mutable state separate.
        let binding = machine.binding().clone();
        send(line, machine, frame, binding.hello_frame().message)?;
        if !matches!(receive(line, machine)?, OwnedMessage::Hello) {
            return Err("history peer omitted Hello".into());
        }
        send(line, machine, frame, SessionMessage::Ready)?;
        if !matches!(receive(line, machine)?, OwnedMessage::Ready) {
            return Err("history peer omitted Ready".into());
        }
    } else {
        if !matches!(receive(line, machine)?, OwnedMessage::Hello) {
            return Err("history source omitted Hello".into());
        }
        let binding = machine.binding().clone();
        send(line, machine, frame, binding.hello_frame().message)?;
        if !matches!(receive(line, machine)?, OwnedMessage::Ready) {
            return Err("history source omitted Ready".into());
        }
        send(line, machine, frame, SessionMessage::Ready)?;
    }
    Ok(())
}
fn offer<C: ProtectedFrameCarrier>(
    line: &mut ProtectedCarrier<C>,
    machine: &mut SessionMachine,
    frame: &mut [u8; FRAME_BYTES],
    payload: &[u8],
) -> Result<()> {
    let sequence = machine.next_sequence();
    send(
        line,
        machine,
        frame,
        SessionMessage::Offered { sequence, payload },
    )?;
    if !matches!(receive(line, machine)?, OwnedMessage::Accepted(s) if s == sequence) {
        return Err("history offer not accepted".into());
    }
    if !matches!(receive(line, machine)?, OwnedMessage::Delivered(s) if s == sequence) {
        return Err("history offer not delivered".into());
    }
    Ok(())
}
fn receive_offered<C: ProtectedFrameCarrier>(
    line: &mut ProtectedCarrier<C>,
    machine: &mut SessionMachine,
) -> Result<(u64, Vec<u8>)> {
    let OwnedMessage::Offered(sequence, payload, length) = receive(line, machine)? else {
        return Err("history peer omitted offered value".into());
    };
    Ok((sequence, payload[..length].to_vec()))
}

#[cfg(test)]
mod proof;
