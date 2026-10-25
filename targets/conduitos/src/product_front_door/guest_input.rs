//! Guest-owner input keeps invitation admission separate from local Body lifecycle.
use super::*;
use crate::{native_guest_face::NativeGuestFace, native_owner_return::NativeOwnerReturnRoute};

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch(
    input: ControllerInput,
    owner_face_presented: bool,
    identities: &BootIdentities,
    face_arrival: &mut FaceArrival,
    current_owner_face: &mut Option<NativeGuestFace>,
    owner_route: &mut Option<NativeOwnerReturnRoute>,
    owner_route_standby: &mut bool,
    display: &mut impl crate::display::PixelTarget,
) -> Result<ControllerControl, &'static str> {
    match input {
        ControllerInput::Service
            if owner_route.as_ref().is_some_and(|route| !route.available()) =>
        {
            *owner_route = None;
            *owner_route_standby = false;
            face_arrival.retire_owner_route(display)?;
            face_arrival.show_owner_result(false, false, "return-expired", display)?;
            arch::early_write(b"CONDUIT_NATIVE_OWNER_ROUTE {\"schema\":\"conduit.conduitos/native-owner-route@1\",\"status\":\"expired\"}\n");
        }
        ControllerInput::Key(event) if owner_face_presented => {
            if let Some(disposition) = owner_selection::dispatch(
                event,
                *identities,
                face_arrival,
                current_owner_face,
                owner_route,
                *owner_route_standby,
                display,
            )? {
                if let owner_selection::SelectionDisposition::Refused(reason) = disposition {
                    owner_selection::record_refusal(reason);
                }
                return Ok(ControllerControl::Continue);
            }
            if *owner_route_standby
                && event.transition() == KeyTransition::Pressed
                && event.usage() == F5
            {
                let prior = current_owner_face
                    .as_ref()
                    .ok_or("native-owner-face-absent")?;
                let fresh = match owner_route
                    .as_mut()
                    .ok_or("native-owner-return-unavailable")?
                    .refresh(*identities, prior)
                {
                    Ok(face) => face,
                    Err(reason) => {
                        arch::early_write(b"CONDUIT_NATIVE_OWNER_REFRESH {\"schema\":\"conduit.conduitos/native-owner-refresh@1\",\"status\":\"refused\",\"code\":\"");
                        arch::early_write(reason.as_bytes());
                        arch::early_write(b"\"}\n");
                        *owner_route = None;
                        *owner_route_standby = false;
                        face_arrival.retire_owner_route(display)?;
                        face_arrival.show_owner_result(false, false, reason, display)?;
                        return Ok(ControllerControl::Continue);
                    }
                };
                face_arrival.activate_refreshed_owner_route(fresh.face().clone(), display)?;
                *current_owner_face = Some(fresh);
                let route = owner_route
                    .as_mut()
                    .ok_or("native-owner-return-unavailable")?;
                match face_arrival.acknowledge_owner_show(route, *identities) {
                    Ok(()) => {
                        *owner_route_standby = false;
                        arch::early_write(b"CONDUIT_NATIVE_OWNER_ROUTE {\"schema\":\"conduit.conduitos/native-owner-route@1\",\"status\":\"active\"}\n");
                    }
                    Err(reason) => {
                        arch::early_write(
                            b"CONDUIT_NATIVE_OWNER_SHOW_ACK {\"status\":\"refused\",\"code\":\"",
                        );
                        arch::early_write(reason.as_bytes());
                        arch::early_write(b"\"}\n");
                        *owner_route = None;
                        *owner_route_standby = false;
                        face_arrival.retire_owner_route(display)?;
                        face_arrival.show_owner_result(false, false, reason, display)?;
                    }
                }
                return Ok(ControllerControl::Continue);
            }
            let input = match face_arrival.accept_guest_key(event, display) {
                Ok(input) => input,
                Err(reason) => {
                    arch::early_write(b"CONDUIT_NATIVE_OWNER_INPUT {\"schema\":\"conduit.conduitos/native-owner-input@1\",\"status\":\"refused\",\"code\":\"");
                    arch::early_write(reason.as_bytes());
                    arch::early_write(b"\"}\n");
                    face_arrival.show_local_refusal(reason, display)?;
                    return Ok(ControllerControl::Continue);
                }
            };
            if let Some((show, interaction)) = input {
                let outcome = owner_route
                    .as_mut()
                    .ok_or("native-owner-return-unavailable")
                    .and_then(|route| route.submit(*identities, &show, &interaction));
                match outcome {
                    Ok(outcome) => {
                        let admitted = owner_route.as_ref().is_some_and(|route| route.available());
                        owner_action_evidence::emit(&show, &interaction, &outcome);
                        let refreshed = outcome.face.is_some();
                        if let Some(face) = outcome.face {
                            *current_owner_face = Some(face.clone());
                            face_arrival.present_owner_face(face.into_face(), admitted, display)?;
                            if admitted
                                && let Some(route) = owner_route.as_mut()
                                && let Err(reason) =
                                    face_arrival.acknowledge_owner_show(route, *identities)
                            {
                                *owner_route = None;
                                face_arrival.retire_owner_route(display)?;
                                face_arrival.show_owner_result(false, false, reason, display)?;
                                return Ok(ControllerControl::Continue);
                            }
                        } else {
                            face_arrival.retire_owner_route(display)?;
                        }
                        face_arrival.show_owner_result(
                            outcome.accepted,
                            refreshed,
                            &outcome.code,
                            display,
                        )?;
                    }
                    Err(reason) => {
                        arch::early_write(b"CONDUIT_NATIVE_OWNER_ACTION {\"schema\":\"conduit.conduitos/native-owner-action@1\",\"status\":\"");
                        arch::early_write(if reason == "control-outcome-unknown" {
                            b"unknown"
                        } else {
                            b"refused"
                        });
                        arch::early_write(b"\",\"code\":\"");
                        arch::early_write(reason.as_bytes());
                        arch::early_write(b"\"}\n");
                        *owner_route = None;
                        face_arrival.retire_owner_route(display)?;
                        face_arrival.show_owner_result(false, false, reason, display)?;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(ControllerControl::Continue)
}
