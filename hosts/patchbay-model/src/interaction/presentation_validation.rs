//! Exact Presentation action validation before ordinary interaction execution.

use super::{PatchbayInteractionRequest, PatchbayRefusal};

pub(super) fn validate_presentation_invocation(
    presentation: &conduit_presentation::Presentation,
    request: &PatchbayInteractionRequest,
) -> Result<(), PatchbayRefusal> {
    let PatchbayInteractionRequest::Invoke { invocation, .. } = request else {
        return Ok(());
    };
    if invocation.presentation_id != presentation.identity.as_str()
        || invocation.presentation_revision != presentation.revision
    {
        return Err(PatchbayRefusal::StalePresentation);
    }
    let action = presentation
        .resolve_action(invocation.presentation_revision, &invocation.action_id)
        .map_err(|error| match error {
            conduit_presentation::PresentationActionRefusal::StaleRevision => {
                PatchbayRefusal::StalePresentation
            }
            conduit_presentation::PresentationActionRefusal::UnknownAction => {
                PatchbayRefusal::UnknownAction
            }
            conduit_presentation::PresentationActionRefusal::Unavailable { .. } => {
                PatchbayRefusal::ActionUnavailable
            }
            conduit_presentation::PresentationActionRefusal::Refused { .. } => {
                PatchbayRefusal::ActionRefused
            }
        })?;
    if action.target != invocation.target_identity {
        return Err(PatchbayRefusal::WrongTarget);
    }
    Ok(())
}
