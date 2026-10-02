//! Current shared view evidence emitted only after successful native presentation.
use crate::{
    arch, front_door::FrontDoor, native_compositor::CompositionReceipt,
    product_journey::ProductJourney,
};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub(super) fn emit(
    door: &FrontDoor,
    journey: &ProductJourney,
    receipt: &CompositionReceipt,
) -> Result<(), &'static str> {
    let Some(view) = door.application_view() else {
        return Ok(());
    };
    let encoded = view
        .encode()
        .map_err(|_| "workspace-view-evidence-invalid")?;
    if encoded.len() > 3 * 1024 {
        return Err("workspace-view-evidence-capacity-exceeded");
    }
    let digest: [u8; 32] = Sha256::digest(&encoded).into();
    let projection = journey.projection();
    let record = serde_json::json!({
        "schema": "conduit.conduitos/workspace-view@1",
        "application": if journey.foreground_is_tutorial() { "tutorial" } else { "patchbay" },
        "view_revision": view.revision,
        "front_door_revision": door.revision(),
        "view_sha256": crate::identity::hex(&digest),
        "body_id": projection.body_id.as_ref().map(|id| id.as_str()),
        "status": projection.status.as_str(),
        "page": door.application_page(),
        "selected_action": door.selected_application_action().map(|action| action.id.as_str()),
        "nodes": view.nodes.iter().map(|node| serde_json::json!({"key": node.key, "text": node.text, "value": node.value})).collect::<Vec<_>>(),
        "actions": view.actions.iter().map(|action| action.id.as_str()).collect::<Vec<_>>(),
        "presentation_id": receipt.presentation_id.as_str(),
        "manifestation_id": receipt.manifestation_id.as_str(),
    });
    let bytes = serde_json::to_vec(&record).map_err(|_| "workspace-view-evidence-encoding")?;
    if bytes.len() > 16 * 1024 {
        return Err("workspace-view-evidence-capacity-exceeded");
    }
    arch::early_write(b"CONDUIT_WORKSPACE_VIEW ");
    arch::early_write(&bytes);
    arch::early_write(b"\n");
    Ok(())
}
