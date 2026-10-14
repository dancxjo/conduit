//! Exact Face action validation before ordinary interaction execution.

use super::{PatchbayInteractionRequest, PatchbayRefusal};

pub(super) fn validate_face_invocation(
    face: &conduit_face::Face,
    request: &PatchbayInteractionRequest,
) -> Result<(), PatchbayRefusal> {
    let PatchbayInteractionRequest::Invoke { invocation, .. } = request else {
        return Ok(());
    };
    if invocation.face_id != face.identity.as_str() || invocation.face_revision != face.revision {
        return Err(PatchbayRefusal::StaleFace);
    }
    let action = face
        .resolve_action(invocation.face_revision, &invocation.action_id)
        .map_err(|error| match error {
            conduit_face::FaceActionRefusal::StaleRevision => PatchbayRefusal::StaleFace,
            conduit_face::FaceActionRefusal::UnknownAction => PatchbayRefusal::UnknownAction,
            conduit_face::FaceActionRefusal::Unavailable { .. } => {
                PatchbayRefusal::ActionUnavailable
            }
            conduit_face::FaceActionRefusal::Refused { .. } => PatchbayRefusal::ActionRefused,
        })?;
    if action.target != invocation.target_identity {
        return Err(PatchbayRefusal::WrongTarget);
    }
    Ok(())
}
