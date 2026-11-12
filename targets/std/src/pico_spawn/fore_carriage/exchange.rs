//! One bounded opaque request/result. Application consumption precedes ACK.
use super::*;
use conduit_wire::SessionMessage;
use std::time::Instant;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeExchangeOutcome {
    /// Both transport deliveries completed. This proves no durable recording.
    Delivered,
    /// Request was not accepted. No automatic retry or result ACK was sent.
    Pressure,
}
impl<F: Frames> Carriage<F> {
    pub(super) fn exchange(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        timeout: Duration,
        consume: impl FnOnce(&[u8]) -> Result<(), ()>,
    ) -> Result<ForeExchangeOutcome, ForeCarriageError> {
        self.state.check(timeout)?;
        // All six frames must fit before the first request is attempted:
        // Offered, Accepted, Delivered, result Offered, Accepted, Delivered.
        if !self.state.machines.iter().all(Machine::is_active)
            || output.len() < self.state.bounds.maximum_frame_bytes as usize
            || input.len()
                > self.state.machines[0]
                    .binding()
                    .limits
                    .maximum_payload_bytes as usize
            || self.state.bounds.maximum_frames - self.state.used_frames < 6
        {
            return Err(ForeCarriageError::Bounds);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ForeCarriageError::Bounds)?;
        let result = self.exchange_inner(input, output, deadline, consume);
        if result.is_err() {
            self.cancel();
        }
        result
    }
    fn exchange_inner(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        deadline: Instant,
        consume: impl FnOnce(&[u8]) -> Result<(), ()>,
    ) -> Result<ForeExchangeOutcome, ForeCarriageError> {
        let left = || {
            deadline
                .checked_duration_since(Instant::now())
                .filter(|t| !t.is_zero())
                .ok_or(ForeCarriageError::OutcomeUnknown)
        };
        let request = self.state.machines[0].next_sequence();
        let result = self.state.machines[1].next_sequence();
        self.send(
            0,
            Message::Message(SessionMessage::Offered {
                sequence: request,
                payload: input,
            }),
            left()?,
        )?;
        let mut consume = Some(consume);
        let mut delivered = false;
        let mut consumed = false;
        // Serial request/result profile: input delivery precedes result offer.
        // Streaming/interleaved profiles use the explicit send/receive API.
        for _ in 0..3 {
            let frame = self.receive(output, left()?)?;
            match (frame.route, frame.frame.message) {
                (0, Message::Message(SessionMessage::Pressure { sequence }))
                    if sequence == request =>
                {
                    // Pressure after a result cannot establish an unaccepted request.
                    if consumed {
                        return Err(ForeCarriageError::Scope);
                    }
                    return Ok(ForeExchangeOutcome::Pressure);
                }
                (0, Message::Message(SessionMessage::Accepted { sequence }))
                    if sequence == request => {}
                (0, Message::Message(SessionMessage::Delivered { sequence }))
                    if sequence == request =>
                {
                    delivered = true;
                }
                (1, Message::Message(SessionMessage::Offered { sequence, payload }))
                    if sequence == result && delivered && !consumed =>
                {
                    consume.take().ok_or(ForeCarriageError::Scope)?(payload)
                        .map_err(|_| ForeCarriageError::ConsumerRefused)?;
                    // The callback has returned and holds no buffer loan. No
                    // durable commit is inferred from its application delivery.
                    self.send(
                        1,
                        Message::Message(SessionMessage::Accepted { sequence }),
                        left()?,
                    )?;
                    self.send(
                        1,
                        Message::Message(SessionMessage::Delivered { sequence }),
                        left()?,
                    )?;
                    consumed = true;
                }
                _ => return Err(ForeCarriageError::Scope),
            }
            if delivered && consumed {
                return Ok(ForeExchangeOutcome::Delivered);
            }
        }
        Err(ForeCarriageError::Scope)
    }
}
