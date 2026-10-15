//! An explicit installed peer supplies real terminal effects for the remote Body.
use super::PresentationHost;
#[path = "wire.rs"]
mod wire;
use conduit_face::{Face, MaskShow};
use conduit_std_host::{
    hosted_terminal_mask_host::{receive_terminal_frame_and_ack, TerminalFrameReceipt},
    terminal_face_mask::TerminalMaskExecution,
    terminal_mask_execution::HostedTerminalMaskExecution,
};
use std::{
    io::{BufRead, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

struct Shown {
    face: Face,
    show: MaskShow,
    execution: HostedTerminalMaskExecution,
    effect: TerminalFrameReceipt,
    route_id: conduit_core::PlanId,
}

pub(crate) fn run(
    state: &Path,
    invitation_path: &Path,
    authorize_join: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    if !authorize_join {
        return Err("presentation admission requires --authorize-join".into());
    }
    let invitation =
        serde_json::from_slice(&super::super::bounded_read(invitation_path, 512 * 1024)?)
            .map_err(|error| format!("presentation invitation:{error}"))?;
    let mut peer = PresentationHost::prepare(state)?;
    let (provider, mut foreground) = UnixStream::pair().map_err(io_error)?;
    foreground
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(io_error)?;
    foreground
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(io_error)?;
    peer.runtime.attach_presentation_terminal(provider)?;
    let (mut owner, snapshot) = wire::OwnerRoute::admit(&mut peer, invitation)?;
    let mut shown = present(&mut peer, &mut foreground, output, snapshot)?;
    owner.acknowledge(&shown.show)?;
    report(output)?;
    loop {
        let mut bytes = Vec::new();
        let length = (&mut *input)
            .take(257)
            .read_until(b'\n', &mut bytes)
            .map_err(io_error)?;
        if length == 0 || bytes == b"quit\n" || bytes == b"quit\r\n" {
            if shown.execution.has_pending_play() {
                shown.execution.close_without_input().map_err(debug_error)?;
            }
            return Ok(());
        }
        if length > 256 {
            return Err("terminal command exceeds 256 bytes".into());
        }
        let command = std::str::from_utf8(&bytes)
            .map_err(|_| "terminal command is not UTF-8")?
            .trim();
        match command {
            "actions" => crate::body_terminal_owner::report_actions(output, &shown.face)?,
            "evidence" => {
                let mut evidence =
                    crate::body_terminal_owner::evidence_for_show(&shown.face, &shown.show)?;
                evidence["route_plan_id"] = serde_json::json!(shown.route_id);
                evidence["host_id"] = serde_json::json!(shown.show.show.host_id);
                evidence["boot_id"] = serde_json::json!(shown.show.show.boot_id);
                evidence["bytes_written"] = serde_json::json!(shown.effect.bytes_written);
                evidence["frame_sha256"] = serde_json::json!(shown.effect.frame_sha256);
                evidence["show_sha256"] = serde_json::json!(shown.effect.show_sha256);
                writeln!(output, "Peer terminal evidence {evidence}").map_err(io_error)?;
                output.flush().map_err(io_error)?;
            }
            "show" | "refresh" => {
                if shown.execution.has_pending_play() {
                    shown.execution.close_without_input().map_err(debug_error)?;
                }
                let snapshot = owner.refresh()?;
                shown = present(&mut peer, &mut foreground, output, snapshot)?;
                owner.acknowledge(&shown.show)?;
                report(output)?;
            }
            _ => {
                let action = command
                    .strip_prefix("action ")
                    .map(|text| {
                        text.split_once(' ')
                            .map_or((Some(text), ""), |(id, value)| (Some(id), value))
                    })
                    .or_else(|| command.strip_prefix("apply ").map(|value| (None, value)));
                if let Some((identity, value)) = action {
                    let interaction = match crate::body_terminal_owner::interaction_for_action(
                        &shown.face,
                        &shown.show,
                        identity,
                        value,
                    ) {
                        Ok(value) => value,
                        Err(error) => {
                            writeln!(output, "Action refused: {error}").map_err(io_error)?;
                            continue;
                        }
                    };
                    let returned = shown.execution.interact(interaction).map_err(debug_error)?;
                    let (response, next) = owner.action(&shown.show, returned.interaction)?;
                    writeln!(output, "Peer terminal action {response}").map_err(io_error)?;
                    output.flush().map_err(io_error)?;
                    let Some(next) = next else {
                        return Ok(());
                    };
                    shown = present(&mut peer, &mut foreground, output, next)?;
                    owner.acknowledge(&shown.show)?;
                    report(output)?;
                } else {
                    writeln!(output, "Enter actions, action <identity> [value], apply <value>, show, evidence, or quit.").map_err(io_error)?;
                }
            }
        }
    }
}

fn present(
    peer: &mut PresentationHost,
    foreground: &mut UnixStream,
    output: &mut impl Write,
    snapshot: wire::Snapshot,
) -> Result<Shown, String> {
    let host = peer.runtime.presentation_host_mut()?;
    let mut execution = host.prepare_remote_terminal_mask_execution(&snapshot.route)?;
    let (show, effect) = std::thread::scope(|scope| {
        let rendered = scope.spawn(|| {
            host.represent_attached_terminal_face_with_execution(&snapshot.face, &mut execution)
        });
        let effect = receive_terminal_frame_and_ack(foreground, output).map_err(io_error);
        let show = rendered
            .join()
            .map_err(|_| "presentation renderer stopped".to_string())?;
        Ok::<_, String>((show?, effect?))
    })?;
    Ok(Shown {
        face: snapshot.face,
        show,
        execution,
        effect,
        route_id: snapshot.route.route_plan_id,
    })
}
fn report(output: &mut impl Write) -> Result<(), String> {
    writeln!(
        output,
        "\nTerminal ready. Enter actions, action <identity> [value], show, evidence, or quit."
    )
    .map_err(io_error)?;
    output.flush().map_err(io_error)
}

fn io_error(error: std::io::Error) -> String {
    format!("presentation terminal:{error}")
}
fn debug_error(error: impl std::fmt::Debug) -> String {
    format!("presentation terminal:{error:?}")
}
