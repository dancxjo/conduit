//! WASM ABI for the two exact live split Text Lab sessions.

use crate::text_lab_split::BrowserTextLabFragment;
use conduit_core::{Plan, PlanFragment};
use conduit_std_catalog::{
    exact_text_lab_split_plan, TEXT_LAB_BROWSER_HOST, TEXT_LAB_FORWARD_LINE,
    TEXT_LAB_MAXIMUM_VALUES, TEXT_LAB_NATIVE_HOST, TEXT_LAB_RETURN_LINE,
};
use conduit_text::MAX_TEXT_BYTES;
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionBinding, SessionMachine,
    SessionMessage, SessionRole, SessionTerminalDisposition,
};
use std::cell::RefCell;

const FRAME_BYTES: usize = 1_024;
const LINE_NONE: i32 = 0;
const LINE_FORWARD: i32 = 1;
const LINE_RETURN: i32 = 2;
const STATUS_RUNNING: i32 = 0;
const STATUS_COMPLETE: i32 = 1;
const ERROR_NOT_STARTED: i32 = -201;
const ERROR_PREPARE: i32 = -202;
const ERROR_SESSION: i32 = -203;
const ERROR_KERNEL: i32 = -204;

thread_local! {
    static LIVE: RefCell<Option<TextLabBrowserLive>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; FRAME_BYTES]> = const { RefCell::new([0; FRAME_BYTES]) };
}

#[derive(Clone, Copy)]
enum SentAction {
    None,
    ForwardAccepted(u64),
    ForwardDelivered(u64),
    ForwardTerminal,
    ReturnInputClosed,
}

struct TextLabBrowserLive {
    fragment: BrowserTextLabFragment,
    forward_binding: SessionBinding,
    forward: SessionMachine,
    returned_binding: SessionBinding,
    returned: SessionMachine,
    output: [u8; FRAME_BYTES],
    output_len: usize,
    output_line: i32,
    expected_line: i32,
    sent_action: SentAction,
    delivered_values: u32,
    complete: bool,
}

fn fragment<'a>(plan: &'a Plan, host: &str) -> Result<&'a PlanFragment, i32> {
    plan.fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == host)
        .ok_or(ERROR_PREPARE)
}

fn binding(
    plan: &Plan,
    source_host: &str,
    sink_host: &str,
    line: &str,
) -> Result<SessionBinding, i32> {
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
        .ok_or(ERROR_PREPARE)?;
    SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        connection,
    )
    .map_err(|_| ERROR_SESSION)
}

impl TextLabBrowserLive {
    fn prepare(base: &str) -> Result<Self, i32> {
        let exact = exact_text_lab_split_plan(base).map_err(|_| ERROR_PREPARE)?;
        let forward_binding = binding(
            &exact.plan,
            TEXT_LAB_NATIVE_HOST,
            TEXT_LAB_BROWSER_HOST,
            TEXT_LAB_FORWARD_LINE,
        )?;
        let returned_binding = binding(
            &exact.plan,
            TEXT_LAB_BROWSER_HOST,
            TEXT_LAB_NATIVE_HOST,
            TEXT_LAB_RETURN_LINE,
        )?;
        let forward = SessionMachine::new(forward_binding.clone(), SessionRole::Sink)
            .map_err(|_| ERROR_SESSION)?;
        let returned = SessionMachine::new(returned_binding.clone(), SessionRole::Source)
            .map_err(|_| ERROR_SESSION)?;
        let fragment = BrowserTextLabFragment::prepare(base).map_err(|_| ERROR_KERNEL)?;
        let mut live = Self {
            fragment,
            forward_binding,
            forward,
            returned_binding,
            returned,
            output: [0; FRAME_BYTES],
            output_len: 0,
            output_line: LINE_NONE,
            expected_line: LINE_FORWARD,
            sent_action: SentAction::None,
            delivered_values: 0,
            complete: false,
        };
        let hello_binding = live.forward_binding.clone();
        let hello = hello_binding.hello_frame().message;
        live.emit_forward(hello, SentAction::None)?;
        Ok(live)
    }

    fn emit_forward(&mut self, message: SessionMessage<'_>, action: SentAction) -> Result<(), i32> {
        let binding = self.forward_binding.clone();
        let frame = binding.frame(message);
        self.forward
            .admit_outbound(frame)
            .map_err(|_| ERROR_SESSION)?;
        self.encode(LINE_FORWARD, frame, action)
    }

    fn emit_return(&mut self, message: SessionMessage<'_>, action: SentAction) -> Result<(), i32> {
        let binding = self.returned_binding.clone();
        let frame = binding.frame(message);
        self.returned
            .admit_outbound(frame)
            .map_err(|_| ERROR_SESSION)?;
        self.encode(LINE_RETURN, frame, action)
    }

    fn encode(
        &mut self,
        line: i32,
        frame: conduit_wire::SessionFrame<'_>,
        action: SentAction,
    ) -> Result<(), i32> {
        self.output_len =
            encode_session_frame_into(frame, &mut self.output, MAX_TEXT_BYTES, FRAME_BYTES as u32)
                .map_err(|_| ERROR_SESSION)?;
        self.output_line = line;
        self.sent_action = action;
        Ok(())
    }

    fn ingest(&mut self, line: i32, bytes: &[u8]) -> Result<(), i32> {
        if self.output_line != LINE_NONE || line != self.expected_line {
            return Err(ERROR_SESSION);
        }
        let frame = decode_session_frame(bytes, MAX_TEXT_BYTES, FRAME_BYTES as u32)
            .map_err(|_| ERROR_SESSION)?;
        match line {
            LINE_FORWARD => {
                self.forward
                    .admit_inbound(frame)
                    .map_err(|_| ERROR_SESSION)?;
                self.ingest_forward(frame.message)
            }
            LINE_RETURN => {
                self.returned
                    .admit_inbound(frame)
                    .map_err(|_| ERROR_SESSION)?;
                self.ingest_return(frame.message)
            }
            _ => Err(ERROR_SESSION),
        }
    }

    fn ingest_forward(&mut self, message: SessionMessage<'_>) -> Result<(), i32> {
        match message {
            SessionMessage::Hello(_) => self.emit_forward(SessionMessage::Ready, SentAction::None),
            SessionMessage::Ready if self.forward.is_active() => {
                self.expected_line = LINE_RETURN;
                let hello_binding = self.returned_binding.clone();
                let hello = hello_binding.hello_frame().message;
                self.emit_return(hello, SentAction::None)
            }
            SessionMessage::Offered { sequence, payload } => {
                self.fragment
                    .admit_text(sequence, payload)
                    .map_err(|_| ERROR_KERNEL)?;
                self.emit_forward(
                    SessionMessage::Accepted { sequence },
                    SentAction::ForwardAccepted(sequence),
                )
            }
            SessionMessage::InputClosed { final_sequence }
                if final_sequence == TEXT_LAB_MAXIMUM_VALUES as u64 =>
            {
                self.fragment.close_text_input().map_err(|_| ERROR_KERNEL)?;
                self.fragment.finish().map_err(|_| ERROR_KERNEL)
            }
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence,
            } if final_sequence == TEXT_LAB_MAXIMUM_VALUES as u64 => self.emit_forward(
                SessionMessage::Terminal {
                    disposition: SessionTerminalDisposition::Completed,
                    final_sequence,
                },
                SentAction::ForwardTerminal,
            ),
            _ => Err(ERROR_SESSION),
        }
    }

    fn ingest_return(&mut self, message: SessionMessage<'_>) -> Result<(), i32> {
        match message {
            SessionMessage::Hello(_) => self.emit_return(SessionMessage::Ready, SentAction::None),
            SessionMessage::Ready if self.returned.is_active() => {
                self.expected_line = LINE_FORWARD;
                Ok(())
            }
            SessionMessage::Accepted { sequence } => self
                .fragment
                .accept_upper(sequence)
                .map_err(|_| ERROR_KERNEL),
            SessionMessage::Delivered { sequence } => {
                self.fragment
                    .deliver_upper(sequence)
                    .map_err(|_| ERROR_KERNEL)?;
                self.delivered_values = self.delivered_values.checked_add(1).ok_or(ERROR_KERNEL)?;
                self.expected_line = LINE_FORWARD;
                Ok(())
            }
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence,
            } if final_sequence == TEXT_LAB_MAXIMUM_VALUES as u64 => {
                if !self.returned.is_terminal() || !self.forward.is_terminal() {
                    return Err(ERROR_SESSION);
                }
                self.complete = true;
                self.expected_line = LINE_NONE;
                Ok(())
            }
            _ => Err(ERROR_SESSION),
        }
    }

    fn sent(&mut self) -> Result<(), i32> {
        if self.output_line == LINE_NONE {
            return Err(ERROR_SESSION);
        }
        self.output_len = 0;
        self.output_line = LINE_NONE;
        let action = core::mem::replace(&mut self.sent_action, SentAction::None);
        match action {
            SentAction::None => Ok(()),
            SentAction::ForwardAccepted(sequence) => self.emit_forward(
                SessionMessage::Delivered { sequence },
                SentAction::ForwardDelivered(sequence),
            ),
            SentAction::ForwardDelivered(sequence) => {
                let offer = self.fragment.next_upper_offer().map_err(|_| ERROR_KERNEL)?;
                if offer.sequence != sequence {
                    return Err(ERROR_SESSION);
                }
                self.expected_line = LINE_RETURN;
                self.emit_return(
                    SessionMessage::Offered {
                        sequence,
                        payload: &offer.bytes,
                    },
                    SentAction::None,
                )
            }
            SentAction::ForwardTerminal => {
                self.expected_line = LINE_RETURN;
                self.emit_return(
                    SessionMessage::InputClosed {
                        final_sequence: TEXT_LAB_MAXIMUM_VALUES as u64,
                    },
                    SentAction::ReturnInputClosed,
                )
            }
            SentAction::ReturnInputClosed => self.emit_return(
                SessionMessage::Terminal {
                    disposition: SessionTerminalDisposition::Completed,
                    final_sequence: TEXT_LAB_MAXIMUM_VALUES as u64,
                },
                SentAction::None,
            ),
        }
    }
}

fn with_live(action: impl FnOnce(&mut TextLabBrowserLive) -> Result<(), i32>) -> i32 {
    LIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(live) = slot.as_mut() else {
            return ERROR_NOT_STARTED;
        };
        action(live).map_or_else(|code| code, |_| STATUS_RUNNING)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_input_capacity() -> u32 {
    FRAME_BYTES as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_start(base_length: u32) -> i32 {
    LIVE.with(|slot| slot.borrow_mut().take());
    let length = base_length as usize;
    if length == 0 || length > FRAME_BYTES {
        return ERROR_PREPARE;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        let Ok(base) = core::str::from_utf8(&input[..length]) else {
            return ERROR_PREPARE;
        };
        match TextLabBrowserLive::prepare(base) {
            Ok(live) => {
                LIVE.with(|slot| *slot.borrow_mut() = Some(live));
                STATUS_RUNNING
            }
            Err(code) => code,
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_ingest(line: i32, length: u32) -> i32 {
    let length = length as usize;
    if length > FRAME_BYTES {
        return ERROR_SESSION;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        with_live(|live| live.ingest(line, &input[..length]))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_sent() -> i32 {
    with_live(TextLabBrowserLive::sent)
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_output_line() -> i32 {
    LIVE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(LINE_NONE, |live| live.output_line)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_expected_line() -> i32 {
    LIVE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(LINE_NONE, |live| live.expected_line)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_output_ptr() -> *const u8 {
    LIVE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(core::ptr::null(), |live| live.output.as_ptr())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_output_len() -> u32 {
    LIVE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |live| live.output_len as u32)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_status() -> i32 {
    LIVE.with(|slot| {
        slot.borrow().as_ref().map_or(ERROR_NOT_STARTED, |live| {
            if live.complete {
                STATUS_COMPLETE
            } else {
                STATUS_RUNNING
            }
        })
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_text_lab_delivered_values() -> u32 {
    LIVE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |live| live.delivered_values)
    })
}
