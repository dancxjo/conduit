//! One foreground terminal provider of the actual installed owner.
//! The owner alone prepares and acknowledges the ordinary Mask Show and its
//! typed action return through the retained Mask Play.

use conduit_presentation::{FaceInteraction, FaceInteractionArgument};
use std::{
    io::{BufRead, Read, Write},
    path::Path,
};

const MAX_COMMAND_BYTES: u64 = 256;

pub(crate) fn run(
    state_dir: &Path,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    let mut attached =
        crate::durable_host_control::terminal_attach::attach_and_show(state_dir, output)?;
    report_show(output, &attached)?;
    loop {
        let mut bytes = Vec::new();
        let length = (&mut *input)
            .take(MAX_COMMAND_BYTES + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|error| format!("read owner terminal command: {error}"))?;
        if length == 0 || bytes == b"quit\n" || bytes == b"quit\r\n" {
            let attached_host = attached.advertisement.host_id.clone();
            let attached_boot = attached.advertisement.boot_id.clone();
            let attached_generation = attached.advertisement.offer_generation;
            drop(attached);
            let (_, retired) = crate::durable_host_control::local_face_snapshot(state_dir)?;
            if retired.host_id != attached_host
                || retired.boot_id != attached_boot
                || retired.offer_generation <= attached_generation
            {
                return Err("owner terminal detachment was not acknowledged".into());
            }
            return Ok(());
        }
        if length as u64 > MAX_COMMAND_BYTES {
            return Err("owner terminal command exceeds 256 bytes".into());
        }
        let command = std::str::from_utf8(&bytes)
            .map_err(|_| "owner terminal command must be UTF-8".to_string())?
            .trim();
        if let Some(value) = command.strip_prefix("apply ") {
            let mut available = attached
                .face
                .actions
                .iter()
                .filter(|action| action.availability.is_available());
            let selected = available.next();
            if available.next().is_some()
                || selected.is_none_or(|action| action.arguments.len() != 1)
            {
                writeln!(
                    output,
                    "Apply needs exactly one available, single-value action."
                )
                .map_err(|error| format!("write owner terminal help: {error}"))?;
                continue;
            }
            let action = selected.expect("checked one available action");
            let argument = &action.arguments[0];
            let interaction = FaceInteraction::new(
                &attached.face,
                &attached.show,
                &action.identity,
                &action.target,
                vec![FaceInteractionArgument {
                    name: argument.name.clone(),
                    value_kind: argument.contract.value_kind.as_str().into(),
                    value: value.as_bytes().to_vec(),
                }],
                1,
            );
            let interaction = match interaction {
                Ok(interaction) => interaction,
                Err(error) => {
                    writeln!(output, "Action refused: {error:?}")
                        .map_err(|error| format!("write owner terminal refusal: {error}"))?;
                    continue;
                }
            };
            let result = crate::durable_host_control::submit_attached_terminal_interaction(
                state_dir,
                attached.route_plan_id.clone(),
                attached.show.clone(),
                interaction,
            );
            match result {
                Ok(value) => writeln!(output, "{value}"),
                Err(error) => writeln!(output, "Action refused: {error}"),
            }
            .map_err(|error| format!("write owner terminal action: {error}"))?;
            drop(attached);
            attached =
                crate::durable_host_control::terminal_attach::attach_and_show(state_dir, output)?;
            report_show(output, &attached)?;
            continue;
        }
        writeln!(
            output,
            "Enter apply <value> for the available action, or quit to detach."
        )
        .map_err(|error| format!("write owner terminal help: {error}"))?;
    }
}

fn report_show(
    output: &mut impl Write,
    attached: &crate::durable_host_control::terminal_attach::AttachedTerminalSession,
) -> Result<(), String> {
    writeln!(
        output,
        "\r\nOwner terminal Show {} · route Plan {} · Host {} · Boot {} · offer generation {} · {} bytes written and flushed. Enter apply <value> for the available action, or quit to detach.",
        attached.show.show_id.as_str(),
        attached.route_plan_id.as_str(),
        attached.advertisement.host_id.as_str(),
        attached.advertisement.boot_id.as_str(),
        attached.advertisement.offer_generation.0,
        attached.effect.bytes_written,
    )
    .map_err(|error| format!("write owner terminal receipt: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("flush owner terminal receipt: {error}"))
}
