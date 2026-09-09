//! Logical deliveries from the one keyboard already initialized by this Boot.
//!
//! This implementation receives canonical events from the native input adapter;
//! it never opens or polls a device. Its two reservations are delivery slots,
//! not duplicate claims on the physical controller/device/interface/endpoint.
use alloc::{format, vec};
use conduit_core::{HostAdvertisement, resource_offer, resource_requirement};

use super::WorksetRefusal;
use crate::keyboard_offer::{KeyboardOffer, KeyboardRealization};

pub(super) const IMPLEMENTATION: &str = "conduitos/body-keyboard-delivery@1";
pub(super) const RESOURCE: &str = "conduitos.resource/body-keyboard-delivery-slot@1";
pub(super) const OPERATION: &str = "conduit.host/body-next-key-event@1";

pub(super) fn install(
    host: &mut HostAdvertisement,
    keyboard: KeyboardOffer<'_>,
    build: &str,
) -> Result<KeyboardRealization, WorksetRefusal> {
    keyboard.validate(build).map_err(|_| WorksetRefusal::Host)?;
    let mut offer = conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::keyboard_contract(),
        conduit_semantic_catalog::KEYBOARD_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            execution_profile: "conduitos/foreground-keyboard-delivery@1",
            artifact: "conduitos/body-keyboard-delivery@1",
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: OPERATION.into(),
            target_kind: Some(conduit_human::KEY_EVENT_INFO_ID.into()),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        }],
        // Charge each delivery the whole fixed adapter envelope conservatively;
        // even a one-Form Body must admit its tables, Signs and retained editor.
        vec![
            resource_requirement(
                "conduit.resource/runtime-memory@1",
                (core::mem::size_of::<super::NativeWorksetPlay>() + 512) as u32,
            ),
            resource_requirement(RESOURCE, 1),
        ],
        vec![],
    );
    offer.implementation.artifact_id = format!("conduitos-build/{build}").into();
    offer.limits.max_active_instances = 2;
    offer.resource_requirements.sort();
    // The native Body has only the initialized delivery implementation. The
    // separate direct-device proof entrance retains its own physical offer.
    host.capabilities
        .retain(|offer| offer.kind_id.as_str() != conduit_semantic_catalog::KEYBOARD_KIND);
    host.capabilities.push(offer);
    host.resources.push(resource_offer(
        &format!(
            "conduitos-keyboard-deliveries-{}-{}",
            host.boot_id.as_str(),
            crate::identity::hex(&keyboard.realization.endpoint_id)
        ),
        RESOURCE,
        2,
    ));
    host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    Ok(keyboard.realization)
}
