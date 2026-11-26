//! Current admitted browser Face exchange; carrier checks precede owner authority.
use super::*;
use conduit_face::{MaskShow, OwnerFaceSnapshotRequest};

pub(super) fn snapshot(
    snapshot: &BrowserAdmittedSnapshot,
    socket: &mut transport::Socket,
    binding: &LinkBindingId,
    state_dir: Option<&Path>,
    window_id: Option<&str>,
    request: OwnerFaceSnapshotRequest,
) -> Result<(), String> {
    // Full eligibility is selected by the authoritative owner before its raw
    // Face response is sized. Existing peers keep the original request order.
    let prefetched = match prepare_full_route(snapshot, socket, binding, state_dir, window_id) {
        Ok(route) => route,
        Err(_) => {
            socket.send(&Out::FaceSnapshotResponse {
                protocol: PROTOCOL,
                response: OwnerFaceSnapshotResponse::Refused {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    code: "owner-mask-route-unavailable".into(),
                },
            })?;
            return Ok(());
        }
    };
    // The carrier and lease prove reachability. The serialized
    // owner checks this credential and its current Part again
    // against authoritative admission before projecting a Face.
    let mut fresh_request = request;
    fresh_request.last_seen_revision = None;
    fresh_request.last_seen_identity = None;
    let mut response = state_dir
        .ok_or_else(|| "owner Face requires the installed service actor".to_string())
        .and_then(|dir| crate::durable_host_control::face_snapshot(dir, fresh_request))
        .unwrap_or_else(|error| OwnerFaceSnapshotResponse::Refused {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            code: match error.as_str() {
                "face-frame-pressure" => "face-frame-pressure",
                "owner-face-credential-not-admitted" => "face-credential-not-admitted",
                "owner-face-current-part-unavailable" => "face-current-part-unavailable",
                _ => "face-unavailable",
            }
            .into(),
        });
    if let OwnerFaceSnapshotResponse::Snapshot {
        face,
        interactions_admitted,
        route,
        ..
    } = &mut response
    {
        let selected = match prefetched {
            Some(route) => Ok(route),
            None => select_route(snapshot, socket, binding, state_dir, window_id),
        };
        match selected {
            Ok(selected)
                if selected.face_id == face.identity && selected.face_revision == face.revision =>
            {
                *interactions_admitted |= crate::durable_host_control::has_remote_mask_action(face);
                *route = Some(Box::new(selected));
            }
            _ => {
                response = OwnerFaceSnapshotResponse::Refused {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    code: "owner-mask-route-unavailable".into(),
                };
            }
        }
    }
    send_response(
        socket,
        &Out::FaceSnapshotResponse {
            protocol: PROTOCOL,
            response,
        },
    )?;

    Ok(())
}

pub(super) fn acknowledge_show(
    socket: &mut transport::Socket,
    binding: &LinkBindingId,
    state_dir: Option<&Path>,
    window_id: Option<&str>,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
) -> Result<(), String> {
    #[cfg(not(unix))]
    let _ = (binding, state_dir, window_id, request, show);
    #[cfg(unix)]
    let result = state_dir
        .zip(window_id)
        .ok_or_else(|| "owner Show requires the installed service actor".to_string())
        .and_then(|(dir, window)| {
            crate::durable_host_control::browser::acknowledge_show(
                dir,
                window,
                binding.clone(),
                request,
                show,
            )
        });
    #[cfg(not(unix))]
    let result = Err::<(), String>("owner Show requires an installed Unix service actor".into());
    socket.send(&Out::FaceShowResponse {
        protocol: PROTOCOL,
        accepted: result.is_ok(),
        code: result.err().map_or_else(String::new, |error| {
            if error.contains("Stale") || error.contains("stale") {
                "stale-face-or-show".into()
            } else {
                "mask-show-unavailable".into()
            }
        }),
    })?;
    Ok(())
}

pub(super) fn selection(
    socket: &mut transport::Socket,
    state_dir: Option<&Path>,
    window_id: Option<&str>,
    binding: &LinkBindingId,
    frame: In,
) -> Result<(), String> {
    match frame {
        In::FaceComponentMaskRequest {
            protocol: PROTOCOL,
            request_id,
            request,
            show,
            look,
            recipe,
        } => {
            #[cfg(unix)]
            let result = state_dir
                .zip(window_id)
                .ok_or_else(|| {
                    "component selection requires the installed service actor".to_string()
                })
                .and_then(|(dir, window)| {
                    crate::durable_host_control::browser::component_mask(
                        dir,
                        window,
                        binding.clone(),
                        request,
                        show,
                        look,
                        recipe,
                    )
                });
            #[cfg(not(unix))]
            let result = {
                let _ = (request, show, look, recipe);
                Err::<(), String>("owner-unavailable".into())
            };
            let code = result.as_ref().err().map_or_else(String::new, |error| {
                if error.contains("stale") || error.contains("Stale") {
                    "stale-face-or-show".into()
                } else {
                    "component-mask-selection-unavailable".into()
                }
            });
            socket.send(&Out::FaceComponentMaskResponse {
                protocol: PROTOCOL,
                request_id,
                accepted: result.is_ok(),
                code,
            })?;
        }
        In::FaceWardrobeRequest {
            protocol: PROTOCOL,
            request_id,
            request,
            owner_plan_id,
            basis_revision,
            action,
        } => {
            wardrobe::respond(
                socket,
                state_dir,
                window_id,
                binding,
                request_id,
                request,
                owner_plan_id,
                basis_revision,
                action,
            )?;
        }
        _ => return Err("unsupported Face selection frame".into()),
    }
    Ok(())
}

fn prepare_full_route(
    snapshot: &BrowserAdmittedSnapshot,
    socket: &transport::Socket,
    binding: &LinkBindingId,
    state_dir: Option<&Path>,
    window_id: Option<&str>,
) -> Result<Option<conduit_face::RemoteOwnerMaskRouteSeal>, String> {
    let offered = snapshot
        .browser_advertisement
        .as_deref()
        .is_some_and(|host| {
            host.capabilities
                .contains(&conduit_browser_mask_offer::full_profile::offer())
        });
    if offered {
        select_route(snapshot, socket, binding, state_dir, window_id).map(Some)
    } else {
        Ok(None)
    }
}

fn select_route(
    snapshot: &BrowserAdmittedSnapshot,
    socket: &transport::Socket,
    binding: &LinkBindingId,
    state_dir: Option<&Path>,
    window_id: Option<&str>,
) -> Result<conduit_face::RemoteOwnerMaskRouteSeal, String> {
    #[cfg(unix)]
    {
        let (dir, window) = state_dir
            .zip(window_id)
            .ok_or("owner route requires the installed service actor")?;
        let evidence = socket.line_evidence(snapshot, window)?;
        crate::durable_host_control::browser::mask_route(
            dir,
            window,
            snapshot.credential.clone(),
            binding.clone(),
            evidence,
        )
    }
    #[cfg(not(unix))]
    {
        let _ = (snapshot, socket, binding, state_dir, window_id);
        Err("owner route requires an installed Unix service actor".into())
    }
}

/// Shared production response admission, also exercised by the genuine ABI proof.
pub(super) fn send_response(socket: &mut transport::Socket, frame: &Out) -> Result<(), String> {
    let Out::FaceSnapshotResponse {
        protocol: PROTOCOL,
        response,
    } = frame
    else {
        return Err("unsupported Face response frame".into());
    };
    let fits = match response {
        OwnerFaceSnapshotResponse::Snapshot {
            face,
            route: Some(route),
            ..
        } if conduit_browser_mask_offer::full_profile::selected(&route.planned_mask) => {
            use conduit_browser_mask_offer::full_profile::bounded_json_bytes;
            use conduit_host_browser_make as bounds;
            bounded_json_bytes(face, bounds::OWNER_FACE_FULL_MAXIMUM_FACE_BYTES as usize).is_ok()
                && bounded_json_bytes(route, bounds::OWNER_FACE_FULL_MAXIMUM_ROUTE_BYTES as usize)
                    .is_ok()
                && bounded_json_bytes(
                    response,
                    bounds::OWNER_FACE_FULL_MAXIMUM_RESPONSE_BYTES as usize,
                )
                .is_ok()
        }
        _ => {
            serde_json::to_vec(response)
                .map_err(|e| format!("encode owner Mask route: {e}"))?
                .len()
                <= MAX_OWNER_FACE_RESPONSE_BYTES
        }
    };
    if fits {
        socket.send(frame)
    } else {
        socket.send(&Out::FaceSnapshotResponse {
            protocol: PROTOCOL,
            response: OwnerFaceSnapshotResponse::Refused {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                code: "face-route-frame-pressure".into(),
            },
        })
    }
}
