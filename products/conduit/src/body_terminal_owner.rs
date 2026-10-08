//! One foreground terminal provider of the actual installed owner.
//! The owner alone prepares and acknowledges the ordinary Mask Show and its
//! typed action return through the retained Mask Play.

use crate::durable_host_control::terminal_attach::TerminalWardrobeCommand;
use conduit_presentation::{FaceInteraction, FaceInteractionArgument, MaskShow, Presentation};
use std::{
    io::{BufRead, Read, Write},
    path::Path,
    thread,
    time::{Duration, Instant},
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
    let mut wardrobe = crate::durable_host_control::attached_wardrobe(
        state_dir,
        &attached,
        0,
        TerminalWardrobeCommand::Inspect,
    )?;
    report_wardrobe(output, &wardrobe)?;
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
            // The service observes the closed provider socket on its next
            // turn. Observe that retirement before reporting a clean exit.
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let (_, retired) = crate::durable_host_control::local_face_snapshot(state_dir)?;
                if retired.host_id != attached_host || retired.boot_id != attached_boot {
                    return Err("owner terminal Host Boot changed during detachment".into());
                }
                if retired.offer_generation > attached_generation {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("owner terminal detachment was not acknowledged".into());
                }
                thread::sleep(Duration::from_millis(20));
            }
            return Ok(());
        }
        if length as u64 > MAX_COMMAND_BYTES {
            return Err("owner terminal command exceeds 256 bytes".into());
        }
        let command = std::str::from_utf8(&bytes)
            .map_err(|_| "owner terminal command must be UTF-8".to_string())?
            .trim();
        let wardrobe_command = match command {
            "wardrobe" => Some(TerminalWardrobeCommand::Inspect),
            "wardrobe wear" => Some(TerminalWardrobeCommand::Wear),
            "wardrobe doff" => Some(TerminalWardrobeCommand::Doff),
            "wardrobe prefer" => Some(TerminalWardrobeCommand::Prefer),
            _ => None,
        };
        if let Some(action) = wardrobe_command {
            let revision = wardrobe["wardrobe"]["revision"]
                .as_u64()
                .ok_or("owner wardrobe report omitted revision")?;
            match crate::durable_host_control::attached_wardrobe(
                state_dir, &attached, revision, action,
            ) {
                Ok(report) => {
                    wardrobe = report;
                    report_wardrobe(output, &wardrobe)?;
                }
                Err(error) => writeln!(output, "Wardrobe refused: {error}")
                    .map_err(|error| format!("write wardrobe refusal: {error}"))?,
            }
            continue;
        }
        if command == "show" {
            crate::durable_host_control::terminal_attach::refresh_show(
                state_dir,
                &mut attached,
                output,
            )?;
            report_show(output, &attached)?;
            wardrobe = crate::durable_host_control::attached_wardrobe(
                state_dir,
                &attached,
                0,
                TerminalWardrobeCommand::Inspect,
            )?;
            report_wardrobe(output, &wardrobe)?;
            continue;
        }
        if command == "actions" {
            report_actions(output, &attached.face)?;
            continue;
        }
        let action_input = command.strip_prefix("action ").map(|input| {
            input
                .split_once(' ')
                .map_or((input, ""), |(id, value)| (id, value))
        });
        if let Some((identity, value)) = action_input
            .map(|(id, value)| (Some(id), value))
            .or_else(|| command.strip_prefix("apply ").map(|value| (None, value)))
        {
            if wardrobe["show_id"].is_null() {
                writeln!(output, "Action refused: no current selected Mask Show. Enter show to present a fresh Show on this terminal.")
                    .map_err(|error| format!("write stale Show refusal: {error}"))?;
                continue;
            }
            let interaction =
                interaction_for_action(&attached.face, &attached.show, identity, value);
            let interaction = match interaction {
                Ok(interaction) => interaction,
                Err(error) => {
                    writeln!(output, "Action refused: {error}")
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
                Ok(value) if value["schema"] == "conduit.todo/committed-action@1" => writeln!(
                    output,
                    "Todo saved at revision {}.",
                    value["state_revision"]
                ),
                Ok(value) => writeln!(output, "{value}"),
                Err(error) => writeln!(output, "Action refused: {error}"),
            }
            .map_err(|error| format!("write owner terminal action: {error}"))?;
            drop(attached);
            attached =
                crate::durable_host_control::terminal_attach::attach_and_show(state_dir, output)?;
            report_show(output, &attached)?;
            wardrobe = crate::durable_host_control::attached_wardrobe(
                state_dir,
                &attached,
                0,
                TerminalWardrobeCommand::Inspect,
            )?;
            report_wardrobe(output, &wardrobe)?;
            continue;
        }
        writeln!(
            output,
            "Enter wardrobe (inspect), wardrobe wear/doff/prefer (this terminal Mask only), show to present again, actions to list choices, action <identity> [value], apply <value>, or quit to detach."
        )
        .map_err(|error| format!("write owner terminal help: {error}"))?;
    }
}

fn report_actions(output: &mut impl Write, face: &Presentation) -> Result<(), String> {
    let mut count = 0;
    for action in face
        .actions
        .iter()
        .filter(|action| action.availability.is_available())
    {
        let target = face
            .subjects
            .iter()
            .find(|subject| subject.identity == action.target)
            .map_or(action.target.as_str(), |subject| subject.name.as_str());
        let argument = action
            .arguments
            .first()
            .map_or(String::new(), |argument| format!(" <{}>", argument.name));
        writeln!(
            output,
            "{} — {}: action {}{}",
            action.name, target, action.identity, argument
        )
        .map_err(|error| format!("write terminal actions: {error}"))?;
        count += 1;
    }
    if count == 0 {
        writeln!(output, "No available actions on this Face.")
            .map_err(|error| format!("write terminal actions: {error}"))?;
    }
    output
        .flush()
        .map_err(|error| format!("flush terminal actions: {error}"))
}

fn interaction_for_action(
    face: &Presentation,
    show: &MaskShow,
    identity: Option<&str>,
    value: &str,
) -> Result<FaceInteraction, String> {
    let mut available = face.actions.iter().filter(|action| {
        action.availability.is_available()
            && identity.is_none_or(|id| action.identity.as_str() == id)
    });
    let selected = available.next();
    if available.next().is_some()
        || selected.is_none_or(|action| {
            action.arguments.len() > 1
                || (identity.is_none() && action.arguments.len() != 1)
                || (action.arguments.is_empty() && !value.is_empty())
        })
    {
        return Err("Choose one available action: action <identity> [value]. Apply needs exactly one available, single-value action.".into());
    }
    let action = selected.expect("checked one available action");
    let arguments = action
        .arguments
        .iter()
        .map(|argument| FaceInteractionArgument {
            name: argument.name.clone(),
            value_kind: argument.contract.value_kind.as_str().into(),
            value: value.as_bytes().to_vec(),
        })
        .collect();
    FaceInteraction::new(face, show, &action.identity, &action.target, arguments, 1)
        .map_err(|error| format!("{error:?}"))
}

#[cfg(test)]
mod tests;

fn report_wardrobe(output: &mut impl Write, report: &serde_json::Value) -> Result<(), String> {
    let worn = report["wardrobe"]["worn"]
        .as_array()
        .ok_or("owner wardrobe omitted worn Masks")?;
    let selected = report["selected"]["route_id"].as_str().unwrap_or("none");
    let show = report["show_id"].as_str().unwrap_or("none");
    let planning = &report["reconciliation"]["planning"];
    writeln!(
        output,
        "Owner Body wardrobe: {} worn terminal Mask, {} admitted route; selected route: {selected}; current Show: {show}; planning: {planning}. Preference: {}. Wear/doff/prefer address the currently admitted terminal Mask; browser selection awaits a complete carrier-Line Mask Plan.",
        worn.len(),
        report["admitted_routes"].as_array().map_or(0, Vec::len),
        report["wardrobe"]["preference"],
    )
    .map_err(|error| format!("write owner wardrobe: {error}"))?;
    if report["fresh_show_required"] == true {
        writeln!(output, "The sealed terminal route is selected, but its prior Show is no longer current. Enter show for a fresh acknowledged Show.")
            .map_err(|error| format!("write wardrobe Show status: {error}"))?;
    }
    output
        .flush()
        .map_err(|error| format!("flush owner wardrobe: {error}"))
}

fn report_show(
    output: &mut impl Write,
    attached: &crate::durable_host_control::terminal_attach::AttachedTerminalSession,
) -> Result<(), String> {
    writeln!(
        output,
        "\r\nOwner terminal Show {} · route Plan {} · Host {} · Boot {} · offer generation {} · {} bytes written and flushed. Enter actions to list choices, action <identity> [value] for an available action, apply <value> when only one value action is available, show to present again, or quit to detach.",
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
