//! Apply one typed native interaction and seal the refreshed Face route.

use super::*;

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
    let face = crate::durable_host_control::face_snapshot(state_dir, request)
        .ok()
        .map(|mut response| {
            if let OwnerFaceSnapshotResponse::Snapshot {
                interactions_admitted,
                route,
                ..
            } = &mut response
            {
                let available = more_actions
                    && code != "control-outcome-unknown"
                    && current_time_millis().is_ok_and(|now| now < grant.expires_at_millis);
                if available {
                    let selected = crate::durable_host_control::local_face_snapshot(state_dir)
                        .and_then(|(_, owner_offer)| {
                            let binding = super::super::native_lines::binding_reference(
                                listener,
                                &grant.binding_reference(action.sequence),
                            );
                            let (face_line, return_line) = super::super::native_lines::offer_pair(
                                &owner_offer,
                                &grant.receipt,
                                &binding,
                            );
                            crate::durable_host_control::select_native_mask_route(
                                state_dir,
                                grant.receipt.clone(),
                                face_line,
                                return_line,
                                grant.expires_at_millis,
                            )
                        });
                    if let Ok(selected) = selected {
                        *route = Some(Box::new(selected));
                        *interactions_admitted = true;
                    }
                }
            }
            response
        });
    Response {
        schema: RESPONSE_SCHEMA,
        accepted,
        code,
        face,
    }
}
