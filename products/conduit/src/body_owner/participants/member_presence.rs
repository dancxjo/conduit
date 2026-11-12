//! Already admitted members use the same participant pump and typed Face frames.
//! Native Lines are offered only while their actual bounded TLS socket is held;
//! the serialized owner separately selects the route and accepts its Show.
use super::{
    presence_pump::{PresenceEvent, PresencePump},
    transport::Socket,
    In, Out, PROTOCOL,
};
use crate::durable_host::owner::MemberPresentationBasis;
use conduit_body::{HostPresenceLease, MembershipCredential};
use conduit_core::LinkBindingId;
use conduit_face::{
    OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse, OWNER_FACE_RESPONSE_SCHEMA,
};
use conduit_std_host::secure_websocket::SecureWebSocketLine;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(crate) fn serve_member_presence(
    state: &Path,
    line: SecureWebSocketLine,
    binding: LinkBindingId,
    member: &MemberPresentationBasis,
) -> Result<(), String> {
    serve_with_member_dispatch(
        line,
        binding.clone(),
        &member.credential,
        |sequence| {
            crate::durable_host_control::renew_carrier_presence(state, binding.clone(), sequence)
        },
        |socket, frame| dispatch(state, socket, member, frame),
        Some((state, member)),
    )
}

#[cfg(test)]
pub(crate) fn serve_with_carrier(
    line: SecureWebSocketLine,
    binding: LinkBindingId,
    credential: &MembershipCredential,
    renew: impl FnMut(u64) -> Result<HostPresenceLease, String>,
) -> Result<(), String> {
    serve_with_dispatch(line, binding, credential, renew, |_, _| {
        Err("no presentation selected in authentication test".into())
    })
}

pub(crate) fn serve_with_dispatch(
    line: SecureWebSocketLine,
    binding: LinkBindingId,
    credential: &MembershipCredential,
    renew: impl FnMut(u64) -> Result<HostPresenceLease, String>,
    dispatch: impl FnMut(&Socket, In) -> Result<Out, String>,
) -> Result<(), String> {
    serve_with_member_dispatch(line, binding, credential, renew, dispatch, None)
}
#[allow(clippy::too_many_arguments)]
fn serve_with_member_dispatch(
    mut line: SecureWebSocketLine,
    binding: LinkBindingId,
    credential: &MembershipCredential,
    mut renew: impl FnMut(u64) -> Result<HostPresenceLease, String>,
    mut dispatch: impl FnMut(&Socket, In) -> Result<Out, String>,
    member_action: Option<(&Path, &MemberPresentationBasis)>,
) -> Result<(), String> {
    line.complete_bounded_input();
    let mut socket = Socket::from_secure(line, binding)?;
    let result = (|| {
        let anchor = Instant::now();
        socket.replace_input_deadline(anchor + Duration::from_millis(super::LEASE_MS))?;
        let lease = renew(1)?;
        let overall = anchor + Duration::from_secs(60);
        let mut pump =
            PresencePump::start(&mut socket, credential, lease, anchor, overall, 128, true)?;
        let mut pending = super::pending_action::PendingAction::default();
        while !pump.wait_remaining().is_zero() {
            if let Some(response) = pending.take_response() {
                socket.send(&response)?;
            }
            let hint = pending.is_pending().then_some(Duration::from_millis(50));
            let deferred = pending.take_deferred();
            let Some((frame, bytes)) = pump.receive(&mut socket, hint, deferred)? else {
                continue;
            };
            if pending.is_pending()
                && !matches!(
                    &frame,
                    In::PresenceRenewal { .. } | In::PresenceLeave { .. }
                )
            {
                pending.defer((frame, bytes))?;
                continue;
            }
            match pump.handle_presence(&mut socket, frame, &mut renew)? {
                PresenceEvent::Continue => {}
                PresenceEvent::Leave => return Ok(()),
                PresenceEvent::Other(In::FaceInteractionRequest {
                    protocol: PROTOCOL,
                    request,
                    show,
                    interaction,
                }) => {
                    let (state, member) =
                        member_action.ok_or("member action lacks installed owner context")?;
                    if !matches_request(&request, member) {
                        return Err("member interaction credential mismatch".into());
                    }
                    if let Err(error) = pending.begin_member(
                        state,
                        socket.binding().clone(),
                        (request, show, interaction),
                    ) {
                        socket.send(&super::pending_action::response(Err(error)))?;
                    }
                }
                PresenceEvent::Other(frame) => {
                    let response = dispatch(&socket, frame)?;
                    socket.send(&response)?;
                }
            }
        }
        Err("member presence lifetime ended".into())
    })();
    socket.close();
    result
}

fn matches_request(request: &OwnerFaceSnapshotRequest, member: &MemberPresentationBasis) -> bool {
    let c = &member.credential;
    request.has_exact_basis()
        && request.credential_id == c.credential_id.as_str()
        && request.body_id == c.body_id
        && request.part_id == c.part_id
        && request.host_id == c.host_id
        && request.boot_id == c.boot_id
}
fn dispatch(
    state: &Path,
    socket: &Socket,
    member: &MemberPresentationBasis,
    frame: In,
) -> Result<Out, String> {
    match frame {
        In::FaceSnapshotRequest {
            protocol: PROTOCOL,
            request,
        } if matches_request(&request, member) => {
            let response = prepare_face(state, socket, member, request).unwrap_or_else(|error| {
                OwnerFaceSnapshotResponse::Refused {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    code: error.chars().take(128).collect(),
                }
            });
            let frame = Out::FaceSnapshotResponse {
                protocol: PROTOCOL,
                response,
            };
            let maximum =
                conduit_conduitos_mask_offer::NativeMaskStorageProfile::owner_document_profile(
                    &member.host_advertisement,
                )
                .owner_document_bytes();
            if serde_json::to_vec(&frame).map_err(|e| e.to_string())?.len() > maximum {
                return Ok(Out::FaceSnapshotResponse {
                    protocol: PROTOCOL,
                    response: OwnerFaceSnapshotResponse::Refused {
                        schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                        code: "native-face-document-pressure".into(),
                    },
                });
            }
            Ok(frame)
        }
        In::FaceShowAcknowledgement {
            protocol: PROTOCOL,
            request,
            show,
        } if matches_request(&request, member) => {
            let outcome = crate::durable_host_control::carrier_show(
                state,
                socket.binding().clone(),
                request,
                *show,
            );
            Ok(Out::FaceShowResponse {
                protocol: PROTOCOL,
                accepted: outcome.is_ok(),
                code: outcome
                    .err()
                    .map_or_else(String::new, |_| "native-show-refused".into()),
            })
        }
        _ => Err("member carrier refused unsupported or stale presentation frame".into()),
    }
}
fn prepare_face(
    state: &Path,
    socket: &Socket,
    member: &MemberPresentationBasis,
    mut request: OwnerFaceSnapshotRequest,
) -> Result<OwnerFaceSnapshotResponse, String> {
    request.last_seen_revision = None;
    request.last_seen_identity = None;
    for _ in 0..3 {
        let (_, owner) = crate::durable_host_control::local_face_snapshot(state)?;
        let (face_line, return_line, interaction_line) =
            socket.native_graphical_lines(&owner, member)?;
        let seal = crate::durable_host_control::carrier_mask_route(
            state,
            socket.binding().clone(),
            face_line,
            return_line,
            interaction_line,
        )?;
        let response = crate::durable_host_control::face_snapshot(state, request.clone())?;
        let OwnerFaceSnapshotResponse::Snapshot { schema, face, .. } = response else {
            return Err("full-owner-face-unavailable".into());
        };
        if schema != OWNER_FACE_RESPONSE_SCHEMA {
            return Err("owner-face-schema".into());
        }
        if seal.face_id == face.identity
            && seal.face_revision == face.revision
            && seal.face_basis == face.basis
        {
            return Ok(OwnerFaceSnapshotResponse::Snapshot {
                schema,
                face,
                interactions_admitted: seal.interaction_line.is_some(),
                route: Some(Box::new(seal)),
            });
        }
    }
    Err("owner-face-transition-pressure".into())
}
