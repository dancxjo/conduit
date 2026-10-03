//! An authenticated foreground terminal provider for the installed owner.
//! The connection is retained by the actual StdHost, not by a second Host.

use super::{body::HostSource, constant_time_equal, DurableHostRuntime, PROTOCOL};
use conduit_presentation::{LocalOwnerMaskRouteSeal, MaskShow};
use std::os::unix::net::UnixStream;

#[path = "terminal_attach/client.rs"]
mod client;
#[path = "terminal_attach/wire.rs"]
mod wire;
pub(crate) use client::attach_and_show;
pub(super) use wire::MAGIC;
use wire::{AttachReply, AttachRequest};

pub(super) struct AttachedTerminalRoute {
    pub seal: LocalOwnerMaskRouteSeal,
    pub show: MaskShow,
}

/// The ordinary control listener has already consumed the discriminating byte.
/// Malformed or refused attachment requests never terminate the owner service.
pub(super) fn serve(
    stream: &mut UnixStream,
    runtime: &mut DurableHostRuntime,
    token: &[u8; 32],
    first: u8,
) -> Result<(), String> {
    let mut attached_here = false;
    let mut attached_reply_sent = false;
    let result = (|| {
        let mut request = wire::read_request(stream, first)?;
        let authenticated = constant_time_equal(&request.token, token);
        request.token.fill(0);
        if !authenticated {
            return Err("unauthorized".into());
        }
        if request.protocol != PROTOCOL {
            return Err("terminal-attachment-protocol".into());
        }
        attach(stream, runtime, &request)?;
        attached_here = true;
        let HostSource::Body { owner, .. } = &mut runtime.host else {
            unreachable!("only an installed Body can attach a terminal");
        };
        let (face, seal) = owner.seal_attached_terminal_route()?;
        wire::write_reply(stream, &AttachReply::Attached { protocol: PROTOCOL })?;
        attached_reply_sent = true;
        let show = owner
            .host
            .current_mut()
            .present_attached_terminal_face(&face)?;
        owner.validate_attached_terminal_route(&seal, &show)?;
        let advertisement = owner.host.advertisement().clone();
        runtime.terminal_route = Some(AttachedTerminalRoute {
            seal: seal.clone(),
            show: show.clone(),
        });
        wire::write_reply(
            stream,
            &AttachReply::Show {
                protocol: PROTOCOL,
                route_plan_id: seal.route_plan_id,
                show: Box::new(show),
                advertisement,
            },
        )?;
        Ok::<(), String>(())
    })();
    if let Err(code) = result {
        if attached_here {
            let _ = retire(runtime);
        }
        if !attached_reply_sent {
            let _ = wire::write_reply(
                stream,
                &AttachReply::Refused {
                    protocol: PROTOCOL,
                    code,
                },
            );
        }
    }
    Ok(())
}

fn attach(
    stream: &UnixStream,
    runtime: &mut DurableHostRuntime,
    request: &AttachRequest,
) -> Result<(), String> {
    let HostSource::Body { owner, running, .. } = &mut runtime.host else {
        return Err("installed-host-has-no-body".into());
    };
    if running.is_some() || owner.host.is_playing() {
        return Err("terminal-route-requires-lulled-body".into());
    }
    if runtime.terminal_route.is_some()
        || owner.host.current_mut().terminal_attachment_mut().is_some()
    {
        return Err("terminal-provider-already-attached".into());
    }
    let face = owner.local_face_snapshot()?;
    let advertised = owner.host.advertisement();
    if face.basis.body_id.as_ref() != Some(&request.body_id)
        || face.identity != request.face_id
        || face.revision != request.face_revision
        || advertised.host_id != request.host_id
        || advertised.boot_id != request.boot_id
        || advertised.offer_generation != request.offer_generation
    {
        return Err("stale-terminal-attachment-basis".into());
    }
    let provider = stream
        .try_clone()
        .map_err(|error| format!("clone selected terminal connection: {error}"))?;
    owner.host.current_mut().attach_terminal_mask(provider)
}

pub(super) fn is_attached(runtime: &mut DurableHostRuntime) -> bool {
    match &mut runtime.host {
        HostSource::Body {
            owner,
            running: None,
            ..
        } => owner.host.current_mut().terminal_attachment_mut().is_some(),
        _ => false,
    }
}

/// The service checks the selected provider and the semantic seal even while
/// the workload is lulled and there are no incoming control requests.
pub(super) fn retire_closed_attachment(runtime: &mut DurableHostRuntime) -> Result<(), String> {
    if !is_attached(runtime) {
        runtime.terminal_route = None;
        return Ok(());
    }
    let HostSource::Body { owner, .. } = &runtime.host else {
        unreachable!("only an installed Body can attach a terminal");
    };
    let live = owner
        .host
        .current()
        .terminal_attachment_is_live()
        .unwrap_or(false);
    let valid = runtime.terminal_route.as_ref().is_some_and(|route| {
        owner
            .validate_attached_terminal_route(&route.seal, &route.show)
            .is_ok()
    });
    if !live || !valid {
        retire(runtime)?;
    }
    Ok(())
}

fn retire(runtime: &mut DurableHostRuntime) -> Result<(), String> {
    runtime.terminal_route = None;
    if !is_attached(runtime) {
        return Ok(());
    }
    let HostSource::Body { owner, .. } = &mut runtime.host else {
        unreachable!("only an installed Body can attach a terminal");
    };
    owner.host.current_mut().detach_terminal_mask()
}

#[cfg(test)]
#[path = "terminal_attach/tests.rs"]
mod tests;
