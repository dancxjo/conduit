//! Nonvisual client of the installed Host's one authoritative Birth encounter.

use std::{
    io::{BufRead, Write},
    path::Path,
};

use conduit_core::HostAdvertisement;
use conduit_presentation::Presentation;
use conduit_std_host::spoken_face_mask::SpokenFaceSession;
use conduit_std_host::terminal_face_mask::TerminalMaskExecution;
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;

use super::installed_presentation::present;
use super::opening_readout::opening_body_commands;
pub(super) use super::opening_readout::opening_commands;
use super::{
    command_input::{CommandInput, DirectInput, InputEvent, SpokenInput},
    debug_error,
    input::{parse_command, SCREEN_FREE_COMMANDS},
    selected_playback::SelectedPlayback,
    selected_readout::{emit_readout, OutputPhase},
};
use crate::{
    cli::BirthSpeechOptions,
    durable_host_control::{self, BirthTransition},
};

pub(crate) fn run_installed(
    state_dir: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    run_with_input(state_dir, &mut DirectInput(input), output, None)
}

pub(crate) fn run_installed_spoken(
    state_dir: &Path,
    options: &BirthSpeechOptions,
    output: &mut impl Write,
) -> Result<(), String> {
    let mut input = SpokenInput::from_stdin()?;
    run_with_input(state_dir, &mut input, output, Some(options))
}

pub(crate) fn run_retained(
    state_dir: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    run_retained_with_input(state_dir, &mut DirectInput(input), output, None)
}

pub(crate) fn run_retained_spoken(
    state_dir: &Path,
    options: &BirthSpeechOptions,
    output: &mut impl Write,
) -> Result<(), String> {
    let mut input = SpokenInput::from_stdin()?;
    run_retained_with_input(state_dir, &mut input, output, Some(options))
}

fn run_retained_with_input(
    state_dir: &Path,
    input: &mut impl CommandInput,
    output: &mut impl Write,
    speech_options: Option<&BirthSpeechOptions>,
) -> Result<(), String> {
    let (face, advertisement) = durable_host_control::local_face_snapshot(state_dir)?;
    let body_id = face
        .basis
        .body_id
        .clone()
        .ok_or("installed owner has no retained Body to read")?;
    let playback = speech_options
        .map(|options| SelectedPlayback::prepare(options, &advertisement))
        .transpose()?;
    run_body(
        state_dir,
        body_id,
        face,
        Some(advertisement),
        input,
        playback,
        output,
    )
}

fn run_with_input(
    state_dir: &Path,
    input: &mut impl CommandInput,
    output: &mut impl Write,
    speech_options: Option<&BirthSpeechOptions>,
) -> Result<(), String> {
    let (mut face, advertisement) = durable_host_control::birth_face(state_dir)?;
    let playback = speech_options
        .map(|options| SelectedPlayback::prepare(options, &advertisement))
        .transpose()?;
    let mut execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
    let mut show = present(&face, &mut execution, output)?;
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(debug_error)?;
    writeln!(
        output,
        "Installed Host screen-free Birth. {} {SCREEN_FREE_COMMANDS}",
        if playback.is_some() {
            "Selected speaker playback; receipts follow each drained Play."
        } else {
            "Text readout; no speech audio has been produced."
        }
    )
    .map_err(|error| error.to_string())?;
    let mut sequence = 0_u64;
    for command in opening_commands(playback.is_some()) {
        sequence += 1;
        reader
            .command(&face, &show, command, sequence)
            .map_err(debug_error)?;
        if emit_readout(
            state_dir,
            input,
            playback.as_ref(),
            &mut reader,
            &face,
            &show,
            &advertisement,
            &mut sequence,
            OutputPhase::Birth,
            output,
        )? {
            break;
        }
    }

    loop {
        write!(output, "birth> ").map_err(|error| error.to_string())?;
        output.flush().map_err(|error| error.to_string())?;
        let line = match input.next() {
            InputEvent::Line(line) => line,
            InputEvent::Eof => {
                execution.close_without_input().map_err(debug_error)?;
                return Ok(());
            }
            InputEvent::Refused(message) => {
                writeln!(output, "Refused input: {message}").map_err(|error| error.to_string())?;
                continue;
            }
            InputEvent::Io(error) => return Err(error),
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
        emit_readout(
            state_dir,
            input,
            playback.as_ref(),
            &mut reader,
            &face,
            &show,
            &advertisement,
            &mut sequence,
            OutputPhase::Birth,
            output,
        )?;
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
                emit_readout(
                    state_dir,
                    input,
                    playback.as_ref(),
                    &mut reader,
                    &face,
                    &show,
                    &advertisement,
                    &mut sequence,
                    OutputPhase::Birth,
                    output,
                )?;
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
                emit_readout(
                    state_dir,
                    input,
                    playback.as_ref(),
                    &mut reader,
                    &face,
                    &show,
                    &advertisement,
                    &mut sequence,
                    OutputPhase::Birth,
                    output,
                )?;
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
                return run_body(
                    state_dir,
                    body_id,
                    presentation,
                    None,
                    input,
                    playback,
                    output,
                );
            }
        }
    }
}

fn run_body(
    state_dir: &Path,
    body_id: conduit_body::BodyId,
    mut face: Presentation,
    expected_host: Option<HostAdvertisement>,
    input: &mut impl CommandInput,
    playback: Option<SelectedPlayback>,
    output: &mut impl Write,
) -> Result<(), String> {
    let (current, mut advertisement) = durable_host_control::local_face_snapshot(state_dir)?;
    if current.basis.body_id.as_ref() != Some(&body_id)
        || current.identity != face.identity
        || expected_host
            .as_ref()
            .is_some_and(|host| host != &advertisement || current != face)
    {
        return Err(
            "retained owner Body, Face, or Host Boot changed before the session began".into(),
        );
    }
    face = current;
    let mut execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
    let mut show = present(&face, &mut execution, output)?;
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(debug_error)?;
    #[cfg(unix)]
    let mut wardrobe_report = None;
    writeln!(
        output,
        "Continuing retained Body {}. Commands: {SCREEN_FREE_COMMANDS} refresh, wardrobe, wardrobe wear/doff CHOICE, wardrobe prefer CHOICE ... . Inspect first and use each numbered choice or its exact route ID. Preference ranks routes; doff the current route to allow selection of the next preferred route. Selected speaker sessions announce the owner's current wardrobe and result.",
        body_id.as_str()
    )
    .map_err(|error| error.to_string())?;
    let mut sequence = 0_u64;
    for command in opening_body_commands(playback.is_some()) {
        sequence += 1;
        reader
            .command(&face, &show, command, sequence)
            .map_err(debug_error)?;
        if emit_readout(
            state_dir,
            input,
            playback.as_ref(),
            &mut reader,
            &face,
            &show,
            &advertisement,
            &mut sequence,
            OutputPhase::Body,
            output,
        )? {
            break;
        }
    }
    loop {
        write!(output, "body> ").map_err(|error| error.to_string())?;
        output.flush().map_err(|error| error.to_string())?;
        let line = match input.next() {
            InputEvent::Line(line) => line,
            InputEvent::Eof => {
                execution.close_without_input().map_err(debug_error)?;
                return Ok(());
            }
            InputEvent::Refused(message) => {
                writeln!(output, "Refused input: {message}").map_err(|error| error.to_string())?;
                continue;
            }
            InputEvent::Io(error) => return Err(error),
        };
        if line == "quit" {
            execution.close_without_input().map_err(debug_error)?;
            return Ok(());
        }
        let (current, host) = durable_host_control::local_face_snapshot(state_dir)?;
        if current.basis.body_id.as_ref() != Some(&body_id) {
            return Err("installed owner changed Body identity".into());
        }
        if current != face || host != advertisement {
            // The line was entered while the previous Face was on offer. Even
            // a bare `activate` must never be reinterpreted after refreshing
            // the reader's focus and Show against a different owner state.
            execution.close_without_input().map_err(debug_error)?;
            face = current;
            advertisement = host;
            #[cfg(unix)]
            {
                wardrobe_report = None;
            }
            execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
            show = present(&face, &mut execution, output)?;
            reader
                .refresh(face.clone(), show.clone())
                .map_err(debug_error)?;
            writeln!(
                output,
                "Owner Face or Host Boot changed. Face revision {} is current. Read-only help and read all use the new Face; actions must be chosen again.",
                face.revision
            )
                .map_err(|error| error.to_string())?;
            if line != "read all" && line != "help" {
                writeln!(
                    output,
                    "Refused the pending command because it named an older Face."
                )
                .map_err(|error| error.to_string())?;
                emit_readout(
                    state_dir,
                    input,
                    playback.as_ref(),
                    &mut reader,
                    &face,
                    &show,
                    &advertisement,
                    &mut sequence,
                    OutputPhase::Body,
                    output,
                )?;
                continue;
            }
        }
        if line == "refresh" {
            writeln!(output, "Owner Face revision {} is current.", face.revision)
                .map_err(|error| error.to_string())?;
            continue;
        }
        #[cfg(unix)]
        if super::wardrobe::handle(
            state_dir,
            &line,
            &mut wardrobe_report,
            playback
                .as_ref()
                .map(|selected| super::wardrobe_speech::Announcement {
                    state_dir,
                    input,
                    selected,
                    face: &face,
                    show: &show,
                    host: &advertisement,
                })
                .as_mut(),
            output,
        )? {
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
        emit_readout(
            state_dir,
            input,
            playback.as_ref(),
            &mut reader,
            &face,
            &show,
            &advertisement,
            &mut sequence,
            OutputPhase::Body,
            output,
        )?;
        if let Some(interaction) = result.interaction {
            writeln!(
                output,
                "Interaction: action={} face-revision={} show={}",
                interaction.action_id,
                face.revision,
                show.show_id.as_str()
            )
            .map_err(|error| error.to_string())?;
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
            #[cfg(unix)]
            {
                wardrobe_report = None;
            }
            execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
            show = present(&face, &mut execution, output)?;
            reader
                .refresh(face.clone(), show.clone())
                .map_err(debug_error)?;
            emit_readout(
                state_dir,
                input,
                playback.as_ref(),
                &mut reader,
                &face,
                &show,
                &advertisement,
                &mut sequence,
                OutputPhase::Body,
                output,
            )?;
        }
    }
}
