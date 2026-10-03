//! Local terminal Mask for the installed owner's current authoritative Face.
//! Local navigation is presentation state. Actions cross the ordinary terminal
//! Mask interaction Fore and then the authenticated current owner control route.

use std::{
    io::{BufRead, Read, Write},
    path::Path,
};

use conduit_core::HostAdvertisement;
use conduit_presentation::Presentation;
use conduit_std_host::terminal_face_mask::{
    TerminalFaceMask, TerminalInput, TerminalInputOutcome, TerminalMaskExecution,
};
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;

const COLUMNS: usize = 80;
const ROWS: usize = 24;
const MAX_COMMAND_BYTES: u64 = 256;

pub(crate) fn run(
    state_dir: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    let (mut face, mut host) = crate::durable_host_control::local_face_snapshot(state_dir)?;
    let (mut mask, mut execution) = prepare(&face, &host)?;
    present(&mut mask, &mut execution, output, &host)?;
    writeln!(output, "Commands: next, previous, page down, page up, inspect, control next, control previous, type TEXT, apply, refresh, help, quit. The clock interval action is available after lull.")
        .map_err(io_error)?;
    let mut sequence = 0_u64;
    loop {
        write!(output, "body> ").map_err(io_error)?;
        output.flush().map_err(io_error)?;
        let mut bytes = Vec::new();
        let length = match (&mut *input)
            .take(MAX_COMMAND_BYTES + 1)
            .read_until(b'\n', &mut bytes)
        {
            Ok(length) => length,
            Err(error) => {
                let _ = execution.cancel();
                return Err(io_error(error));
            }
        };
        if length == 0 {
            execution.close_without_input().map_err(debug_error)?;
            return Ok(());
        }
        if length as u64 > MAX_COMMAND_BYTES {
            execution.cancel().map_err(debug_error)?;
            return Err("terminal command exceeds 256 bytes".into());
        }
        let command = match std::str::from_utf8(&bytes) {
            Ok(command) => command.trim_end_matches(['\r', '\n']),
            Err(_) => {
                let _ = execution.cancel();
                return Err("terminal command is not UTF-8".into());
            }
        };
        match command {
            "quit" => {
                execution.close_without_input().map_err(debug_error)?;
                return Ok(());
            }
            "help" => {
                writeln!(output, "Next/previous read the Face; page down/up scroll; inspect shows precise facts; control next/previous focuses an available action; type TEXT edits its value; apply submits its exact Face and Show through the owner; refresh requests the current revision; quit closes this Mask.")
                    .map_err(io_error)?;
            }
            "refresh" => {
                let (current_face, current_host) =
                    match crate::durable_host_control::local_face_snapshot(state_dir) {
                        Ok(snapshot) => snapshot,
                        Err(error) => {
                            let _ = execution.cancel();
                            return Err(error);
                        }
                    };
                if current_face == face && current_host == host {
                    writeln!(
                        output,
                        "Owner Face is unchanged at revision {}.",
                        face.revision
                    )
                    .map_err(io_error)?;
                    continue;
                }
                execution.close_without_input().map_err(debug_error)?;
                face = current_face;
                host = current_host;
                (mask, execution) = prepare(&face, &host)?;
                present(&mut mask, &mut execution, output, &host)?;
            }
            "next" | "previous" | "page down" | "page up" | "inspect" | "control next"
            | "control previous" => {
                sequence = sequence
                    .checked_add(1)
                    .ok_or("terminal input sequence exhausted")?;
                let key = match command {
                    "next" => TerminalInput::NextClause,
                    "previous" => TerminalInput::PreviousClause,
                    "page down" => TerminalInput::PageDown,
                    "page up" => TerminalInput::PageUp,
                    "control next" => TerminalInput::NextControl,
                    "control previous" => TerminalInput::PreviousControl,
                    _ => TerminalInput::Inspect,
                };
                let show = mask
                    .show()
                    .ok_or("terminal has no acknowledged Show")?
                    .clone();
                if mask.input(key, &show, sequence).map_err(debug_error)?
                    == TerminalInputOutcome::Redraw
                {
                    execution.close_without_input().map_err(debug_error)?;
                    present(&mut mask, &mut execution, output, &host)?;
                }
            }
            "apply" => {
                sequence = sequence
                    .checked_add(1)
                    .ok_or("terminal input sequence exhausted")?;
                let show = mask
                    .show()
                    .ok_or("terminal has no acknowledged Show")?
                    .clone();
                match mask.input(TerminalInput::Apply, &show, sequence) {
                    Ok(TerminalInputOutcome::Interaction(interaction)) => {
                        let correlated = execution.interact(interaction).map_err(debug_error)?;
                        match crate::durable_host_control::submit_local_face_interaction(
                            state_dir,
                            show,
                            correlated.interaction,
                        ) {
                            Ok(result) => writeln!(output, "{}", result).map_err(io_error)?,
                            Err(refusal) => {
                                writeln!(output, "Action refused: {refusal}").map_err(io_error)?
                            }
                        }
                        (face, host) = crate::durable_host_control::local_face_snapshot(state_dir)?;
                        (mask, execution) = prepare(&face, &host)?;
                        present(&mut mask, &mut execution, output, &host)?;
                    }
                    Ok(_) => writeln!(output, "No action was submitted.").map_err(io_error)?,
                    Err(refusal) => {
                        writeln!(output, "Action refused: {refusal}").map_err(io_error)?
                    }
                }
            }
            value if value.starts_with("type ") => {
                let typed = &value[5..];
                if typed.is_empty() || typed.len() > 32 {
                    writeln!(output, "Type 1 to 32 UTF-8 bytes.").map_err(io_error)?;
                    continue;
                }
                for ch in typed.chars() {
                    sequence = sequence
                        .checked_add(1)
                        .ok_or("terminal input sequence exhausted")?;
                    let show = mask
                        .show()
                        .ok_or("terminal has no acknowledged Show")?
                        .clone();
                    match mask.input(TerminalInput::Text(ch), &show, sequence) {
                        Ok(TerminalInputOutcome::Redraw) => {
                            execution.close_without_input().map_err(debug_error)?;
                            present(&mut mask, &mut execution, output, &host)?;
                        }
                        Ok(_) => {}
                        Err(refusal) => {
                            writeln!(output, "Input refused: {refusal}").map_err(io_error)?;
                            break;
                        }
                    }
                }
            }
            _ => {
                writeln!(output, "Unknown command; enter help.").map_err(io_error)?;
            }
        }
    }
}

fn prepare(
    face: &Presentation,
    host: &HostAdvertisement,
) -> Result<(TerminalFaceMask, HostedTerminalMaskExecution), String> {
    let admitted = face.actions.iter().any(|action| {
        action.intent == crate::durable_host::owner::clock_interval_action()
            && action.availability.is_available()
    });
    let mask = if admitted {
        TerminalFaceMask::prepare(face.clone(), COLUMNS, ROWS)
    } else {
        TerminalFaceMask::prepare_read_only(face.clone(), COLUMNS, ROWS)
    }
    .map_err(debug_error)?;
    let execution = HostedTerminalMaskExecution::new(host).map_err(debug_error)?;
    Ok((mask, execution))
}

fn present(
    mask: &mut TerminalFaceMask,
    execution: &mut HostedTerminalMaskExecution,
    output: &mut impl Write,
    host: &HostAdvertisement,
) -> Result<(), String> {
    mask.present(execution, output).map_err(debug_error)?;
    let show = mask.show().ok_or("terminal did not acknowledge a Show")?;
    let status = writeln!(
        output,
        "\r\nOwner Face revision {} · Show {} · Host {} · Boot {}",
        mask.presentation().revision,
        show.show_id.as_str(),
        host.host_id.as_str(),
        host.boot_id.as_str(),
    );
    if let Err(error) = status {
        let _ = execution.cancel();
        return Err(io_error(error));
    }
    Ok(())
}

fn debug_error(error: impl std::fmt::Debug) -> String {
    format!("terminal Mask: {error:?}")
}
fn io_error(error: std::io::Error) -> String {
    format!("terminal I/O: {error}")
}
