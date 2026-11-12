//! Explicit bounded handshake; no discovery, scope adoption or automatic retry.
use super::*;
use conduit_wire::SessionMessage;
use std::time::Instant;
impl<F: Frames> Carriage<F> {
    pub(super) fn handshake(
        &mut self,
        output: &mut [u8],
        timeout: Duration,
    ) -> Result<(), ForeCarriageError> {
        self.state.check(timeout)?;
        if self.state.used_frames != 0 {
            return Err(ForeCarriageError::Scope);
        }
        // Two Hello/Ready exchanges each need two sends and two receives. Do
        // not begin a protocol whose declared frame/storage budgets cannot fit.
        if self.state.bounds.maximum_frames < 8
            || output.len() < self.state.bounds.maximum_frame_bytes as usize
        {
            return Err(ForeCarriageError::Bounds);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ForeCarriageError::Bounds)?;
        let result = self.handshake_inner(output, deadline);
        if result.is_err() {
            self.cancel();
        }
        result
    }
    fn handshake_inner(
        &mut self,
        output: &mut [u8],
        deadline: Instant,
    ) -> Result<(), ForeCarriageError> {
        let left = || {
            deadline
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .ok_or(ForeCarriageError::OutcomeUnknown)
        };
        // Both directions may reply in either interleaving on the same stream.
        for route in 0..2 {
            self.send(route, Message::Hello, left()?)?;
        }
        let mut hello = [false; 2];
        let mut ready = [false; 2];
        for _ in 0..4 {
            let frame = self.receive(output, left()?)?;
            let route = frame.route;
            match frame.frame.message {
                Message::Hello if !hello[route] => {
                    hello[route] = true;
                    self.send(route, Message::Message(SessionMessage::Ready), left()?)?;
                }
                Message::Message(SessionMessage::Ready) if hello[route] && !ready[route] => {
                    ready[route] = true
                }
                _ => return Err(ForeCarriageError::Scope),
            }
        }
        if !hello.into_iter().all(|v| v)
            || !ready.into_iter().all(|v| v)
            || !self.state.machines.iter().all(Machine::is_active)
        {
            return Err(ForeCarriageError::Scope);
        }
        Ok(())
    }
}
