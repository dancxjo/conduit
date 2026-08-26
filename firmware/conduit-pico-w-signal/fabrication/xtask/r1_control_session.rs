//! Transport-neutral host execution for exact R1 three-peer control Plans.

use conduit_std_host::pico_control_source::PicoControlSource;
use conduit_std_host::r1_control::{R1InputEvent, R1MergedInput};
use conduit_wire::{SessionMessage, SessionTerminalDisposition};

use super::r1_signal::R1SessionIo;
use super::PicoResult;

const SESSION_FRAME_BYTES: usize = conduit_net::R1_MAXIMUM_FRAME_BYTES as usize;

pub fn handshake(io: &mut impl R1SessionIo, source: &mut PicoControlSource) -> PicoResult<()> {
    let binding = source.binding().clone();
    let hello = binding.hello_frame();
    source.admit_outbound(hello)?;
    io.send(&hello)?;
    let mut bytes = [0_u8; SESSION_FRAME_BYTES];
    let received = io.receive(&mut bytes)?;
    if !matches!(received.message, SessionMessage::Hello(_)) {
        return Err("R1 control sink did not return exact Hello".into());
    }
    source.admit_inbound(received)?;

    let ready = binding.frame(SessionMessage::Ready);
    source.admit_outbound(ready)?;
    io.send(&ready)?;
    let received = io.receive(&mut bytes)?;
    if !matches!(received.message, SessionMessage::Ready) {
        return Err("R1 control sink did not return exact Ready".into());
    }
    source.admit_inbound(received)?;
    if !source.is_active() {
        return Err("R1 control session did not become active".into());
    }
    Ok(())
}

pub fn deliver_input(
    io: &mut impl R1SessionIo,
    source: &mut PicoControlSource,
    input: R1InputEvent,
    after_accepted: &mut impl FnMut(u64) -> PicoResult<()>,
) -> PicoResult<R1MergedInput> {
    let (sequence, payload) = source.offer_input(input)?;
    let binding = source.binding().clone();
    loop {
        let offered = binding.frame(SessionMessage::Offered {
            sequence,
            payload: &payload,
        });
        source.admit_outbound(offered)?;
        io.send(&offered)?;
        let mut bytes = [0_u8; SESSION_FRAME_BYTES];
        let response = io.receive(&mut bytes)?;
        source.admit_inbound(response)?;
        match response.message {
            SessionMessage::Pressure { sequence: found } if found == sequence => {
                source.pressure(sequence)?;
            }
            SessionMessage::Accepted { sequence: found } if found == sequence => break,
            _ => return Err("R1 control sink returned an unexpected offer disposition".into()),
        }
    }
    after_accepted(sequence)?;
    let mut bytes = [0_u8; SESSION_FRAME_BYTES];
    let delivered = io.receive(&mut bytes)?;
    source.admit_inbound(delivered)?;
    if !matches!(delivered.message, SessionMessage::Delivered { sequence: found } if found == sequence)
    {
        return Err("R1 control sink returned an unexpected delivery disposition".into());
    }
    source.delivered(sequence).map_err(Into::into)
}

pub fn replay_offered(
    io: &mut impl R1SessionIo,
    source: &mut PicoControlSource,
    sequence: u64,
    payload: &[u8],
    after_accepted: &mut impl FnMut(u64) -> PicoResult<()>,
) -> PicoResult<R1MergedInput> {
    let binding = source.binding().clone();
    let offered = binding.frame(SessionMessage::Offered { sequence, payload });
    // Checkpoint reconciliation retained this exact Offered transfer in the
    // source machine, so retransmission must not admit it as a new offer.
    io.send(&offered)?;
    let mut bytes = [0_u8; SESSION_FRAME_BYTES];
    let accepted = io.receive(&mut bytes)?;
    source.admit_inbound(accepted)?;
    if !matches!(accepted.message, SessionMessage::Accepted { sequence: found } if found == sequence)
    {
        return Err("R1 control sink did not accept the reconciled offered input".into());
    }
    after_accepted(sequence)?;
    let delivered = io.receive(&mut bytes)?;
    source.admit_inbound(delivered)?;
    if !matches!(delivered.message, SessionMessage::Delivered { sequence: found } if found == sequence)
    {
        return Err("R1 control sink did not deliver the reconciled offered input".into());
    }
    source.delivered(sequence).map_err(Into::into)
}

pub fn finish(io: &mut impl R1SessionIo, source: &mut PicoControlSource) -> PicoResult<u64> {
    let final_sequence = source.final_sequence()?;
    let binding = source.binding().clone();
    let closed = binding.frame(SessionMessage::InputClosed { final_sequence });
    source.admit_outbound(closed)?;
    io.send(&closed)?;
    let terminal = binding.frame(SessionMessage::Terminal {
        disposition: SessionTerminalDisposition::Completed,
        final_sequence,
    });
    source.admit_outbound(terminal)?;
    io.send(&terminal)?;
    let mut bytes = [0_u8; SESSION_FRAME_BYTES];
    let response = io.receive(&mut bytes)?;
    if !matches!(
        response.message,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: found,
        } if found == final_sequence
    ) {
        return Err("R1 control sink returned an unexpected terminal disposition".into());
    }
    source.admit_inbound(response)?;
    if !source.is_terminal() {
        return Err("R1 control session did not reach reciprocal terminal agreement".into());
    }
    Ok(final_sequence)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use conduit_core::{BootId, HostId};
    use conduit_signal::SIGNAL_ENCODED_LEN;
    use conduit_std_host::r1_control::R1ControlPeer;
    use conduit_wire::{
        decode_session_frame, encode_session_frame_into, SessionBinding, SessionFrame,
        SessionMachine, SessionRole,
    };

    use super::*;

    #[derive(Clone, Copy)]
    enum Response {
        Hello,
        Ready,
        Pressure(u64),
        Accepted(u64),
        Delivered(u64),
        Terminal(u64),
    }

    struct FakeIo {
        binding: SessionBinding,
        sink: SessionMachine,
        responses: VecDeque<Response>,
        pressured_once: bool,
    }

    impl FakeIo {
        fn new(binding: SessionBinding) -> Self {
            Self {
                sink: SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap(),
                binding,
                responses: VecDeque::new(),
                pressured_once: false,
            }
        }
    }

    impl R1SessionIo for FakeIo {
        fn send(&mut self, frame: &SessionFrame<'_>) -> PicoResult<()> {
            self.sink
                .admit_inbound(*frame)
                .map_err(|error| format!("fake control sink rejected inbound: {error:?}"))?;
            match frame.message {
                SessionMessage::Hello(_) => self.responses.push_back(Response::Hello),
                SessionMessage::Ready => self.responses.push_back(Response::Ready),
                SessionMessage::Offered { sequence, .. } if !self.pressured_once => {
                    self.pressured_once = true;
                    self.responses.push_back(Response::Pressure(sequence));
                }
                SessionMessage::Offered { sequence, .. } => {
                    self.responses.push_back(Response::Accepted(sequence));
                    self.responses.push_back(Response::Delivered(sequence));
                }
                SessionMessage::InputClosed { .. } => {}
                SessionMessage::Terminal { final_sequence, .. } => {
                    self.responses.push_back(Response::Terminal(final_sequence));
                }
                _ => return Err("fake control sink received an unexpected source frame".into()),
            }
            Ok(())
        }

        fn receive<'a>(&mut self, bytes: &'a mut [u8]) -> PicoResult<SessionFrame<'a>> {
            let response = self
                .responses
                .pop_front()
                .ok_or("fake control sink has no queued response")?;
            let binding = self.binding.clone();
            let frame = match response {
                Response::Hello => binding.hello_frame(),
                Response::Ready => binding.frame(SessionMessage::Ready),
                Response::Pressure(sequence) => {
                    binding.frame(SessionMessage::Pressure { sequence })
                }
                Response::Accepted(sequence) => {
                    binding.frame(SessionMessage::Accepted { sequence })
                }
                Response::Delivered(sequence) => {
                    binding.frame(SessionMessage::Delivered { sequence })
                }
                Response::Terminal(final_sequence) => binding.frame(SessionMessage::Terminal {
                    disposition: SessionTerminalDisposition::Completed,
                    final_sequence,
                }),
            };
            self.sink
                .admit_outbound(frame)
                .map_err(|error| format!("fake control sink rejected outbound: {error:?}"))?;
            let length = encode_session_frame_into(
                frame,
                bytes,
                SIGNAL_ENCODED_LEN,
                conduit_net::R1_MAXIMUM_FRAME_BYTES,
            )
            .map_err(|error| format!("fake control sink encode failed: {error:?}"))?;
            decode_session_frame(
                &bytes[..length],
                SIGNAL_ENCODED_LEN,
                conduit_net::R1_MAXIMUM_FRAME_BYTES,
            )
            .map_err(|error| format!("fake control sink decode failed: {error:?}").into())
        }
    }

    #[test]
    fn transport_neutral_driver_delivers_exact_inputs_with_pressure_and_terminal() {
        let exact = conduit_system_continuity::exact_r1_control_plan(
            BootId::from(conduit_net::R1_PICO_BOOT_ID),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )
        .unwrap();
        let mut source =
            PicoControlSource::prepare_plan(exact.plan, &HostId::from(conduit_net::R1_STD_HOST_ID))
                .unwrap();
        let mut io = FakeIo::new(source.binding().clone());
        handshake(&mut io, &mut source).unwrap();
        let mut accepted = Vec::new();
        for peer in [
            R1ControlPeer::Terminal,
            R1ControlPeer::BrowserA,
            R1ControlPeer::BrowserB,
        ] {
            for (peer_sequence, level) in [(0, true), (1, false)] {
                let merged = deliver_input(
                    &mut io,
                    &mut source,
                    R1InputEvent {
                        peer,
                        peer_sequence,
                        level,
                    },
                    &mut |sequence| {
                        accepted.push(sequence);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(merged.input.peer, peer);
                assert_eq!(merged.input.level, level);
            }
        }
        assert_eq!(finish(&mut io, &mut source).unwrap(), 6);
        assert_eq!(accepted, (0..6).collect::<Vec<_>>());
        assert_eq!(source.pressure_retries(), 1);
    }
}
