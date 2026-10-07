//! Retained Body screen-free session over the owner's current Face and Mask wardrobe.

use std::{io::Write, path::Path};

use conduit_core::HostAdvertisement;
use conduit_presentation::Presentation;
use conduit_std_host::spoken_face_mask::SpokenFaceSession;
use conduit_std_host::terminal_face_mask::TerminalMaskExecution;
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;

use super::{
    command_input::{CommandInput, InputEvent},
    debug_error,
    input::{parse_command, SCREEN_FREE_COMMANDS},
    installed_presentation::present,
    opening_readout::opening_body_commands,
    selected_playback::SelectedPlayback,
    selected_readout::{emit_readout, OutputPhase},
};
use crate::durable_host_control;

pub(super) fn run_body(
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
        match super::wardrobe::handle(
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
            super::wardrobe::Handled::No => {}
            super::wardrobe::Handled::ReadOnly => continue,
            super::wardrobe::Handled::Changed => {
                // A wardrobe mutation can change the selected Mask without
                // changing the Face identity. Retire the old local Show and
                // cursor anyway: `activate` must require a newly chosen focus.
                execution.close_without_input().map_err(debug_error)?;
                let (current, host) = durable_host_control::local_face_snapshot(state_dir)?;
                if current.basis.body_id.as_ref() != Some(&body_id) {
                    return Err("installed owner changed Body identity".into());
                }
                if current != face || host != advertisement {
                    wardrobe_report = None;
                }
                face = current;
                advertisement = host;
                execution =
                    HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
                show = present(&face, &mut execution, output)?;
                reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(debug_error)?;
                writeln!(output, "Wardrobe changed. The previous local Show and Face focus are retired; choose a current action again.")
                    .map_err(|error| error.to_string())?;
                continue;
            }
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
