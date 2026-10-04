//! An authenticated foreground terminal provider for the installed owner.
//! The connection is retained by the actual StdHost, not by a second Host.

use super::{body::HostSource, constant_time_equal, DurableHostRuntime, PROTOCOL};
use conduit_core::PlanId;
use conduit_presentation::{
    BodyMaskWardrobe, FaceInteraction, LocalOwnerMaskRouteSeal, MaskShow, MaskWardrobe,
    MaskWardrobeAction, MaskWardrobeControl, MaskWardrobeLifetime,
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
pub(crate) use client::{attach_and_show, attached_wardrobe, AttachedTerminalSession};
pub(super) use wire::MAGIC;
use wire::{AttachReply, AttachRequest};

pub(super) struct AttachedTerminalRoute {
    pub seal: LocalOwnerMaskRouteSeal,
    pub show: MaskShow,
    pub wardrobe: MaskWardrobeControl,
    pub execution: HostedTerminalMaskExecution,
    /// Doff retires this acknowledgement. Rewear selects a sealed route, but
    /// cannot silently make the old Show current again.
    pub acknowledged_show_selected: bool,
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
        let routes = owner.admit_attached_terminal_show(&seal, &show)?;
        let mask = seal.planned_mask.mask.plot_identity.clone();
        let wardrobe = MaskWardrobeControl::new_from_admitted_routes(
            BodyMaskWardrobe::new(
                seal.body_id.clone(),
                None,
                MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![mask.clone()], vec![mask])
                    .map_err(|error| format!("wear attached terminal Mask: {error:?}"))?,
            )
            .map_err(|error| format!("scope attached terminal Mask: {error:?}"))?,
            &routes,
            None,
        )
        .map_err(|error| format!("select attached terminal Mask: {error:?}"))?;
        let advertisement = owner.host.advertisement().clone();
        runtime.terminal_route = Some(AttachedTerminalRoute {
            seal: seal.clone(),
            show: show.clone(),
            wardrobe,
            execution,
            acknowledged_show_selected: true,
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

impl DurableHostRuntime {
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
        owner.validate_attached_terminal_route(&route.seal, show)?;
        route
            .execution
            .validate_current_host(owner.host.advertisement())
            .map_err(|error| format!("attached terminal Host changed: {error:?}"))?;
        let routes = owner.admit_attached_terminal_show(&route.seal, show)?;
        let mask = route.seal.planned_mask.mask.plot_identity.clone();
        let transition = match command {
            TerminalWardrobeCommand::Inspect => None,
            TerminalWardrobeCommand::Wear => Some(MaskWardrobeAction::Wear(mask)),
            TerminalWardrobeCommand::Doff => Some(MaskWardrobeAction::Doff(mask)),
            TerminalWardrobeCommand::Prefer => Some(MaskWardrobeAction::Prefer(vec![mask])),
        }
        .map(|action| {
            route
                .wardrobe
                .apply(basis_revision, action, &routes)
                .map_err(|error| format!("attached terminal wardrobe refused: {error:?}"))
        })
        .transpose()?;
        if matches!(command, TerminalWardrobeCommand::Doff) {
            route.acknowledged_show_selected = false;
        }
        let reconciliation = route
            .wardrobe
            .scoped_wardrobe
            .wardrobe
            .reconcile(
                &route.wardrobe.active_plan_id,
                routes.routes(),
                route.wardrobe.selected.as_ref(),
            )
            .map_err(|error| format!("inspect attached terminal wardrobe: {error:?}"))?;
        let selected = route.wardrobe.selected.as_ref();
        Ok(serde_json::json!({
            "schema": "conduit.body/attached-terminal-wardrobe@1",
            "scope": "foreground-terminal-attachment",
            "durable": false,
            "body_id": route.seal.body_id,
            "face_id": route.seal.face_id,
            "face_revision": route.seal.face_revision,
            "host_id": route.seal.owner_offer.host_id,
            "boot_id": route.seal.owner_offer.boot_id,
            "offer_generation": route.seal.owner_offer.offer_generation.0,
            "route_plan_id": route.seal.route_plan_id,
            "wardrobe": route.wardrobe.scoped_wardrobe.wardrobe,
            "admitted_routes": routes.routes(),
            "selected": selected,
            "show_id": if route.acknowledged_show_selected && selected.is_some() {
                Some(show.show_id.as_str())
            } else {
                None
            },
            "fresh_show_required": selected.is_some() && !route.acknowledged_show_selected,
            "reconciliation": reconciliation,
            "transition": transition,
            "unadmitted_masks": "not selectable through this attached terminal route",
        }))
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
        if !route.acknowledged_show_selected
            || route.wardrobe.selected.as_ref().is_none_or(|selected| {
                selected.plan_id != *route_plan_id
                    || selected.mask_plot != route.seal.planned_mask.mask.plot_identity
            })
        {
            return Err("attached terminal Mask is no longer selected".into());
        }
        owner.validate_attached_terminal_route(&route.seal, show)?;
        route
            .execution
            .validate_current_host(owner.host.advertisement())
            .map_err(|error| format!("attached terminal Host changed: {error:?}"))?;
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
        retire(state_dir, runtime)?;
    }
    Ok(())
}

fn retire(state_dir: &Path, runtime: &mut DurableHostRuntime) -> Result<(), String> {
    runtime.terminal_route = None;
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
