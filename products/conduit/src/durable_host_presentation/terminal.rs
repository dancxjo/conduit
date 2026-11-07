//! An explicit installed peer supplies real terminal effects for the remote Body.
use super::{wire, PresentationHost};
use crate::durable_host::routed_join::MaskSelection;
#[path = "speech.rs"]
mod speech;
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
    run_entrance(state, Entrance::Join(invitation), input, output)
}

pub(crate) fn run_member(
    state: &Path,
    authorize_return: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    if !authorize_return {
        return Err("member presentation requires --authorize-return".into());
    }
    let encounter = wire::load_member_encounter(state)?;
    run_entrance(state, Entrance::Return(encounter), input, output)
}

enum Entrance {
    Join(conduit_body::PortableInvitation),
    Return(crate::durable_host::routed_join::MemberEncounter),
}

fn run_entrance(
    state: &Path,
    entrance: Entrance,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), String> {
    let mut peer = PresentationHost::prepare(state, matches!(&entrance, Entrance::Return(_)))?;
    let (provider, mut foreground) = UnixStream::pair().map_err(io_error)?;
    foreground
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(io_error)?;
    foreground
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(io_error)?;
    peer.runtime.attach_presentation_terminal(provider)?;
    let (mut owner, snapshot) = match entrance {
        Entrance::Join(invitation) => {
            wire::OwnerRoute::admit(&mut peer, invitation, MaskSelection::Terminal)?
        }
        Entrance::Return(encounter) => wire::OwnerRoute::return_initialized(
            &mut peer.runtime,
            &mut peer.status,
            &peer.state,
            encounter,
            MaskSelection::Terminal,
        )?,
    };
    let mut shown = present_current(&mut peer, &mut foreground, output, &mut owner, snapshot)?;
    report(output)?;
    let mut last_spoken = None;
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
            if let Err(error) = owner.leave() {
                writeln!(output, "Member departure unacknowledged: {error}").map_err(io_error)?;
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
                if let Some(summary) = &last_spoken {
                    writeln!(
                        output,
                        "Peer spoken evidence {}",
                        serde_json::to_string(summary).map_err(|error| error.to_string())?
                    )
                    .map_err(io_error)?;
                }
                output.flush().map_err(io_error)?;
            }
            "read settings" => {
                if shown.execution.has_pending_play() {
                    shown.execution.close_without_input().map_err(debug_error)?;
                }
                let summary = speech::read(&mut peer, &mut owner, &shown.face)?;
                let snapshot = owner.refresh()?;
                if snapshot.face != shown.face {
                    return Err("thermostat Face changed during the spoken reading".into());
                }
                last_spoken = Some(summary);
                writeln!(output, "Settings audio artifacts are ready.").map_err(io_error)?;
                shown = present_current(&mut peer, &mut foreground, output, &mut owner, snapshot)?;
                report(output)?;
            }
            "show" | "refresh" => {
                if shown.execution.has_pending_play() {
                    shown.execution.close_without_input().map_err(debug_error)?;
                }
                let snapshot = owner.refresh()?;
                shown = present_current(&mut peer, &mut foreground, output, &mut owner, snapshot)?;
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
                    shown = present_current(&mut peer, &mut foreground, output, &mut owner, next)?;
                    report(output)?;
                } else {
                    writeln!(output, "Enter actions, action <identity> [value], apply <value>, show, read settings, evidence, or quit.").map_err(io_error)?;
                }
            }
        }
    }
}

// Only presentation is repeated. The caller's semantic action is never replayed.
fn present_current(
    peer: &mut PresentationHost,
    foreground: &mut UnixStream,
    output: &mut impl Write,
    owner: &mut wire::OwnerRoute,
    mut snapshot: wire::Snapshot,
) -> Result<Shown, String> {
    for attempt in 0..2 {
        let mut shown = present(peer, foreground, output, snapshot)?;
        match owner.acknowledge_status(&shown.show)? {
            wire::ShowAcceptance::Accepted => return Ok(shown),
            wire::ShowAcceptance::Stale => {
                if shown.execution.has_pending_play() {
                    shown.execution.close_without_input().map_err(debug_error)?;
                }
                if attempt == 1 {
                    return Err(
                        "owner Face changed again during bounded terminal presentation".into(),
                    );
                }
                snapshot = owner.refresh()?;
            }
        }
    }
    unreachable!("finite presentation loop returns or refuses")
}

fn present(
    peer: &mut PresentationHost,
    foreground: &mut UnixStream,
    output: &mut impl Write,
    snapshot: wire::Snapshot,
) -> Result<Shown, String> {
    if !snapshot.interactions_admitted {
        return Err("terminal snapshot has no interaction authority".into());
    }
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
        "\nTerminal ready. Enter actions, action <identity> [value], show, read settings, evidence, or quit."
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

#[cfg(test)]
mod member_entrance_tests {
    use super::*;

    #[test]
    fn member_terminal_requires_authorization_before_opening_retained_state() {
        let mut output = Vec::new();
        let error = run_member(
            Path::new("/nonexistent/unowned-conduit-state"),
            false,
            &mut &b"quit\n"[..],
            &mut output,
        )
        .unwrap_err();
        assert_eq!(error, "member presentation requires --authorize-return");
        assert!(output.is_empty());
    }

    #[test]
    fn presentation_entrance_refuses_a_body_owner() {
        let (_owner, state) = crate::durable_host::owner::member_return_owner_fixture();
        for returning in [false, true] {
            assert!(PresentationHost::prepare(&state, returning).is_err());
        }
    }
}
