//! Bounded protocol and mailbox phases; a sent frame never becomes unsent.
use super::*;
pub(super) struct Route {
    pub endpoint: u16,
    pub cord: u16,
    pub machine: Machine,
    pub hello_sent: bool,
    pub peer_hello: bool,
    pub ready_sent: bool,
}
impl Route {
    pub fn new(endpoint: u16, cord: u16, machine: Machine) -> Result<Self, ForeIoRefusal> {
        Ok(Self {
            endpoint,
            cord,
            machine,
            hello_sent: false,
            peer_hello: false,
            ready_sent: false,
        })
    }
}
#[derive(Clone, Copy)]
pub(super) enum Pending {
    None,
    Sent(Event),
    Input {
        sequence: u64,
        start: usize,
        length: usize,
    },
    Accepted(u64),
    Delivered(u64),
    Pressure(u64),
}
impl<C: MessageTransport> ForeIoActor<'_, '_, '_, C> {
    pub(super) fn check(&self) -> Result<(), ForeIoRefusal> {
        if self.state.retired {
            Err(ForeIoRefusal::Inactive)
        } else if self.state.used_frames >= self.state.maximum_frames {
            Err(ForeIoRefusal::Bounds)
        } else {
            Ok(())
        }
    }
    /// Retire only this mailbox bridge. No native kernel cancellation, Value
    /// release, transport close, reconnection or application retry is invented.
    pub fn retire(&mut self) {
        self.state.retired = true;
        let _ = self.port.retire();
    }
    pub fn frames_used(&self) -> u32 {
        self.state.used_frames
    }
    pub(super) fn apply(&mut self) -> Result<ForeIoProgress, ForeIoRefusal> {
        let result = match self.state.pending {
            Pending::None => return Ok(ForeIoProgress::Idle),
            Pending::Sent(event) => self.port.sent(event),
            Pending::Input {
                sequence,
                start,
                length,
            } => {
                let bytes = self
                    .buffer
                    .get(start..start + length)
                    .ok_or(ForeIoRefusal::Bounds)?;
                self.port.stage_ingress(sequence, bytes)
            }
            Pending::Accepted(s) => self.port.peer_accepted(s),
            Pending::Delivered(s) => self.port.peer_delivered(s),
            Pending::Pressure(s) => self.port.peer_pressure(s),
        };
        match result {
            Ok(()) => {
                self.state.pending = Pending::None;
                Ok(ForeIoProgress::Progress)
            }
            Err(bridge::Refusal::Busy) => Ok(ForeIoProgress::Busy),
            Err(_) => Err(ForeIoRefusal::Bridge),
        }
    }
    pub(super) fn encode(
        &mut self,
        route: usize,
        message: Message<'_>,
    ) -> Result<usize, ForeIoRefusal> {
        let machine = &mut self.state.routes[route].machine;
        let binding = machine.binding();
        let frame = binding.frame(message);
        let length = conduit_wire::encode_external_fore_session_frame_into(
            frame,
            self.buffer,
            binding.limits.maximum_payload_bytes,
            binding.maximum_frame_bytes,
        )
        .map_err(|_| ForeIoRefusal::Wire)?;
        machine
            .admit_outbound(frame)
            .map_err(|_| ForeIoRefusal::Wire)?;
        Ok(length)
    }
    pub(super) fn admit_received(
        &mut self,
        length: usize,
    ) -> Result<ForeIoProgress, ForeIoRefusal> {
        let max_payload = self
            .state
            .routes
            .iter()
            .map(|r| r.machine.binding().limits.maximum_payload_bytes)
            .max()
            .ok_or(ForeIoRefusal::Bounds)?;
        let max_frame = self
            .state
            .routes
            .iter()
            .map(|r| r.machine.binding().maximum_frame_bytes)
            .max()
            .ok_or(ForeIoRefusal::Bounds)?;
        let frame = conduit_wire::decode_external_fore_session_frame(
            &self.buffer[..length],
            max_payload,
            max_frame,
        )
        .map_err(|_| ForeIoRefusal::Wire)?;
        let index = self
            .state
            .routes
            .iter()
            .position(|r| r.machine.binding().scope_commitment == frame.scope_commitment)
            .ok_or(ForeIoRefusal::Scope)?;
        if length
            > self.state.routes[index]
                .machine
                .binding()
                .maximum_frame_bytes as usize
        {
            return Err(ForeIoRefusal::Bounds);
        }
        match frame.message {
            Message::Hello | Message::Message(SessionMessage::Ready) => {}
            Message::Message(SessionMessage::Offered { .. }) if index == 0 => {}
            Message::Message(
                SessionMessage::Accepted { .. }
                | SessionMessage::Delivered { .. }
                | SessionMessage::Pressure { .. },
            ) if index == 1 => {}
            _ => return Err(ForeIoRefusal::Unsupported),
        }
        self.state.routes[index]
            .machine
            .admit_inbound(frame)
            .map_err(|_| ForeIoRefusal::Wire)?;
        self.state.pending = match frame.message {
            Message::Hello => {
                self.state.routes[index].peer_hello = true;
                Pending::None
            }
            Message::Message(SessionMessage::Ready) => Pending::None,
            Message::Message(SessionMessage::Offered { sequence, payload }) => Pending::Input {
                sequence,
                start: length - payload.len(),
                length: payload.len(),
            },
            Message::Message(SessionMessage::Accepted { sequence }) => Pending::Accepted(sequence),
            Message::Message(SessionMessage::Delivered { sequence }) => {
                Pending::Delivered(sequence)
            }
            Message::Message(SessionMessage::Pressure { sequence }) => Pending::Pressure(sequence),
            _ => return Err(ForeIoRefusal::Unsupported),
        };
        Ok(ForeIoProgress::Progress)
    }
}
