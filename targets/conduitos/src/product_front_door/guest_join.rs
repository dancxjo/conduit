//! Exact admission handoff into the native guest's current Face.

use conduit_body::PortableAdmissionReceipt;
use conduit_core::{BootId, HostId, OfferGeneration};

use crate::{
    arch, front_door::FrontDoor, native_guest_part::NativeGuestPart,
    product_journey::JourneyProjection, spore_join::PendingNativeJoin,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn enter(
    door: &mut FrontDoor,
    journey: JourneyProjection,
    pending: PendingNativeJoin,
    receipt: Option<PortableAdmissionReceipt>,
    host_id: &HostId,
    boot_id: &BootId,
    generation: OfferGeneration,
) -> Result<(), &'static str> {
    let joined = receipt
        .map(|receipt| NativeGuestPart::install(&pending, receipt, host_id, boot_id, generation))
        .transpose()
        .map_err(|error| error.as_str())?;
    door.observe_journey(journey)
        .map_err(|error| error.as_str())?;
    door.await_join(pending).map_err(|error| error.as_str())?;
    if let Some(joined) = joined {
        let credential = joined.credential();
        let observation = serde_json::to_vec(&serde_json::json!({
            "schema": "conduit.conduitos/native-guest-part@1",
            "body_id": credential.body_id.as_str(),
            "part_id": credential.part_id.as_str(),
            "host_id": credential.host_id.as_str(),
            "boot_id": credential.boot_id.as_str(),
            "membership_installed": true,
            "workset_known_here": false,
            "plan_created": false,
            "play_created": false,
        }))
        .map_err(|_| "native-guest-observation-encoding-invalid")?;
        if observation.len() > 1_024 {
            return Err("native-guest-observation-bound-exceeded");
        }
        door.install_join(joined).map_err(|error| error.as_str())?;
        arch::early_write(b"CONDUIT_NATIVE_GUEST_PART ");
        arch::early_write(&observation);
        arch::early_write(b"\nCONDUIT_JOIN_CHECKPOINT part-installed\n");
    }
    Ok(())
}
