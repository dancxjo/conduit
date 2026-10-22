//! Edit-current opens only an already-admitted, empty canonical resident editor.
use crate::{
    front_door::FrontDoor,
    identity::BootIdentities,
    product_journey::{ProductJourney, SourceEditError},
};

pub(super) fn open_source_editor(
    front_door: &mut FrontDoor,
    journey: &mut ProductJourney,
    request: patchbay_application::PatchbayApplicationRequest,
    identities: &BootIdentities,
) -> Result<(), &'static str> {
    match journey.open_patchbay_source(request, identities) {
        Ok(receipt) => {
            let value = serde_json::to_value(receipt).map_err(|_| "native-source-edit-receipt")?;
            super::native_action_evidence::emit_with_limit(
                b"CONDUIT_NATIVE_SOURCE_EDITOR ",
                &value,
                4096,
            )?;
        }
        Err(SourceEditError::Refused(refusal)) => {
            front_door
                .observe_product(journey)
                .map_err(|_| "native-source-editor-observation")?;
            front_door
                .action_refused(refusal.as_str())
                .map_err(|_| "native-source-editor-refusal-presentation")?;
            return Ok(());
        }
        Err(SourceEditError::Fatal(error)) => return Err(error.as_str()),
    }
    front_door
        .observe_product(journey)
        .map_err(|_| "native-source-editor-observation")?;
    Ok(())
}
