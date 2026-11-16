//! Closed typed outgoing owners. A wire ceiling funds an output SLOT, never an
//! estimate of retained DTO backing. Unknown or generic JSON owners refuse.
use super::*;
use std::mem::size_of_val;
pub(super) fn envelope(response: &Response) -> Option<usize> {
    let boxed = match response {
        Response::BodyMemberCarrierReceiver { receiver, .. } => {
            size_of_val(receiver.as_ref()).checked_add(receiver.retained_backing_bytes()?)?
        }
        Response::BodyMemberCarrierChallenge {
            challenge,
            signature,
            ..
        } => size_of_val(challenge.as_ref())
            .checked_add(challenge.retained_backing_bytes()?)?
            .checked_add(signature.capacity())?,
        Response::BodyMemberCarrierInspection { inspection, .. } => {
            size_of_val(inspection.as_ref()).checked_add(inspection.retained_backing_bytes()?)?
        }
        Response::BodyMemberPresenceRenewed { lease, .. } => {
            size_of_val(lease.as_ref()).checked_add(lease.retained_backing_bytes()?)?
        }
        Response::BodyNativeMaskRoute { route, .. } => {
            crate::durable_host::owner::Owner::native_reply_seal_bytes(route)?
        }
        Response::BodyMemberCarrierRetired { .. } | Response::BodyNativeMaskShowAccepted { .. } => {
            0
        }
        // Generic Snapshot/attestation/JSON action replies deliberately retain
        // their existing unqualified path; the producer must not attach a guard.
        _ => return None,
    };
    let codec = match response {
        Response::BodyNativeMaskRoute { route, .. } => {
            route.planned_mask.json_workspace()?.codec_bytes
        }
        _ => 0,
    };
    size_of::<GuardedResponse>()
        .checked_add(size_of::<RefCell<Option<Response>>>())?
        .checked_add(boxed)?
        .checked_add(codec)?
        .checked_add(MAXIMUM_LOCAL_CONTROL_FRAME_BYTES)
}
