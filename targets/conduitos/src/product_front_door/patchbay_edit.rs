//! A Source edit needs its own admitted document editor, not the keyboard sample.
use crate::front_door::FrontDoor;

pub(super) fn refuse_unadmitted_editor(
    front_door: &mut FrontDoor,
    journey: &crate::product_journey::ProductJourney,
) -> Result<(), &'static str> {
    front_door
        .observe_product(journey)
        .map_err(|_| "native-source-editor-observation")?;
    front_door
        .action_refused("native-source-editor-not-admitted")
        .map_err(|_| "native-source-editor-refusal-presentation")?;
    crate::arch::early_write(
        b"CONDUIT_WORKSPACE_CHECKPOINT patchbay-edit-refused reason=native-source-editor-not-admitted\n",
    );
    Ok(())
}
