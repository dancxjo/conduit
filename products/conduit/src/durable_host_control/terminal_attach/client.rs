//! The foreground terminal supplies actual output bytes to the installed
//! owner's Host; it cannot mint the owner's Show or replace its Face.

use super::TerminalWardrobeCommand;
use super::{wire, AttachReply, AttachRequest};
use crate::durable_host_control::{body, CONTROL_OUTCOME_UNKNOWN, PROTOCOL};
use conduit_core::{HostAdvertisement, PlanId};
use conduit_presentation::{ManifestationLifecycle, MaskShow, Presentation};
use conduit_std_host::hosted_terminal_mask_host::{
    receive_terminal_frame_and_ack, TerminalFrameReceipt,
};
use sha2::{Digest, Sha256};
use socket2::{Domain, SockAddr, Socket, Type};
use std::{
    io::Write,
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::Path,
    time::Duration,
};

pub(crate) struct AttachedTerminalSession {
    // Keeping this socket open is the selected terminal provider's lifetime.
    _connection: UnixStream,
    pub show: MaskShow,
    pub face: Presentation,
    pub route_plan_id: PlanId,
    pub advertisement: HostAdvertisement,
    pub effect: TerminalFrameReceipt,
}

pub(crate) fn refresh_show(
    state_dir: &Path,
    attached: &mut AttachedTerminalSession,
    output: &mut impl Write,
) -> Result<(), String> {
    let route_plan_id = attached.route_plan_id.clone();
    let old_show = attached.show.clone();
    let state = state_dir.to_path_buf();
    std::thread::scope(|scope| {
        let control = scope.spawn(move || {
            body::call(
                &state,
                crate::durable_host_control::Request::BodyAttachedTerminalRefreshShow {
                    protocol: PROTOCOL,
                    token: body::token(&state)?,
                    route_plan_id,
                    show: Box::new(old_show),
                },
            )
        });
        let effect = receive_terminal_frame_and_ack(&mut attached._connection, output)
            .map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_string())?;
        let response = control
            .join()
            .map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_string())??;
        let (show, advertisement) = match response {
            crate::durable_host_control::Response::BodyAttachedTerminalRefreshedShow {
                protocol: PROTOCOL,
                show,
                advertisement,
            } => (*show, advertisement),
            crate::durable_host_control::Response::Refused { code, .. } => return Err(code),
            _ => return Err(CONTROL_OUTCOME_UNKNOWN.into()),
        };
        let (face, current) = body::local_face_snapshot(state_dir)?;
        if show.validate(&face).is_err()
            || show.show.lifecycle != ManifestationLifecycle::Available
            || show.show.failure.is_some()
            || advertisement != current
            || advertisement != attached.advertisement
            || effect.show_sha256
                != <[u8; 32]>::from(Sha256::digest(show.show_id.as_str().as_bytes()))
        {
            return Err(CONTROL_OUTCOME_UNKNOWN.into());
        }
        attached.show = show;
        attached.face = face;
        attached.effect = effect;
        Ok(())
    })
}

pub(crate) fn attached_wardrobe(
    state_dir: &Path,
    attached: &AttachedTerminalSession,
    basis_revision: u64,
    command: TerminalWardrobeCommand,
) -> Result<serde_json::Value, String> {
    match body::call(
        state_dir,
        crate::durable_host_control::Request::BodyAttachedTerminalWardrobe {
            protocol: PROTOCOL,
            token: body::token(state_dir)?,
            route_plan_id: attached.route_plan_id.clone(),
            show: Box::new(attached.show.clone()),
            basis_revision,
            command,
        },
    )? {
        crate::durable_host_control::Response::BodyAttachedTerminalWardrobe {
            protocol: PROTOCOL,
            report,
        } => Ok(*report),
        crate::durable_host_control::Response::Refused { code, .. } => Err(code),
        _ => Err(CONTROL_OUTCOME_UNKNOWN.into()),
    }
}

pub(crate) fn attach_and_show(
    state_dir: &Path,
    output: &mut impl Write,
) -> Result<AttachedTerminalSession, String> {
    let (face, before) = body::local_face_snapshot(state_dir)?;
    let body_id = face
        .basis
        .body_id
        .clone()
        .ok_or("owner terminal Face has no Body basis")?;
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)
        .map_err(|error| format!("open terminal control socket: {error}"))?;
    let address = SockAddr::unix(state_dir.join("control.sock"))
        .map_err(|error| format!("address terminal control socket: {error}"))?;
    socket
        .connect_timeout(&address, Duration::from_secs(2))
        .map_err(|error| format!("connect terminal control socket: {error}"))?;
    let mut stream = UnixStream::from(OwnedFd::from(socket));
    let mut request = AttachRequest {
        protocol: PROTOCOL,
        token: body::token(state_dir)?,
        body_id,
        host_id: before.host_id.clone(),
        boot_id: before.boot_id.clone(),
        offer_generation: before.offer_generation,
        face_id: face.identity.clone(),
        face_revision: face.revision,
    };
    let submitted = wire::write_request(&mut stream, &request);
    request.token.fill(0);
    submitted.map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_string())?;
    match wire::read_reply(&mut stream).map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_string())? {
        AttachReply::Attached { protocol: PROTOCOL } => {}
        AttachReply::Refused {
            protocol: PROTOCOL,
            code,
        } => return Err(format!("owner terminal attachment refused: {code}")),
        _ => return Err(CONTROL_OUTCOME_UNKNOWN.into()),
    }
    let effect = receive_terminal_frame_and_ack(&mut stream, output)
        .map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_string())?;
    let (route_plan_id, show, advertisement) =
        match wire::read_reply(&mut stream).map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_string())? {
            AttachReply::Show {
                protocol: PROTOCOL,
                route_plan_id,
                show,
                advertisement,
            } => (route_plan_id, show, advertisement),
            AttachReply::Refused {
                protocol: PROTOCOL,
                code,
            } => return Err(format!("owner terminal Show refused: {code}")),
            _ => return Err(CONTROL_OUTCOME_UNKNOWN.into()),
        };
    // Attachment changes the owner's offer generation. The Show was sealed
    // against the Face after that change, not the Face used to request the
    // attachment. Read the current owner snapshot before validating it.
    let (face, current) =
        body::local_face_snapshot(state_dir).map_err(|_| CONTROL_OUTCOME_UNKNOWN.to_string())?;
    if show.validate(&face).is_err()
        || show.show.lifecycle != ManifestationLifecycle::Available
        || show.show.failure.is_some()
        || show.show.host_id != advertisement.host_id
        || show.show.boot_id != advertisement.boot_id
        || show.show.offer_generation != advertisement.offer_generation
        || advertisement != current
        || advertisement.host_id != before.host_id
        || advertisement.boot_id != before.boot_id
        || effect.show_sha256 != <[u8; 32]>::from(Sha256::digest(show.show_id.as_str().as_bytes()))
    {
        return Err(CONTROL_OUTCOME_UNKNOWN.into());
    }
    Ok(AttachedTerminalSession {
        _connection: stream,
        show: *show,
        face,
        route_plan_id,
        advertisement,
        effect,
    })
}
