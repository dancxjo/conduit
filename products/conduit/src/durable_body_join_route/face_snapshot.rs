//! One owner Face on the admitted native carrier, with an optional sealed return.

use super::*;

pub(super) fn serve_optional_face_snapshot(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    listener: &SecureWebSocketListener,
    state_dir: &Path,
    member: &crate::durable_host::owner::MemberPresentationBasis,
) -> Result<Option<return_route::Grant>, String> {
    use std::io::ErrorKind;
    line.set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|error| format!("set owner Face deadline: {error:?}"))?;
    let mut bytes = vec![0; MAX_OWNER_FACE_RESPONSE_BYTES];
    let length = match line.receive_binary(&mut bytes) {
        Ok(length) => length,
        Err(SecureWebSocketError::Disconnected)
        | Err(SecureWebSocketError::Transport(
            ErrorKind::TimedOut
            | ErrorKind::WouldBlock
            | ErrorKind::UnexpectedEof
            | ErrorKind::ConnectionReset,
        )) => {
            return Ok(None);
        }
        Err(error) => return Err(format!("receive owner Face request: {error:?}")),
    };
    let (mut request, return_requested, mask) =
        return_route::decode_face_request(&bytes[..length])?;
    let credential = &member.credential;
    if !request.has_exact_basis()
        || request.credential_id != credential.credential_id.as_str()
        || request.body_id != credential.body_id
        || request.part_id != credential.part_id
        || request.host_id != credential.host_id
        || request.boot_id != credential.boot_id
    {
        send(
            line,
            &OwnerFaceSnapshotResponse::Refused {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                code: "face-credential-mismatch".into(),
            },
        )?;
        return Err("owner Face request differs from the admitted guest".into());
    }
    request.last_seen_revision = None;
    request.last_seen_identity = None;
    let mut response = crate::durable_host_control::face_snapshot(state_dir, request.clone())
        .unwrap_or_else(|error| OwnerFaceSnapshotResponse::Refused {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            code: if error == "face-frame-pressure" {
                "face-frame-pressure"
            } else {
                "face-unavailable"
            }
            .into(),
        });
    let mut grant = if return_requested
        && matches!(&response, OwnerFaceSnapshotResponse::Snapshot { face, .. }
            if mask == return_route::MaskSelection::Terminal || crate::durable_host_control::has_native_return_route_intent(face))
    {
        Some(return_route::Grant::issue(member, mask)?)
    } else {
        None
    };
    if let Some(selected) = &grant {
        response = match selected.prepare_face_route(listener, state_dir, request, 0) {
            Ok(response) => response,
            Err(error) => {
                grant = None;
                OwnerFaceSnapshotResponse::Refused {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    code: error.chars().take(256).collect(),
                }
            }
        };
    } else if let OwnerFaceSnapshotResponse::Snapshot {
        interactions_admitted,
        route,
        ..
    } = &mut response
    {
        *interactions_admitted = false;
        *route = None;
    }
    response_document::send_for_profile(
        line,
        &response,
        conduit_conduitos_mask_offer::NativeMaskStorageProfile::owner_document_profile(
            &member.host_advertisement,
        ),
    )?;
    if let Some(grant) = &grant {
        send(line, grant)?;
    }
    Ok(grant)
}

/// Inspection of the exact selected input Back; a Show ACK grant is independent
/// of permission to return a human interaction occurrence.
pub(super) fn route_allows_interaction(route: &conduit_face::RemoteOwnerMaskRouteSeal) -> bool {
    route
        .planned_mask
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|p| p.gear_id == route.planned_mask.mask.interaction_output.gear_id)
        .is_some_and(|input| {
            !matches!(
                input.execution_profile_id.as_str(),
                conduit_conduitos_mask_offer::READ_ONLY_INTERACTION_PROFILE
                    | conduit_conduitos_mask_offer::READ_ONLY_COLLECTION_INTERACTION_PROFILE
            )
        })
}
