//! An authenticated foreground terminal provider for the installed owner.
//! The connection is retained by the actual StdHost, not by a second Host.

use super::{body::HostSource, constant_time_equal, DurableHostRuntime, PROTOCOL};
use conduit_core::PlanId;
use conduit_presentation::{
    FaceInteraction, LocalOwnerMaskRouteSeal, MaskShow, MaskWardrobeAction,
};
use conduit_std_host::{
    terminal_face_mask::TerminalMaskExecution, terminal_mask_execution::HostedTerminalMaskExecution,
};
use serde::{Deserialize, Serialize};
use std::{os::unix::net::UnixStream, path::Path};

#[path = "terminal_attach/client.rs"]
mod client;
#[path = "terminal_attach/wire.rs"]
mod wire;
pub(crate) use client::{
    attach_and_show, attached_wardrobe, refresh_show, AttachedTerminalSession,
};
pub(super) use wire::MAGIC;
use wire::{AttachReply, AttachRequest};

pub(super) struct AttachedTerminalRoute {
    pub seal: LocalOwnerMaskRouteSeal,
    pub show: MaskShow,
    pub execution: HostedTerminalMaskExecution,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TerminalWardrobeCommand {
    Inspect,
    Wear,
    Doff,
    Prefer,
}

/// The ordinary control listener has already consumed the discriminating byte.
/// Malformed or refused attachment requests never terminate the owner service.
pub(super) fn serve(
    state_dir: &Path,
    stream: &mut UnixStream,
    runtime: &mut DurableHostRuntime,
    token: &[u8; 32],
    first: u8,
) -> Result<(), String> {
    let mut attached_here = false;
    let mut attached_reply_sent = false;
    let mut frame_acknowledged = false;
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
        refresh_marker(state_dir, runtime)?;
        let HostSource::Body { owner, .. } = &mut runtime.host else {
            unreachable!("only an installed Body can attach a terminal");
        };
        let (face, seal) = owner.seal_attached_terminal_route()?;
        wire::write_reply(stream, &AttachReply::Attached { protocol: PROTOCOL })?;
        attached_reply_sent = true;
        let (show, execution) = owner
            .host
            .current_mut()
            .present_attached_terminal_face_with_interaction(&face)?;
        frame_acknowledged = true;
        owner.acknowledge_attached_terminal_show(&seal, &show)?;
        let advertisement = owner.host.advertisement().clone();
        runtime.terminal_route = Some(AttachedTerminalRoute {
            seal: seal.clone(),
            show: show.clone(),
            execution,
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
            // A marker update failure must stop the service; silently
            // publishing a different offer generation would be false truth.
            retire(state_dir, runtime)?;
        }
        if !attached_reply_sent || frame_acknowledged {
            // After the actual frame was acknowledged, a later owner refusal
            // is known. Before then a failed effect remains outcome-unknown.
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

impl DurableHostRuntime {
    pub(super) fn refresh_attached_terminal_show(
        &mut self,
        route_plan_id: &PlanId,
        old_show: &MaskShow,
    ) -> Result<(MaskShow, conduit_core::HostAdvertisement), String> {
        use conduit_std_host::terminal_face_mask::TerminalMaskExecution;
        let mut route = self
            .terminal_route
            .take()
            .ok_or("no current attached terminal route")?;
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("terminal Show requires a lulled Body".into());
        };
        if &route.seal.route_plan_id != route_plan_id || &route.show != old_show {
            self.terminal_route = Some(route);
            return Err("attached terminal route or Show differs".into());
        }
        owner.validate_attached_terminal_route(&route.seal, old_show)?;
        route
            .execution
            .validate_current_host(owner.host.advertisement())
            .map_err(|error| format!("attached terminal Host changed: {error:?}"))?;
        route
            .execution
            .close_without_input()
            .map_err(|error| format!("close prior terminal Mask Play: {error:?}"))?;
        let face = owner.local_face_snapshot()?;
        let show = owner
            .host
            .current_mut()
            .represent_attached_terminal_face_with_execution(&face, &mut route.execution)?;
        owner.acknowledge_attached_terminal_show(&route.seal, &show)?;
        let advertisement = owner.host.advertisement().clone();
        route.show = show.clone();
        self.terminal_route = Some(route);
        Ok((show, advertisement))
    }

    pub(super) fn attached_terminal_wardrobe(
        &mut self,
        route_plan_id: &PlanId,
        show: &MaskShow,
        basis_revision: u64,
        command: TerminalWardrobeCommand,
    ) -> Result<serde_json::Value, String> {
        let route = self
            .terminal_route
            .as_mut()
            .ok_or("no current attached terminal route")?;
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("terminal wardrobe requires a lulled Body".into());
        };
        if &route.seal.route_plan_id != route_plan_id || &route.show != show {
            return Err("attached terminal route or Show differs".into());
        }
        route
            .execution
            .validate_current_host(owner.host.advertisement())
            .map_err(|error| format!("attached terminal Host changed: {error:?}"))?;
        let mask = route.seal.planned_mask.mask.plot_identity.clone();
        let action = match command {
            TerminalWardrobeCommand::Inspect => None,
            TerminalWardrobeCommand::Wear => Some(MaskWardrobeAction::Wear(mask)),
            TerminalWardrobeCommand::Doff => Some(MaskWardrobeAction::Doff(mask)),
            TerminalWardrobeCommand::Prefer => Some(MaskWardrobeAction::Prefer(vec![mask])),
        };
        owner.attached_terminal_wardrobe_report(&route.seal, show, basis_revision, action)
    }

    /// Only the installed owner can consume the retained Mask's typed return.
    /// Taking the route first makes every attempted submission single-use;
    /// failure or a changed Face retires the provider on the next service step.
    pub(super) fn attached_terminal_interaction(
        &mut self,
        route_plan_id: &PlanId,
        show: &MaskShow,
        interaction: FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let mut route = self
            .terminal_route
            .take()
            .ok_or("no current attached terminal route")?;
        let HostSource::Body {
            owner,
            root,
            running: None,
        } = &mut self.host
        else {
            return Err("attached terminal action requires a lulled Body".into());
        };
        if &route.seal.route_plan_id != route_plan_id || &route.show != show {
            return Err("attached terminal route or Show differs".into());
        }
        owner.validate_selected_terminal_show(&route.seal, show)?;
        route
            .execution
            .validate_current_host(owner.host.advertisement())
            .map_err(|error| format!("attached terminal Host changed: {error:?}"))?;
        owner.forget_attached_terminal_show(&route.seal);
        let correlated = route
            .execution
            .interact(interaction)
            .map_err(|error| format!("attached terminal Mask return refused: {error:?}"))?;
        owner.apply_clock_interval_interaction(root, show, &correlated.interaction)
    }
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
        } if !owner.host.is_playing() => {
            owner.host.current_mut().terminal_attachment_mut().is_some()
        }
        _ => false,
    }
}

/// The service checks the selected provider and the semantic seal even while
/// the workload is lulled and there are no incoming control requests.
pub(super) fn retire_closed_attachment(
    state_dir: &Path,
    runtime: &mut DurableHostRuntime,
) -> Result<(), String> {
    if !is_attached(runtime) {
        return retire(state_dir, runtime);
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
        retire(state_dir, runtime)?;
    }
    Ok(())
}

fn retire(state_dir: &Path, runtime: &mut DurableHostRuntime) -> Result<(), String> {
    if let Some(route) = runtime.terminal_route.take() {
        if let HostSource::Body { owner, .. } = &mut runtime.host {
            owner.forget_attached_terminal_show(&route.seal);
        }
    }
    if !is_attached(runtime) {
        return Ok(());
    }
    let HostSource::Body { owner, .. } = &mut runtime.host else {
        unreachable!("only an installed Body can attach a terminal");
    };
    owner.host.current_mut().detach_terminal_mask()?;
    refresh_marker(state_dir, runtime)
}

fn refresh_marker(state_dir: &Path, runtime: &DurableHostRuntime) -> Result<(), String> {
    let advertised = runtime.host.advertisement();
    crate::durable_host::refresh_offer_generation(
        state_dir,
        &advertised.host_id,
        &advertised.boot_id,
        advertised.offer_generation,
    )
}

#[cfg(test)]
#[path = "terminal_attach/tests.rs"]
mod tests;
