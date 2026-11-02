//! Apply one typed native interaction and seal the refreshed Face route.

use super::*;

#[cfg(test)]
pub(super) fn fit_response(response: Response) -> Result<Response, String> {
    fit_response_with_maximum(
        response,
        super::super::response_document::MAX_OWNER_DOCUMENT_BYTES,
    )
}

pub(super) fn fit_response_with_maximum(
    mut response: Response,
    maximum: usize,
) -> Result<Response, String> {
    let bytes = serde_json::to_vec(&response)
        .map_err(|error| format!("encode native return response: {error}"))?;
    if bytes.len() > maximum {
        response.face = None;
        if response.accepted {
            response.code = "accepted-face-pressure";
        }
    }
    Ok(response)
}

pub(super) fn apply(
    listener: &SecureWebSocketListener,
    state_dir: &Path,
    grant: &Grant,
    action: Action,
    more_actions: bool,
) -> Response {
    let request = action.request;
    let result = crate::durable_host_control::submit_native_guest_face_interaction_until(
        state_dir,
        request.clone(),
        action.show,
        action.interaction,
        grant.expires_at_millis,
    );
    let (accepted, code) = match &result {
        Ok(_) => (true, "accepted"),
        Err(error) if error == "control-outcome-unknown" => (false, "control-outcome-unknown"),
        Err(error) if error.contains("control-grant-expired") => (false, "return-expired"),
        Err(error)
            if error.contains("StaleFace")
                || error.contains("StaleShow")
                || error.contains("stale owner Mask Show") =>
        {
            (false, "stale-face-or-show")
        }
        Err(error)
            if error == "body-play-active"
                || error.contains("retired Play")
                || error.contains("clock-play-must-lull") =>
        {
            (false, "clock-play-must-lull")
        }
        Err(error) if error.contains("UnavailableAction") => (false, "action-unavailable"),
        Err(error) if error.contains("RefusedAction") => (false, "action-refused"),
        Err(error) if error.contains("credential-not-admitted") => {
            (false, "credential-not-admitted")
        }
        Err(error) if error.contains("current-part-unavailable") => {
            (false, "current-part-unavailable")
        }
        Err(_) => (false, "interaction-refused"),
    };
    let available = more_actions
        && code != "control-outcome-unknown"
        && current_time_millis().is_ok_and(|now| now < grant.expires_at_millis);
    let face = if available {
        Some(
            grant
                .prepare_face_route(listener, state_dir, request, action.sequence)
                .unwrap_or_else(|error| OwnerFaceSnapshotResponse::Refused {
                    schema: conduit_face::OWNER_FACE_RESPONSE_SCHEMA.into(),
                    code: error.chars().take(256).collect(),
                }),
        )
    } else {
        None
    };
    Response {
        schema: RESPONSE_SCHEMA,
        accepted,
        code,
        face,
    }
}
