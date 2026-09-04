//! Bounded WASM boundary for the single executable-book Play.

use super::BookSession;
use std::cell::RefCell;

pub(super) const INPUT_BYTES: usize = 8 * 1_024;
pub(super) const OUTPUT_BYTES: usize = 128 * 1_024;
const STATUS_READY: i32 = 0;
const ERROR_INPUT: i32 = -401;
const ERROR_PREPARE: i32 = -402;
const ERROR_NOT_RUNNING: i32 = -403;
const ERROR_OUTPUT: i32 = -404;
const ERROR_COMPLETE: i32 = -405;
const ERROR_CANCEL: i32 = -406;
const ERROR_INTERACTION: i32 = -407;
const ERROR_PROJECTION: i32 = -408;

thread_local! {
    static SESSION: RefCell<Option<BookSession>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; INPUT_BYTES]> = const { RefCell::new([0; INPUT_BYTES]) };
    static OUTPUT: RefCell<[u8; OUTPUT_BYTES]> = const { RefCell::new([0; OUTPUT_BYTES]) };
    static OUTPUT_LEN: RefCell<usize> = const { RefCell::new(0) };
    static SOURCE_INTERACTION: RefCell<Option<crate::source_interaction::SourceInteractionEvidence>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn conduit_book_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_book_input_capacity() -> usize {
    INPUT_BYTES
}

#[no_mangle]
pub extern "C" fn conduit_book_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_book_output_len() -> usize {
    OUTPUT_LEN.with(|length| *length.borrow())
}

#[no_mangle]
pub extern "C" fn conduit_book_encode_button_transition(pressed: u32, sequence: u64) -> i32 {
    clear_output();
    let encoded =
        conduit_semantic_catalog::button_transition_value("button/primary", pressed != 0, sequence)
            .and_then(|value| value.canonical_bytes())
            .map_err(|_| ());
    match encoded.and_then(|encoded| write_output_bytes(&encoded)) {
        Ok(()) => STATUS_READY,
        Err(()) => ERROR_OUTPUT,
    }
}

/// Writes the machine-readable browser Gear inventory derived from the same
/// Host advertisement used by planning.
#[no_mangle]
pub extern "C" fn conduit_book_inventory() -> i32 {
    clear_output();
    write_output(&crate::installed_browser::inventory())
        .map(|()| STATUS_READY)
        .unwrap_or(ERROR_OUTPUT)
}

/// Projects the exact fabrication selections used to construct this runtime's
/// ordinary installed-host advertisement.
#[no_mangle]
pub extern "C" fn conduit_book_human_machinery() -> i32 {
    clear_output();
    let implementations = crate::installed_browser::selected_human_machinery()
        .into_iter()
        .map(|id| serde_json::json!({ "id": id, "revision": 1 }))
        .collect::<Vec<_>>();
    write_output(&serde_json::json!({
        "schema": "conduit.browser/selected-human-machinery@1",
        "implementations": implementations,
    }))
    .map(|()| STATUS_READY)
    .unwrap_or(ERROR_OUTPUT)
}

#[no_mangle]
pub extern "C" fn conduit_book_reviewed_gallery() -> i32 {
    clear_output();
    match super::gallery::reviewed_gallery() {
        Ok(gallery) => write_output(&gallery)
            .map(|()| STATUS_READY)
            .unwrap_or(ERROR_OUTPUT),
        Err(_) => ERROR_OUTPUT,
    }
}

/// Projects the exact checked Form beside its Book source without planning or
/// starting a Play.
#[no_mangle]
pub extern "C" fn conduit_book_project_patchbay(source_length: usize, sequence: u64) -> i32 {
    project_patchbay(source_length, sequence, false)
}

/// Projects the same visible checked Form while retaining distinct recursive
/// expansion evidence for the comparison lesson.
#[no_mangle]
pub extern "C" fn conduit_book_project_patchbay_recursive(
    source_length: usize,
    sequence: u64,
) -> i32 {
    project_patchbay(source_length, sequence, true)
}

fn project_patchbay(source_length: usize, sequence: u64, recursive: bool) -> i32 {
    clear_output();
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = core::str::from_utf8(&input[..source_length])
            .map_err(|_| "compact Tour Patchbay source is not UTF-8".to_owned())
            .and_then(|source| super::compact_patchbay::project(source, sequence, recursive));
        input[..source_length].fill(0);
        match result {
            Ok(projection) => write_output(&projection)
                .map(|()| STATUS_READY)
                .unwrap_or(ERROR_OUTPUT),
            Err(message) => {
                let _ = write_output(&super::refusal(message));
                ERROR_PROJECTION
            }
        }
    })
}

/// Admits the exact editable source through the portable typed human-interaction
/// flow before parsing. The retained evidence contains byte count and exact
/// identities, never the submitted source text.
#[no_mangle]
pub extern "C" fn conduit_book_admit_source_interaction(
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
                if write_output(&evidence).is_err() {
                    return ERROR_OUTPUT;
                }
                SOURCE_INTERACTION.with(|slot| *slot.borrow_mut() = Some(evidence));
                STATUS_READY
            }
            Err(message) => {
                let _ = write_output(&super::refusal(message));
                ERROR_INTERACTION
            }
        }
    })
}

/// Starts one exact Play from adjacent UTF-8 Host, Boot, and Form source bytes.
/// Replacing an unfinished session explicitly cancels its kernel scheduler.
#[no_mangle]
pub extern "C" fn conduit_book_start(
    host_length: usize,
    boot_length: usize,
    source_length: usize,
    play_sequence: u64,
) -> i32 {
    start(
        host_length,
        boot_length,
        source_length,
        play_sequence,
        false,
    )
}

/// Starts the same authored Form while selecting reviewed reusable Backs.
#[no_mangle]
pub extern "C" fn conduit_book_start_recursive(
    host_length: usize,
    boot_length: usize,
    source_length: usize,
    play_sequence: u64,
) -> i32 {
    start(host_length, boot_length, source_length, play_sequence, true)
}

fn start(
    host_length: usize,
    boot_length: usize,
    source_length: usize,
    play_sequence: u64,
    recursive: bool,
) -> i32 {
    clear_output();
    let source_interaction = SOURCE_INTERACTION.with(|slot| slot.borrow_mut().take());
    let Some(identity_length) = host_length.checked_add(boot_length) else {
        return ERROR_INPUT;
    };
    let Some(total_length) = identity_length.checked_add(source_length) else {
        return ERROR_INPUT;
    };
    if host_length == 0 || boot_length == 0 || source_length == 0 || total_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    let Some(source_interaction) = source_interaction else {
        let _ = write_output(&super::refusal(
            "source interaction was not admitted before parsing".into(),
        ));
        return ERROR_INTERACTION;
    };
    SESSION.with(|slot| {
        if let Some(previous) = slot.borrow_mut().take() {
            let _ = previous.cancel();
        }
    });
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = (|| {
            let host = core::str::from_utf8(&input[..host_length]).map_err(|_| ERROR_INPUT)?;
            let boot = core::str::from_utf8(&input[host_length..identity_length])
                .map_err(|_| ERROR_INPUT)?;
            let source = core::str::from_utf8(&input[identity_length..total_length])
                .map_err(|_| ERROR_INPUT)?;
            let verified_interaction = crate::source_interaction::admit_source(
                source.as_bytes(),
                source_interaction.sequence,
            )
            .map_err(|message| {
                let _ = write_output(&super::refusal(message));
                ERROR_INTERACTION
            })?;
            if verified_interaction.proposal_identity != source_interaction.proposal_identity {
                write_output(&super::refusal(
                    "source changed after typed interaction admission".into(),
                ))
                .map_err(|_| ERROR_OUTPUT)?;
                return Err(ERROR_INTERACTION);
            }
            let prepared = if recursive {
                BookSession::prepare_recursive(host, boot, source, play_sequence)
            } else {
                BookSession::prepare(host, boot, source, play_sequence)
            };
            let (mut session, mut effect) = match prepared {
                Ok(prepared) => prepared,
                Err(message) => {
                    write_output(&super::refusal(message)).map_err(|_| ERROR_OUTPUT)?;
                    return Err(ERROR_PREPARE);
                }
            };
            session.attach_source_interaction(&mut effect, source_interaction);
            write_output(&effect).map_err(|_| ERROR_OUTPUT)?;
            SESSION.with(|slot| *slot.borrow_mut() = Some(session));
            Ok(STATUS_READY)
        })();
        input[..total_length].fill(0);
        result.unwrap_or_else(|error| error)
    })
}

#[no_mangle]
pub extern "C" fn conduit_book_complete() -> i32 {
    finish(false)
}

#[no_mangle]
pub extern "C" fn conduit_book_complete_with_output(output_length: usize) -> i32 {
    clear_output();
    if output_length == 0 || output_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    SESSION.with(|slot| {
        let Some(mut session) = slot.borrow_mut().take() else {
            return ERROR_NOT_RUNNING;
        };
        INPUT.with(|input| {
            let mut input = input.borrow_mut();
            let progress = session
                .advance_with_output(&input[..output_length])
                .map_err(|_| ERROR_COMPLETE);
            input[..output_length].fill(0);
            match progress.and_then(|progress| {
                let pending = matches!(progress, super::BookProgress::Effect(_));
                write_output(&progress).map_err(|_| ERROR_OUTPUT)?;
                if pending {
                    *slot.borrow_mut() = Some(session);
                }
                Ok(())
            }) {
                Ok(()) => STATUS_READY,
                Err(error) => error,
            }
        })
    })
}

#[no_mangle]
pub extern "C" fn conduit_book_cancel() -> i32 {
    finish(true)
}

fn finish(cancel: bool) -> i32 {
    clear_output();
    SESSION.with(|slot| {
        let Some(mut session) = slot.borrow_mut().take() else {
            return ERROR_NOT_RUNNING;
        };
        if cancel {
            return match session
                .cancel()
                .map(|receipt| super::BookProgress::Receipt(Box::new(receipt)))
                .map_err(|_| ERROR_CANCEL)
                .and_then(|progress| write_output(&progress).map_err(|_| ERROR_OUTPUT))
            {
                Ok(()) => STATUS_READY,
                Err(error) => error,
            };
        }
        let progress = session.advance().map_err(|_| ERROR_COMPLETE);
        match progress.and_then(|progress| {
            let pending = matches!(progress, super::BookProgress::Effect(_));
            write_output(&progress).map_err(|_| ERROR_OUTPUT)?;
            if pending {
                *slot.borrow_mut() = Some(session);
            }
            Ok(())
        }) {
            Ok(()) => STATUS_READY,
            Err(error) => error,
        }
    })
}

fn write_output(value: &impl serde::Serialize) -> Result<(), ()> {
    let encoded = serde_json::to_vec(value).map_err(|_| ())?;
    write_output_bytes(&encoded)
}

fn write_output_bytes(encoded: &[u8]) -> Result<(), ()> {
    if encoded.len() > OUTPUT_BYTES {
        return Err(());
    }
    OUTPUT.with(|output| {
        output.borrow_mut()[..encoded.len()].copy_from_slice(encoded);
    });
    OUTPUT_LEN.with(|length| *length.borrow_mut() = encoded.len());
    Ok(())
}

fn clear_output() {
    OUTPUT_LEN.with(|length| *length.borrow_mut() = 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_book_start_is_refused_without_a_session() {
        assert_eq!(conduit_book_start(0, 0, 0, 0), ERROR_INPUT);
        assert_eq!(conduit_book_complete(), ERROR_NOT_RUNNING);

        INPUT.with(|input| input.borrow_mut()[..3].copy_from_slice(b"one"));
        assert_eq!(conduit_book_admit_source_interaction(3, 1), STATUS_READY);
        INPUT.with(|input| input.borrow_mut()[..5].copy_from_slice(b"hbTwo"));
        assert_eq!(conduit_book_start(1, 1, 3, 1), ERROR_INTERACTION);
        let refusal: serde_json::Value = OUTPUT.with(|output| {
            serde_json::from_slice(&output.borrow()[..conduit_book_output_len()]).unwrap()
        });
        assert_eq!(
            refusal["message"],
            "source changed after typed interaction admission"
        );
        assert_eq!(conduit_book_complete(), ERROR_NOT_RUNNING);

        assert_eq!(conduit_book_inventory(), STATUS_READY);
        assert!(conduit_book_output_len() > 0);
        assert_eq!(conduit_book_human_machinery(), STATUS_READY);
        let machinery: serde_json::Value = OUTPUT.with(|output| {
            serde_json::from_slice(&output.borrow()[..conduit_book_output_len()]).unwrap()
        });
        assert_eq!(
            machinery["schema"],
            "conduit.browser/selected-human-machinery@1"
        );
        assert_eq!(machinery["implementations"].as_array().unwrap().len(), 3);
    }
}
