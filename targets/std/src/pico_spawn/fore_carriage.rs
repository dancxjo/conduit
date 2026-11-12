//! One owned CDC socket, bounded existing CNDF sessions; no execution authority.
use super::{preparation::Frames, PicoSpawnSocket, MAX_PICO_ADMISSION_FRAME_BYTES};
use conduit_wire::{
    decode_external_fore_session_frame, encode_external_fore_session_frame_into,
    ExternalForeSessionBinding as Binding, ExternalForeSessionFrame as Frame,
    ExternalForeSessionMachine as Machine, ExternalForeSessionMessage as Message, SessionRole,
    WireError,
};
use std::time::Duration;

#[derive(Clone, Copy)]
pub struct ForeCarriageBounds {
    /// Total attempted sent/received frames, shared across both directions.
    pub maximum_frames: u32,
    pub maximum_payload_bytes: u32,
    pub maximum_frame_bytes: u32,
    /// Cold encoding storage ceiling; no buffer grows during carriage.
    pub maximum_buffer_bytes: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ForeCarriageError {
    Bounds,
    Scope,
    Wire(WireError),
    /// IO may have partially transferred a frame; no rollback is implied.
    OutcomeUnknown,
    Retired,
}
/// Borrowed exact decoded frame after scope and Wire lifecycle admission.
/// An Offered payload remains application bytes, not a committed observation.
pub struct ForeCarriageFrame<'a> {
    pub route: usize,
    pub frame: Frame<'a>,
}
/// A consuming transition retains the original socket and its sole reader.
/// Preparation/Start and original scope derivation happen independently before
/// the caller uses this carriage. Conversion proves neither Start nor a grant.
pub struct PicoForeCarriage {
    inner: Carriage<super::NativePathCdcLine>,
}
/// Independently checked cold metadata and fixed storage, with no socket or IO.
/// Preparation must precede the consuming transition so refusal preserves the
/// original socket and its sole reader without moving a large socket in errors.
pub struct PreparedForeCarriage {
    state: State,
}
impl PreparedForeCarriage {
    pub fn prepare(
        bindings: [Binding; 2],
        bounds: ForeCarriageBounds,
    ) -> Result<Self, ForeCarriageError> {
        Ok(Self {
            state: State::prepare(bindings, bounds)?,
        })
    }
}
impl PicoSpawnSocket {
    /// Route 0 sends requests; route 1 receives results. Bindings come from
    /// original verified material, never incoming Hello. This moves the same
    /// descriptor and existing framing decoder, without reopen or second reader.
    pub fn into_fore_carriage(self, prepared: PreparedForeCarriage) -> PicoForeCarriage {
        PicoForeCarriage {
            inner: Carriage {
                frames: self.line,
                state: prepared.state,
            },
        }
    }
}
impl PicoForeCarriage {
    /// One explicit frame, with no automatic retry, Ready or acknowledgment.
    /// A successful write is never proof of receiver acceptance or durability.
    pub fn send(
        &mut self,
        route: usize,
        message: Message<'_>,
        timeout: Duration,
    ) -> Result<(), ForeCarriageError> {
        self.inner.send(route, message, timeout)
    }
    /// One explicit receive into caller storage. Caller controls application
    /// acceptance and delivery acknowledgments; they are not durable commit.
    /// Output is untrusted raw frame storage until this method succeeds.
    pub fn receive<'a>(
        &mut self,
        output: &'a mut [u8],
        timeout: Duration,
    ) -> Result<ForeCarriageFrame<'a>, ForeCarriageError> {
        self.inner.receive(output, timeout)
    }
    pub fn is_active(&self) -> bool {
        self.inner.state.machines.iter().all(Machine::is_active) && !self.inner.state.retired
    }
    pub fn frames_used(&self) -> u32 {
        self.inner.state.used_frames
    }
    /// Local retirement only. The Root actor currently refuses terminal control;
    /// no remote cancellation, grant revocation or kernel rollback is invented.
    pub fn cancel(&mut self) {
        self.inner.state.retired = true;
    }
}
struct State {
    machines: [Machine; 2],
    bounds: ForeCarriageBounds,
    buffer: Box<[u8]>,
    used_frames: u32,
    retired: bool,
}
impl State {
    fn prepare(
        bindings: [Binding; 2],
        bounds: ForeCarriageBounds,
    ) -> Result<Self, ForeCarriageError> {
        if bounds.maximum_frames == 0
            || bounds.maximum_payload_bytes == 0
            || bounds.maximum_frame_bytes == 0
            || bounds.maximum_frame_bytes as usize > MAX_PICO_ADMISSION_FRAME_BYTES
            || bounds.maximum_frame_bytes as usize > bounds.maximum_buffer_bytes
            || bindings[0].scope_commitment == bindings[1].scope_commitment
        {
            return Err(ForeCarriageError::Bounds);
        }
        for binding in &bindings {
            binding.validate().map_err(ForeCarriageError::Wire)?;
            if binding.maximum_frame_bytes > bounds.maximum_frame_bytes
                || binding.limits.maximum_payload_bytes > bounds.maximum_payload_bytes
                || binding.limits.maximum_in_flight_items != 1
                || binding.limits.maximum_buffered_bytes as usize > bounds.maximum_buffer_bytes
            {
                return Err(ForeCarriageError::Bounds);
            }
        }
        let [request, result] = bindings;
        Ok(Self {
            machines: [
                Machine::new(request, SessionRole::Source).map_err(ForeCarriageError::Wire)?,
                Machine::new(result, SessionRole::Sink).map_err(ForeCarriageError::Wire)?,
            ],
            bounds,
            buffer: vec![0; bounds.maximum_frame_bytes as usize].into_boxed_slice(),
            used_frames: 0,
            retired: false,
        })
    }
    fn check(&self, timeout: Duration) -> Result<(), ForeCarriageError> {
        if self.retired {
            return Err(ForeCarriageError::Retired);
        }
        if timeout.is_zero() || self.used_frames >= self.bounds.maximum_frames {
            return Err(ForeCarriageError::Bounds);
        }
        Ok(())
    }
}
struct Carriage<F> {
    frames: F,
    state: State,
}
impl<F: Frames> Carriage<F> {
    fn cancel(&mut self) {
        self.state.retired = true;
    }
    fn send(
        &mut self,
        route: usize,
        message: Message<'_>,
        timeout: Duration,
    ) -> Result<(), ForeCarriageError> {
        let result = self.send_inner(route, message, timeout);
        if result.is_err() {
            self.state.retired = true;
        }
        result
    }
    fn send_inner(
        &mut self,
        route: usize,
        message: Message<'_>,
        timeout: Duration,
    ) -> Result<(), ForeCarriageError> {
        self.state.check(timeout)?;
        let machine = self
            .state
            .machines
            .get_mut(route)
            .ok_or(ForeCarriageError::Scope)?;
        let binding = machine.binding();
        let frame = binding.frame(message);
        let n = encode_external_fore_session_frame_into(
            frame,
            &mut self.state.buffer,
            binding.limits.maximum_payload_bytes,
            binding.maximum_frame_bytes,
        )
        .map_err(ForeCarriageError::Wire)?;
        machine
            .admit_outbound(frame)
            .map_err(ForeCarriageError::Wire)?;
        self.state.used_frames += 1;
        self.frames
            .send(&self.state.buffer[..n], timeout)
            .map_err(|_| ForeCarriageError::OutcomeUnknown)
    }
    fn receive<'a>(
        &mut self,
        output: &'a mut [u8],
        timeout: Duration,
    ) -> Result<ForeCarriageFrame<'a>, ForeCarriageError> {
        let result = self.receive_inner(output, timeout);
        if result.is_err() {
            self.state.retired = true;
        }
        result
    }
    fn receive_inner<'a>(
        &mut self,
        output: &'a mut [u8],
        timeout: Duration,
    ) -> Result<ForeCarriageFrame<'a>, ForeCarriageError> {
        self.state.check(timeout)?;
        let maximum = self.state.bounds.maximum_frame_bytes as usize;
        if output.len() < maximum {
            return Err(ForeCarriageError::Bounds);
        }
        self.state.used_frames += 1;
        let bytes = self
            .frames
            .receive(&mut output[..maximum], timeout)
            .map_err(|_| ForeCarriageError::OutcomeUnknown)?;
        let frame = decode_external_fore_session_frame(
            bytes,
            self.state.bounds.maximum_payload_bytes,
            self.state.bounds.maximum_frame_bytes,
        )
        .map_err(ForeCarriageError::Wire)?;
        let route = self
            .state
            .machines
            .iter()
            .position(|m| m.binding().scope_commitment == frame.scope_commitment)
            .ok_or(ForeCarriageError::Scope)?;
        let machine = &mut self.state.machines[route];
        if bytes.len() > machine.binding().maximum_frame_bytes as usize {
            return Err(ForeCarriageError::Bounds);
        }
        machine
            .admit_inbound(frame)
            .map_err(ForeCarriageError::Wire)?;
        Ok(ForeCarriageFrame { route, frame })
    }
}
#[cfg(test)]
mod tests;
