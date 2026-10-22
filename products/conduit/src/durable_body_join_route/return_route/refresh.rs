//! Refresh one admitted native Face after the Owner changes revision.
//! The old finite bearer authenticates this request; it cannot authorize an
//! action against the stale Show. A fresh route and bearer are selected only
//! for the current Face.

use super::*;
use conduit_face::OWNER_FACE_RESPONSE_SCHEMA;

pub(super) const REFRESH_SCHEMA: &str = "conduit.body/native-owner-return-refresh@1";
const REFRESH_RESPONSE_SCHEMA: &str = "conduit.body/native-owner-return-refresh-response@1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Refresh {
    schema: String,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    #[serde(default)]
    mask: Option<MaskSelection>,
    #[serde(default)]
    spoken_selection:
        Option<conduit_std_host::direct_spoken_template_mask::RemoteSpokenTemplateSelection>,
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
        || (request.mask.is_some() && request.spoken_selection.is_some())
        || (request.spoken_selection.is_some() && old.mask != MaskSelection::Terminal)
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
    let mut next = if matches!(&face, OwnerFaceSnapshotResponse::Snapshot { face, .. }
        if old.mask == MaskSelection::Terminal || crate::durable_host_control::has_native_return_route_intent(face))
    {
        let mut grant = old.replacement(next_sequence)?;
        if let Some(mask) = request.mask {
            grant.mask = mask;
            grant.spoken_selection = None;
        }
        if let Some(selection) = request.spoken_selection {
            grant.spoken_selection = Some(selection);
        }
        Some(grant)
    } else {
        None
    };
    let mut refusal = None;
    let selected = if let (Some(grant), OwnerFaceSnapshotResponse::Snapshot { route, .. }) =
        (next.as_ref(), &mut face)
    {
        crate::durable_host_control::local_face_snapshot(state_dir)
            .and_then(|(_, owner_offer)| {
                grant.seal_mask_route(listener, state_dir, &owner_offer, 0)
            })
            .map(|selected| {
                *route = Some(Box::new(selected));
            })
            .map_err(|error| {
                refusal = Some(error);
            })
            .is_ok()
    } else {
        false
    };
    if let Some(code) = refusal {
        face = OwnerFaceSnapshotResponse::Refused {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            code: code.chars().take(256).collect(),
        };
    }
    if let OwnerFaceSnapshotResponse::Snapshot {
        interactions_admitted,
        ..
    } = &mut face
    {
        *interactions_admitted = selected
            && next
                .as_ref()
                .is_some_and(|grant| grant.spoken_selection.is_none());
    }
    if !selected {
        next = None;
    }
    let response = RefreshResponse {
        schema: REFRESH_RESPONSE_SCHEMA,
        face,
        grant: next.clone(),
    };
    let hosted = old.mask == MaskSelection::Terminal;
    let profile = conduit_conduitos_mask_offer::NativeMaskStorageProfile::owner_document_profile(
        &old.receipt.host_advertisement,
    );
    let maximum = if hosted {
        super::super::response_document::MAX_HOSTED_OWNER_DOCUMENT_BYTES
    } else {
        profile.owner_document_bytes()
    };
    if serde_json::to_vec(&response)
        .map_err(|error| format!("encode native Face refresh: {error}"))?
        .len()
        > maximum
    {
        return Err("native Face refresh response pressure".into());
    }
    if hosted {
        super::super::response_document::send_hosted(line, &response)?;
    } else {
        super::super::response_document::send_for_profile(line, &response, profile)?;
    }
    Ok(match next {
        Some(grant) => show_ack::ShowGate::Refreshed(Box::new(grant)),
        None => show_ack::ShowGate::Stopped,
    })
}
