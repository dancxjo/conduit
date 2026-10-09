//! Refresh one admitted native Face after the Owner changes revision.
//! The old finite bearer authenticates this request; it cannot authorize an
//! action against the stale Show. A fresh route and bearer are selected only
//! for the current Face.

use super::*;
use conduit_presentation::OWNER_FACE_RESPONSE_SCHEMA;

pub(super) const REFRESH_SCHEMA: &str = "conduit.body/native-owner-return-refresh@1";
const REFRESH_RESPONSE_SCHEMA: &str = "conduit.body/native-owner-return-refresh-response@1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Refresh {
    schema: String,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
}

#[derive(Serialize)]
struct RefreshResponse {
    schema: &'static str,
    face: OwnerFaceSnapshotResponse,
    grant: Option<Grant>,
}

pub(super) fn serve(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    listener: &SecureWebSocketListener,
    state_dir: &Path,
    old: &Grant,
    next_sequence: u8,
    document: serde_json::Value,
) -> Result<show_ack::ShowGate, String> {
    let mut request: Refresh = serde_json::from_value(document)
        .map_err(|error| format!("decode native Face refresh: {error}"))?;
    if request.schema != REFRESH_SCHEMA
        || request.sequence != 0
        || current_time_millis()? >= old.expires_at_millis
        || !old.matches_request(request.token, &request.request)
    {
        return Err("native Face refresh grant or basis invalid".into());
    }
    line.complete_bounded_input();
    // Refresh always returns a full exact Face. A delta/Unchanged response
    // would leave the guest holding a Show from an earlier revision.
    request.request.last_seen_revision = None;
    request.request.last_seen_identity = None;
    let mut face = crate::durable_host_control::face_snapshot(state_dir, request.request)
        .unwrap_or_else(|_| OwnerFaceSnapshotResponse::Refused {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            code: "face-unavailable".into(),
        });
    let mut next = if matches!(&face, OwnerFaceSnapshotResponse::Snapshot { presentation, .. }
        if crate::durable_host_control::has_native_return_route_intent(presentation))
    {
        let mut grant = Grant::issue(&old.receipt)?;
        // Refresh changes the Face and bearer, not the finite action budget.
        grant.maximum_actions = old
            .maximum_actions
            .saturating_sub(next_sequence.saturating_sub(1));
        Some(grant)
    } else {
        None
    };
    let selected = if let (Some(grant), OwnerFaceSnapshotResponse::Snapshot { route, .. }) =
        (next.as_ref(), &mut face)
    {
        crate::durable_host_control::local_face_snapshot(state_dir)
            .and_then(|(_, owner_offer)| {
                let binding = super::super::native_lines::binding_reference(
                    listener,
                    &grant.binding_reference(0),
                );
                let (face_line, return_line) =
                    super::super::native_lines::offer_pair(&owner_offer, &old.receipt, &binding);
                crate::durable_host_control::select_native_mask_route(
                    state_dir,
                    old.receipt.clone(),
                    face_line,
                    return_line,
                    grant.expires_at_millis(),
                )
            })
            .map(|selected| {
                *route = Some(Box::new(selected));
            })
            .is_ok()
    } else {
        false
    };
    if let OwnerFaceSnapshotResponse::Snapshot {
        interactions_admitted,
        ..
    } = &mut face
    {
        *interactions_admitted = selected;
    }
    if !selected {
        next = None;
    }
    let response = RefreshResponse {
        schema: REFRESH_RESPONSE_SCHEMA,
        face,
        grant: next.clone(),
    };
    if serde_json::to_vec(&response)
        .map_err(|error| format!("encode native Face refresh: {error}"))?
        .len()
        > super::super::response_document::MAX_OWNER_DOCUMENT_BYTES
    {
        return Err("native Face refresh response pressure".into());
    }
    super::super::response_document::send(line, &response)?;
    Ok(match next {
        Some(grant) => show_ack::ShowGate::Refreshed(Box::new(grant)),
        None => show_ack::ShowGate::Stopped,
    })
}
