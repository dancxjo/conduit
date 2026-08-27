//! Live bounded two-WebSocket enactment of the exact split Text Lab Plan.

use crate::text_lab_split::NativeTextLabFragment;
use crate::websocket::{NativeWebSocketLine, NativeWebSocketListener};
use conduit_core::{Plan, PlanFragment};
use conduit_std_catalog::{
    exact_text_lab_line_loss_outcome, exact_text_lab_split_plan, TextLabLineLossReceipt,
    TEXT_LAB_BROWSER_HOST, TEXT_LAB_FORWARD_LINE, TEXT_LAB_MAXIMUM_VALUES, TEXT_LAB_NATIVE_BOOT,
    TEXT_LAB_NATIVE_HOST, TEXT_LAB_RETURN_LINE,
};
use conduit_text::MAX_TEXT_BYTES;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};
use std::io::Write;

pub const TEXT_LAB_LIVE_FRAME_BYTES: u32 = 1_024;

fn return_line_loss(
    plan: &Plan,
    base: &str,
    phase: &str,
    sequence: u64,
    transport_failure: &str,
) -> String {
    let browser_upper = conduit_browser_runtime::presentation_nucleus::browser_text_upper_offer();
    let outcome = match exact_text_lab_line_loss_outcome(base, &browser_upper, TEXT_LAB_RETURN_LINE)
    {
        Ok(outcome) => outcome,
        Err(error) => return format!("CND-TEXT-LIVE-302 loss reconciliation failed: {error}"),
    };
    if outcome.immutable_plan_id != plan.plan_id
        || outcome.source_document_id != plan.source_document_id
        || outcome.checked_form_id != plan.checked_form_id
    {
        return "CND-TEXT-LIVE-302 loss reconciliation changed accepted identity".into();
    }
    let host = conduit_core::HostId::from(TEXT_LAB_NATIVE_HOST);
    let boot = conduit_core::BootId::from(TEXT_LAB_NATIVE_BOOT);
    let active = conduit_core::bind_active_play(&plan.plan_id, &host, &boot, 0);
    let sign = conduit_core::bind_sign(&host, &boot, Some(&active.active_play_id), sequence);
    serde_json::to_string(&TextLabLineLossReceipt {
        schema: "conduit.text-lab/line-loss@1".into(),
        code: "CND-TEXT-LIVE-301".into(),
        phase: phase.into(),
        sequence,
        line_id: outcome.unavailable_line_id.as_str().into(),
        plan_id: outcome.immutable_plan_id.as_str().into(),
        source_document_id: outcome.source_document_id.as_str().into(),
        checked_form_id: outcome.checked_form_id.as_str().into(),
        active_play_id: active.active_play_id.as_str().into(),
        sign_id: sign.sign_id.as_str().into(),
        old_plan_disposition: "immutable".into(),
        fresh_planning: "unrealizable".into(),
        form_unchanged: true,
        refusal: outcome.refusal,
        transport_failure: transport_failure.into(),
    })
    .expect("bounded Text Lab loss receipt is serializable")
}

struct LiveSession {
    binding: SessionBinding,
    machine: SessionMachine,
    line: NativeWebSocketLine,
    input: [u8; TEXT_LAB_LIVE_FRAME_BYTES as usize],
    output: [u8; TEXT_LAB_LIVE_FRAME_BYTES as usize],
}

fn fragment<'a>(plan: &'a Plan, host: &str) -> Result<&'a PlanFragment, String> {
    plan.fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == host)
        .ok_or_else(|| format!("Text Lab fragment for {host} is missing"))
}

fn binding(
    plan: &Plan,
    source_host: &str,
    sink_host: &str,
    line: &str,
) -> Result<SessionBinding, String> {
    let source = fragment(plan, source_host)?;
    let sink = fragment(plan, sink_host)?;
    let connection = source
        .connections
        .iter()
        .find(|connection| {
            connection
                .selected_line
                .as_ref()
                .is_some_and(|selected| selected.line_id.as_str() == line)
        })
        .ok_or_else(|| format!("Text Lab planned Line {line} is missing"))?;
    SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        connection,
    )
    .map_err(|error| format!("{error:?}"))
}

impl LiveSession {
    fn accept(
        listener: &NativeWebSocketListener,
        binding: SessionBinding,
        role: SessionRole,
    ) -> Result<Self, String> {
        Ok(Self {
            machine: SessionMachine::new(binding.clone(), role)
                .map_err(|error| format!("{error:?}"))?,
            binding,
            line: listener.accept().map_err(|error| format!("{error:?}"))?,
            input: [0; TEXT_LAB_LIVE_FRAME_BYTES as usize],
            output: [0; TEXT_LAB_LIVE_FRAME_BYTES as usize],
        })
    }

    fn send(&mut self, message: SessionMessage<'_>) -> Result<(), String> {
        let binding = self.binding.clone();
        let frame = binding.frame(message);
        self.machine
            .admit_outbound(frame)
            .map_err(|error| format!("{error:?}"))?;
        let length = encode_session_frame_into(
            frame,
            &mut self.output,
            MAX_TEXT_BYTES,
            TEXT_LAB_LIVE_FRAME_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;
        self.line
            .send_binary(&self.output[..length])
            .map_err(|error| format!("{error:?}"))
    }

    fn receive(&mut self) -> Result<SessionMessage<'_>, String> {
        let length = self
            .line
            .receive_binary(&mut self.input)
            .map_err(|error| format!("{error:?}"))?;
        let frame = decode_session_frame(
            &self.input[..length],
            MAX_TEXT_BYTES,
            TEXT_LAB_LIVE_FRAME_BYTES,
        )
        .map_err(|error| format!("{error:?}"))?;
        self.machine
            .admit_inbound(frame)
            .map_err(|error| format!("{error:?}"))?;
        Ok(frame.message)
    }

    fn handshake(&mut self) -> Result<(), String> {
        if !matches!(self.receive()?, SessionMessage::Hello(_)) {
            return Err("Text Lab peer omitted Hello".into());
        }
        let hello_binding = self.binding.clone();
        self.send(hello_binding.hello_frame().message)?;
        if !matches!(self.receive()?, SessionMessage::Ready) {
            return Err("Text Lab peer omitted Ready".into());
        }
        self.send(SessionMessage::Ready)?;
        if !self.machine.is_active() {
            return Err("Text Lab session is not active".into());
        }
        Ok(())
    }

    fn close(&mut self) -> Result<(), String> {
        self.line.close().map_err(|error| format!("{error:?}"))
    }
}

pub struct TextLabLiveServer {
    listener: NativeWebSocketListener,
    url: String,
}

impl TextLabLiveServer {
    pub fn bind() -> Result<Self, String> {
        let listener = NativeWebSocketListener::bind_loopback(TEXT_LAB_LIVE_FRAME_BYTES)
            .map_err(|error| format!("{error:?}"))?;
        let url = listener.url().map_err(|error| format!("{error:?}"))?;
        Ok(Self { listener, url })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn run<W: Write>(self, report: &mut W) -> Result<(), String> {
        let exact = exact_text_lab_split_plan(
            &self.url,
            &conduit_browser_runtime::presentation_nucleus::browser_text_upper_offer(),
        )?;
        let forward_binding = binding(
            &exact.plan,
            TEXT_LAB_NATIVE_HOST,
            TEXT_LAB_BROWSER_HOST,
            TEXT_LAB_FORWARD_LINE,
        )?;
        let return_binding = binding(
            &exact.plan,
            TEXT_LAB_BROWSER_HOST,
            TEXT_LAB_NATIVE_HOST,
            TEXT_LAB_RETURN_LINE,
        )?;
        let mut forward =
            LiveSession::accept(&self.listener, forward_binding, SessionRole::Source)?;
        forward.handshake()?;
        let mut returned = LiveSession::accept(&self.listener, return_binding, SessionRole::Sink)?;
        returned.handshake()?;
        let mut native = NativeTextLabFragment::prepare(&self.url)?;

        for expected in ["h", "e", "l", "l", "o"] {
            let offer = native.next_text_offer()?;
            forward.send(SessionMessage::Offered {
                sequence: offer.sequence,
                payload: &offer.bytes,
            })?;
            match forward.receive()? {
                SessionMessage::Accepted { sequence } if sequence == offer.sequence => {
                    native.accept_text(sequence)?;
                }
                other => return Err(format!("unexpected forward acceptance {other:?}")),
            }
            match forward.receive()? {
                SessionMessage::Delivered { sequence } if sequence == offer.sequence => {
                    native.deliver_text(sequence)?;
                }
                other => return Err(format!("unexpected forward delivery {other:?}")),
            }
            let (sequence, payload) = match returned.receive().map_err(|detail| {
                return_line_loss(
                    &exact.plan,
                    &self.url,
                    "return-offer",
                    offer.sequence,
                    &detail,
                )
            })? {
                SessionMessage::Offered { sequence, payload } => (sequence, payload.to_vec()),
                other => return Err(format!("unexpected return offer {other:?}")),
            };
            if sequence != offer.sequence || payload != expected.to_ascii_uppercase().as_bytes() {
                return Err("browser returned the wrong Text Lab value".into());
            }
            native.admit_returned(sequence, &payload)?;
            returned
                .send(SessionMessage::Accepted { sequence })
                .map_err(|detail| {
                    return_line_loss(&exact.plan, &self.url, "return-accepted", sequence, &detail)
                })?;
            native.drive_presentation((sequence + 1) as usize)?;
            returned
                .send(SessionMessage::Delivered { sequence })
                .map_err(|detail| {
                    return_line_loss(
                        &exact.plan,
                        &self.url,
                        "return-delivered",
                        sequence,
                        &detail,
                    )
                })?;
        }
        native.finish_forward()?;
        let final_sequence = TEXT_LAB_MAXIMUM_VALUES as u64;
        forward.send(SessionMessage::InputClosed { final_sequence })?;
        let terminal = SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence,
        };
        forward.send(terminal)?;
        if forward.receive()? != terminal || !forward.machine.is_terminal() {
            return Err("forward Text Lab session lacked terminal agreement".into());
        }
        if returned.receive()? != (SessionMessage::InputClosed { final_sequence }) {
            return Err("return Text Lab session lacked input closure".into());
        }
        if returned.receive()? != terminal {
            return Err("return Text Lab source omitted terminal".into());
        }
        returned.send(terminal)?;
        if !returned.machine.is_terminal() {
            return Err("return Text Lab session lacked terminal agreement".into());
        }
        native.close_return_input()?;
        native.finish()?;
        if native.presented() != "HELLO" {
            return Err("native Text Lab Presentation did not manifest HELLO".into());
        }
        forward.close()?;
        returned.close()?;
        writeln!(
            report,
            "text_lab=HELLO values=5 forward_terminal=completed return_terminal=completed"
        )
        .map_err(|error| error.to_string())
    }
}
