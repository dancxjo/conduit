//! Nonvisual client of the installed Host's one authoritative Birth encounter.

use std::{
    io::{BufRead, Write},
    path::Path,
};

use conduit_presentation::{MaskShow, Presentation};
use conduit_std_host::spoken_face_mask::{ReaderCommand, SpokenFaceSession};
use conduit_std_host::terminal_face_mask::{TerminalFaceMask, TerminalMaskExecution};
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;

use super::{
    debug_error,
    input::{parse_command, read_command_line, SCREEN_FREE_COMMANDS},
};
use crate::durable_host_control::{self, BirthTransition};

pub(crate) fn run_installed(
    state_dir: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    let (mut face, advertisement) = durable_host_control::birth_face(state_dir)?;
    let mut execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
    let mut show = present(&face, &mut execution, output)?;
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(debug_error)?;
    writeln!(output, "Installed Host screen-free Birth. Text readout; no speech audio has been produced. {SCREEN_FREE_COMMANDS}")
        .map_err(|error| error.to_string())?;
    let mut sequence = 1_u64;
    read_all(&mut reader, &face, &show, sequence, output)?;

    loop {
        write!(output, "birth> ").map_err(|error| error.to_string())?;
        output.flush().map_err(|error| error.to_string())?;
        let line = match read_command_line(input).map_err(|error| error.to_string())? {
            Ok(Some(line)) => line,
            Ok(None) => {
                execution.close_without_input().map_err(debug_error)?;
                return Ok(());
            }
            Err(message) => {
                writeln!(output, "Refused input: {message}").map_err(|error| error.to_string())?;
                continue;
            }
        };
        if line == "quit" {
            execution.close_without_input().map_err(debug_error)?;
            return Ok(());
        }
        let command = match parse_command(&line, &reader, &face) {
            Ok(command) => command,
            Err(message) => {
                writeln!(output, "Refused input: {message}").map_err(|error| error.to_string())?;
                continue;
            }
        };
        sequence = sequence.checked_add(1).ok_or("input sequence exhausted")?;
        let result = match reader.command(&face, &show, command, sequence) {
            Ok(result) => result,
            Err(refusal) => {
                writeln!(output, "Refused action: {refusal:?}")
                    .map_err(|error| error.to_string())?;
                continue;
            }
        };
        super::emit_readout(&mut reader, &face, &show, None, output)?;
        let Some(interaction) = result.interaction else {
            continue;
        };
        let correlated = execution.interact(interaction).map_err(debug_error)?;
        let transition = match durable_host_control::submit_birth_interaction(
            state_dir,
            show.clone(),
            correlated.interaction,
        ) {
            Ok(transition) => transition,
            Err(refusal) => {
                if refusal == durable_host_control::CONTROL_OUTCOME_UNKNOWN {
                    return Err(refusal);
                }
                writeln!(output, "Installed Birth refused action: {refusal}")
                    .map_err(|error| error.to_string())?;
                let (current, host) = durable_host_control::birth_face(state_dir)?;
                if host != advertisement {
                    return Err("installed Host changed during Birth refusal".into());
                }
                face = current;
                execution =
                    HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
                show = present(&face, &mut execution, output)?;
                reader
                    .refresh(face.clone(), show.clone())
                    .map_err(debug_error)?;
                super::emit_readout(&mut reader, &face, &show, None, output)?;
                continue;
            }
        };
        match transition {
            BirthTransition::Changed(next) => {
                face = next;
                execution =
                    HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
                show = present(&face, &mut execution, output)?;
                reader
                    .refresh(face.clone(), show.clone())
                    .map_err(debug_error)?;
                super::emit_readout(&mut reader, &face, &show, None, output)?;
            }
            BirthTransition::Born {
                body_id,
                presentation,
            } => {
                if presentation.basis.body_id.as_ref() != Some(&body_id) {
                    return Err("retained Birth response names a different Body".into());
                }
                writeln!(
                    output,
                    "Body retained by this installed Host: {}",
                    body_id.as_str()
                )
                .map_err(|error| error.to_string())?;
                return run_body(state_dir, body_id, presentation, input, output);
            }
        }
    }
}

fn run_body(
    state_dir: &Path,
    body_id: conduit_body::BodyId,
    mut face: Presentation,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    let (current, mut advertisement) = durable_host_control::local_face_snapshot(state_dir)?;
    if current.basis.body_id.as_ref() != Some(&body_id) || current.identity != face.identity {
        return Err("retained owner Face changed before the Body session began".into());
    }
    face = current;
    let mut execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
    let mut show = present(&face, &mut execution, output)?;
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(debug_error)?;
    writeln!(
        output,
        "Continuing retained Body {}. Commands: {SCREEN_FREE_COMMANDS} refresh.",
        body_id.as_str()
    )
    .map_err(|error| error.to_string())?;
    let mut sequence = 1_u64;
    read_all(&mut reader, &face, &show, sequence, output)?;
    loop {
        write!(output, "body> ").map_err(|error| error.to_string())?;
        output.flush().map_err(|error| error.to_string())?;
        let line = match read_command_line(input).map_err(|error| error.to_string())? {
            Ok(Some(line)) => line,
            Ok(None) => {
                execution.close_without_input().map_err(debug_error)?;
                return Ok(());
            }
            Err(message) => {
                writeln!(output, "Refused input: {message}").map_err(|error| error.to_string())?;
                continue;
            }
        };
        if line == "quit" {
            execution.close_without_input().map_err(debug_error)?;
            return Ok(());
        }
        let (current, host) = durable_host_control::local_face_snapshot(state_dir)?;
        if current.basis.body_id.as_ref() != Some(&body_id) {
            return Err("installed owner changed Body identity".into());
        }
        if current.identity != face.identity
            || current.revision != face.revision
            || host != advertisement
        {
            execution.close_without_input().map_err(debug_error)?;
            face = current;
            advertisement = host;
            execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
            show = present(&face, &mut execution, output)?;
            reader
                .refresh(face.clone(), show.clone())
                .map_err(debug_error)?;
            writeln!(output, "Owner Face changed to revision {}.", face.revision)
                .map_err(|error| error.to_string())?;
            super::emit_readout(&mut reader, &face, &show, None, output)?;
        }
        if line == "refresh" {
            writeln!(output, "Owner Face revision {} is current.", face.revision)
                .map_err(|error| error.to_string())?;
            continue;
        }
        let command = match parse_command(&line, &reader, &face) {
            Ok(command) => command,
            Err(message) => {
                writeln!(output, "Refused input: {message}").map_err(|error| error.to_string())?;
                continue;
            }
        };
        sequence = sequence.checked_add(1).ok_or("input sequence exhausted")?;
        let result = match reader.command(&face, &show, command, sequence) {
            Ok(result) => result,
            Err(refusal) => {
                writeln!(output, "Refused action: {refusal:?}")
                    .map_err(|error| error.to_string())?;
                continue;
            }
        };
        super::emit_readout(&mut reader, &face, &show, None, output)?;
        if let Some(interaction) = result.interaction {
            let correlated = execution.interact(interaction).map_err(debug_error)?;
            match durable_host_control::submit_local_face_interaction(
                state_dir,
                show.clone(),
                correlated.interaction,
            ) {
                Ok(result) => writeln!(output, "Owner action result: {result}")
                    .map_err(|error| error.to_string())?,
                Err(refusal) => writeln!(output, "Owner action refused: {refusal}")
                    .map_err(|error| error.to_string())?,
            }
            let (current, host) = durable_host_control::local_face_snapshot(state_dir)?;
            if current.basis.body_id.as_ref() != Some(&body_id) {
                return Err("installed owner changed Body identity".into());
            }
            face = current;
            advertisement = host;
            execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
            show = present(&face, &mut execution, output)?;
            reader
                .refresh(face.clone(), show.clone())
                .map_err(debug_error)?;
            super::emit_readout(&mut reader, &face, &show, None, output)?;
        }
    }
}

fn present(
    face: &Presentation,
    execution: &mut HostedTerminalMaskExecution,
    output: &mut impl Write,
) -> Result<MaskShow, String> {
    let mut mask = TerminalFaceMask::prepare(face.clone(), 80, 24).map_err(debug_error)?;
    mask.present(execution, output).map_err(debug_error)?;
    writeln!(output).map_err(|error| error.to_string())?;
    mask.show()
        .cloned()
        .ok_or_else(|| "terminal did not acknowledge a Show".into())
}

fn read_all(
    reader: &mut SpokenFaceSession,
    face: &Presentation,
    show: &MaskShow,
    sequence: u64,
    output: &mut impl Write,
) -> Result<(), String> {
    reader
        .command(face, show, ReaderCommand::ReadAll, sequence)
        .map_err(debug_error)?;
    super::emit_readout(reader, face, show, None, output)
}
