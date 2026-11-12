//! One bounded actual frame per call. RefCell/kernel loans always end before IO.
use super::*;
use core::{
    future::{poll_fn, Future},
    pin::pin,
    task::Poll,
};
impl<C: MessageTransport> ForeIoActor<'_, '_, '_, C> {
    /// Send one handshake/mailbox frame, or finish its already-sent mailbox
    /// application. Busy preserves the pending APPLY and NEVER resends a frame.
    pub async fn send_pending(&mut self) -> Result<ForeIoProgress, ForeIoRefusal> {
        let result = self.send_inner().await;
        if result.is_err() {
            self.retire();
        }
        result
    }
    async fn send_inner(&mut self) -> Result<ForeIoProgress, ForeIoRefusal> {
        if self.state.retired {
            return Err(ForeIoRefusal::Inactive);
        }
        // Applying a successfully sent final-budget frame requires no more IO.
        if !matches!(self.state.pending, Pending::None) {
            return self.apply();
        }
        self.check()?;
        for index in 0..2 {
            if !self.state.routes[index].hello_sent {
                let n = self.encode(index, Message::Hello)?;
                self.link
                    .send(&self.buffer[..n])
                    .await
                    .map_err(|_| ForeIoRefusal::Io)?;
                self.state.used_frames += 1;
                self.state.routes[index].hello_sent = true;
                return Ok(ForeIoProgress::Progress);
            }
            if self.state.routes[index].peer_hello && !self.state.routes[index].ready_sent {
                let n = self.encode(index, Message::Message(SessionMessage::Ready))?;
                self.link
                    .send(&self.buffer[..n])
                    .await
                    .map_err(|_| ForeIoRefusal::Io)?;
                self.state.used_frames += 1;
                self.state.routes[index].ready_sent = true;
                return Ok(ForeIoProgress::Progress);
            }
        }
        let event = match self.port.next_event() {
            Ok(Some(event)) => event,
            Ok(None) => return Ok(ForeIoProgress::Idle),
            Err(bridge::Refusal::Busy) => return Ok(ForeIoProgress::Busy),
            Err(_) => return Err(ForeIoRefusal::Bridge),
        };
        let route = if matches!(event, Event::EgressOffered { .. }) {
            1
        } else {
            0
        };
        if !self.state.routes[route].machine.is_active() {
            return Ok(ForeIoProgress::Idle);
        }
        let length = match event {
            Event::IngressAccepted(sequence) => {
                self.encode(0, Message::Message(SessionMessage::Accepted { sequence }))?
            }
            Event::IngressDelivered(sequence) => {
                self.encode(0, Message::Message(SessionMessage::Delivered { sequence }))?
            }
            Event::IngressPressure(sequence) => {
                self.encode(0, Message::Message(SessionMessage::Pressure { sequence }))?
            }
            Event::EgressOffered { sequence, .. } => {
                let machine = &mut self.state.routes[1].machine;
                let buffer = &mut *self.buffer;
                let result = self.port.with_egress(sequence, |bytes| {
                    let binding = machine.binding();
                    let frame = binding.frame(Message::Message(SessionMessage::Offered {
                        sequence,
                        payload: bytes,
                    }));
                    let n = conduit_wire::encode_external_fore_session_frame_into(
                        frame,
                        buffer,
                        binding.limits.maximum_payload_bytes,
                        binding.maximum_frame_bytes,
                    )
                    .map_err(|_| ForeIoRefusal::Wire)?;
                    machine
                        .admit_outbound(frame)
                        .map_err(|_| ForeIoRefusal::Wire)?;
                    Ok(n)
                });
                match result {
                    Ok(result) => result?,
                    Err(bridge::Refusal::Busy) => return Ok(ForeIoProgress::Busy),
                    Err(_) => return Err(ForeIoRefusal::Bridge),
                }
            }
        };
        // The synchronous mailbox loan/encoding above has ended completely.
        self.link
            .send(&self.buffer[..length])
            .await
            .map_err(|_| ForeIoRefusal::Io)?;
        self.state.used_frames += 1;
        self.state.pending = Pending::Sent(event);
        Ok(ForeIoProgress::Progress)
    }
    /// Receive at most one actual frame. The decoder and bridge independently
    /// reject foreign scope, sequencing, primitive values and terminal control.
    pub async fn receive_one(&mut self) -> Result<ForeIoProgress, ForeIoRefusal> {
        let result = self.receive_inner().await;
        if result.is_err() {
            self.retire();
        }
        result
    }
    async fn receive_inner(&mut self) -> Result<ForeIoProgress, ForeIoRefusal> {
        if !matches!(self.state.pending, Pending::None) {
            return self.apply();
        }
        self.check()?;
        let length = self
            .link
            .receive(self.buffer)
            .await
            .map_err(|_| ForeIoRefusal::Io)?
            .len();
        self.state.used_frames += 1;
        self.admit_received(length)
    }
    /// Prefer a pending send/APPLY; otherwise race one cancellation-safe framed
    /// receive against an existing cooperative yield. Native installation MUST
    /// guarantee cancellation-safe receive with partial framing retained in the
    /// original transport. No kernel or mailbox borrow crosses either await.
    pub async fn pump<Y: crate::native_runner::CooperativeYield>(
        &mut self,
        yield_to: &Y,
    ) -> Result<ForeIoProgress, ForeIoRefusal> {
        match self.send_pending().await? {
            ForeIoProgress::Idle => {}
            ForeIoProgress::Busy => {
                yield_to.pause().await;
                return Ok(ForeIoProgress::Busy);
            }
            progress => return Ok(progress),
        }
        let result = {
            let receive = self.receive_one();
            let pause = yield_to.pause();
            let mut receive = pin!(receive);
            let mut pause = pin!(pause);
            poll_fn(|cx| {
                if let Poll::Ready(result) = receive.as_mut().poll(cx) {
                    return Poll::Ready(result);
                }
                if pause.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(Ok(ForeIoProgress::Idle));
                }
                Poll::Pending
            })
            .await
        };
        result
    }
}
