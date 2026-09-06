//! Bounded WASM boundary for the two-browser-Host Tour lesson.

use super::protocol::{LineFrame, Output};
use super::session::{Role, Session};
use std::cell::RefCell;

const INPUT_BYTES: usize = 64 * 1_024;
const OUTPUT_BYTES: usize = 64 * 1_024;
const STATUS_READY: i32 = 0;
const ERROR_INPUT: i32 = -451;
const ERROR_PREPARE: i32 = -452;
const ERROR_NOT_RUNNING: i32 = -453;
const ERROR_OUTPUT: i32 = -454;
const ERROR_PROTOCOL: i32 = -455;
const ERROR_COMPLETE: i32 = -456;
const ERROR_CANCEL: i32 = -457;
const ERROR_INTERACTION: i32 = -458;

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; INPUT_BYTES]> = const { RefCell::new([0; INPUT_BYTES]) };
    static OUTPUT: RefCell<[u8; OUTPUT_BYTES]> = const { RefCell::new([0; OUTPUT_BYTES]) };
    static OUTPUT_LEN: RefCell<usize> = const { RefCell::new(0) };
    static SOURCE_INTERACTION: RefCell<Option<crate::source_interaction::SourceInteractionEvidence>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_input_capacity() -> usize {
    INPUT_BYTES
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_output_len() -> usize {
    OUTPUT_LEN.with(|length| *length.borrow())
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_admit_source_interaction(
    source_length: usize,
    sequence: u64,
) -> i32 {
    clear_output();
    SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = None);
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = crate::source_interaction::admit_source(&input[..source_length], sequence);
        input[..source_length].fill(0);
        match result {
            Ok(evidence) => {
                SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = Some(evidence));
                STATUS_READY
            }
            Err(message) => {
                let _ = write_output(&crate::form_runner::refusal(message));
                ERROR_INTERACTION
            }
        }
    })
}

/// Checks and plans the ordinary Form once, then starts its source fragment.
#[no_mangle]
pub extern "C" fn conduit_tour_multi_start_source(
    source_host_length: usize,
    source_boot_length: usize,
    sink_host_length: usize,
    sink_boot_length: usize,
    source_length: usize,
    play_sequence: u64,
) -> i32 {
    clear_output();
    let lengths = [
        source_host_length,
        source_boot_length,
        sink_host_length,
        sink_boot_length,
        source_length,
    ];
    if lengths.contains(&0) {
        return ERROR_INPUT;
    }
    let Some(total_length) = lengths
        .iter()
        .try_fold(0usize, |total, length| total.checked_add(*length))
    else {
        return ERROR_INPUT;
    };
    if total_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    let Some(source_interaction) = SOURCE_INTERACTION.with(|slot| slot.borrow_mut().take()) else {
        let _ = write_output(&crate::form_runner::refusal(
            "source interaction was not admitted before multi-Host parsing".into(),
        ));
        return ERROR_INTERACTION;
    };
    SESSION.with(|slot| {
        if let Some(mut previous) = slot.borrow_mut().take() {
            let _ = previous.cancel();
        }
    });
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let mut offset = 0usize;
            let mut take = |length: usize| {
                let start = offset;
                offset += length;
                core::str::from_utf8(&input[start..offset]).map_err(|_| ERROR_INPUT)
            };
            let source_host = take(source_host_length)?;
            let source_boot = take(source_boot_length)?;
            let sink_host = take(sink_host_length)?;
            let sink_boot = take(sink_boot_length)?;
            let source = take(source_length)?;
            let verified = crate::source_interaction::admit_source(
                source.as_bytes(),
                source_interaction.sequence,
            )
            .map_err(|message| {
                let _ = write_output(&crate::form_runner::refusal(message));
                ERROR_INTERACTION
            })?;
            if verified.proposal_identity != source_interaction.proposal_identity {
                write_output(&crate::form_runner::refusal(
                    "source changed after typed multi-Host interaction admission".into(),
                ))
                .map_err(|_| ERROR_OUTPUT)?;
                return Err(ERROR_INTERACTION);
            }
            let exact =
                super::plan::prepare(source_host, source_boot, sink_host, sink_boot, source)
                    .map_err(|message| {
                        let _ = write_output(&crate::form_runner::refusal(message));
                        ERROR_PREPARE
                    })?;
            let (session, output) =
                Session::prepare(Role::Source, exact, play_sequence, source_interaction).map_err(
                    |message| {
                        let _ = write_output(&crate::form_runner::refusal(message));
                        ERROR_PREPARE
                    },
                )?;
            write_output(&output).map_err(|_| ERROR_OUTPUT)?;
            SESSION.with(|slot| *slot.borrow_mut() = Some(session));
            Ok(STATUS_READY)
        })();
        input[..total_length].fill(0);
        result.unwrap_or_else(|error| error)
    })
}

/// Admits the exact Plan emitted by the source Host and starts only its sink
/// fragment. This boundary never parses, checks, expands, or replans the Form.
#[no_mangle]
pub extern "C" fn conduit_tour_multi_start_sink(
    sink_host_length: usize,
    sink_boot_length: usize,
    plan_length: usize,
    play_sequence: u64,
) -> i32 {
    clear_output();
    let lengths = [sink_host_length, sink_boot_length, plan_length];
    if lengths.contains(&0) {
        return ERROR_INPUT;
    }
    let Some(total_length) = lengths
        .iter()
        .try_fold(0usize, |total, length| total.checked_add(*length))
    else {
        return ERROR_INPUT;
    };
    if total_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    let Some(source_interaction) = SOURCE_INTERACTION.with(|slot| slot.borrow_mut().take()) else {
        let _ = write_output(&crate::form_runner::refusal(
            "source interaction was not admitted before exact Plan admission".into(),
        ));
        return ERROR_INTERACTION;
    };
    SESSION.with(|slot| {
        if let Some(mut previous) = slot.borrow_mut().take() {
            let _ = previous.cancel();
        }
    });
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let sink_host_end = sink_host_length;
            let sink_boot_end = sink_host_end + sink_boot_length;
            let sink_host =
                core::str::from_utf8(&input[..sink_host_end]).map_err(|_| ERROR_INPUT)?;
            let sink_boot = core::str::from_utf8(&input[sink_host_end..sink_boot_end])
                .map_err(|_| ERROR_INPUT)?;
            let plan =
                serde_json::from_slice(&input[sink_boot_end..total_length]).map_err(|_| {
                    let _ = write_output(&crate::form_runner::refusal(
                        "received multi-Host Plan is not valid bounded JSON".into(),
                    ));
                    ERROR_PREPARE
                })?;
            let exact = super::plan::accept(plan, sink_host, sink_boot).map_err(|message| {
                let _ = write_output(&crate::form_runner::refusal(message));
                ERROR_PREPARE
            })?;
            let (session, output) =
                Session::prepare(Role::Sink, exact, play_sequence, source_interaction).map_err(
                    |message| {
                        let _ = write_output(&crate::form_runner::refusal(message));
                        ERROR_PREPARE
                    },
                )?;
            write_output(&output).map_err(|_| ERROR_OUTPUT)?;
            SESSION.with(|slot| *slot.borrow_mut() = Some(session));
            Ok(STATUS_READY)
        })();
        input[..total_length].fill(0);
        result.unwrap_or_else(|error| error)
    })
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_ingest(length: usize) -> i32 {
    clear_output();
    if length == 0 || length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let frame = serde_json::from_slice::<LineFrame>(&input[..length]);
        input[..length].fill(0);
        let Ok(frame) = frame else {
            return ERROR_PROTOCOL;
        };
        with_session(|session| session.ingest(frame), ERROR_PROTOCOL)
    })
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_complete() -> i32 {
    clear_output();
    with_session(Session::complete_manifestation, ERROR_COMPLETE)
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_complete_input(
    play_length: usize,
    request: u32,
    length: usize,
) -> i32 {
    clear_output();
    if play_length == 0
        || play_length > 256
        || length == 0
        || length > crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES
        || play_length.saturating_add(length) > INPUT_BYTES
    {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = match core::str::from_utf8(&input[..play_length]) {
            Ok(play) => with_session(
                |session| {
                    session.complete_input(play, request, &input[play_length..play_length + length])
                },
                ERROR_COMPLETE,
            ),
            Err(_) => ERROR_INPUT,
        };
        input[..play_length + length].fill(0);
        result
    })
}

#[no_mangle]
pub extern "C" fn conduit_tour_multi_cancel() -> i32 {
    clear_output();
    SESSION.with(|slot| {
        let Some(mut session) = slot.borrow_mut().take() else {
            return ERROR_NOT_RUNNING;
        };
        match session
            .cancel()
            .and_then(|output| write_output(&output).map_err(|_| "output".into()))
        {
            Ok(()) => STATUS_READY,
            Err(_) => ERROR_CANCEL,
        }
    })
}

fn with_session(action: impl FnOnce(&mut Session) -> Result<Output, String>, error: i32) -> i32 {
    SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(session) = slot.as_mut() else {
            return ERROR_NOT_RUNNING;
        };
        match action(session).and_then(|output| write_output(&output).map_err(|_| "output".into()))
        {
            Ok(()) => STATUS_READY,
            Err(_) => error,
        }
    })
}

fn write_output(value: &impl serde::Serialize) -> Result<(), ()> {
    let encoded = serde_json::to_vec(value).map_err(|_| ())?;
    if encoded.len() > OUTPUT_BYTES {
        return Err(());
    }
    OUTPUT.with(|output| output.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
    OUTPUT_LEN.with(|length| *length.borrow_mut() = encoded.len());
    Ok(())
}

fn clear_output() {
    OUTPUT_LEN.with(|length| *length.borrow_mut() = 0);
}
