//! Exact Signal capability offer for the inspected classic ESP32 sample.
//!
//! This module describes planning facts only. It does not claim that an image
//! was flashed, booted, or observed on physical hardware.

use alloc::vec;

use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, OfferGeneration, PROTOCOL_VERSION,
};

use crate::{
    pulse_contract_revision, pulse_execution_profile, pulse_face_startup_parameters,
    pulse_host_operation_requirements, pulse_kind, pulse_outputs, pulse_resource_requirements,
    show_contract_revision, show_execution_profile, show_host_operation_requirements, show_inputs,
    show_kind, show_resource_requirements, signal_resource_offers,
    DISTRIBUTED_MAXIMUM_BUFFERED_BYTES, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
};

pub const ESP32_WROOM_LOCAL_HOST_ID: &str = "esp32/wroom32/hw-463-sample";
pub const ESP32_WROOM_LOCAL_BOOT_ID: &str = "esp32/wroom32/build-plan-boot";
pub const ESP32_WROOM_TIMER_POOL_ID: &str = "esp32/wroom32/systimer";
pub const ESP32_WROOM_PRESENTATION_POOL_ID: &str = "esp32/wroom32/uart0-sign";

pub fn esp32_wroom_local_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(ESP32_WROOM_LOCAL_HOST_ID),
        boot_id: BootId::from(ESP32_WROOM_LOCAL_BOOT_ID),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("esp32-wroom32-signal-kernel"),
        resources: signal_resource_offers(
            ESP32_WROOM_TIMER_POOL_ID,
            ESP32_WROOM_PRESENTATION_POOL_ID,
            1,
        ),
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: pulse_face_startup_parameters(),
                shorthand: None,
                capability_id: CapabilityId::from("esp32-wroom-pulse-1"),
                kind_id: pulse_kind(),
                kind_contract_revision: pulse_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: pulse_execution_profile(),
                    implementation_id: ImplementationId::from(
                        "esp32-wroom/kernel-pulse-systimer-v1",
                    ),
                    artifact_id: ArtifactId::from("conduit-signal/esp32-wroom-pulse-v1"),
                },
                inputs: vec![],
                outputs: pulse_outputs(),
                host_operations: pulse_host_operation_requirements(),
                resource_requirements: pulse_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                    max_queue_bytes: DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
                },
            },
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("esp32-wroom-uart-show-1"),
                kind_id: show_kind(),
                kind_contract_revision: show_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from(
                        "esp32-wroom/kernel-uart0-show-signal-v1",
                    ),
                    artifact_id: ArtifactId::from("conduit-signal/esp32-wroom-uart-show-v1"),
                },
                inputs: show_inputs(),
                outputs: vec![],
                host_operations: show_host_operation_requirements(),
                resource_requirements: show_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
                    max_queue_bytes: DISTRIBUTED_MAXIMUM_BUFFERED_BYTES,
                },
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PULSE_KIND, SHOW_KIND};

    #[test]
    fn offer_is_exact_finite_and_keeps_portable_kinds() {
        let offer = esp32_wroom_local_advertisement();
        assert_eq!(offer.host_id.as_str(), ESP32_WROOM_LOCAL_HOST_ID);
        assert_eq!(offer.resources.len(), 2);
        assert!(offer
            .resources
            .iter()
            .all(|resource| resource.capacity_units == 1));
        assert_eq!(offer.capabilities.len(), 2);
        assert_eq!(offer.capabilities[0].kind_id.as_str(), PULSE_KIND);
        assert_eq!(offer.capabilities[1].kind_id.as_str(), SHOW_KIND);
        assert!(offer.capabilities.iter().all(|capability| {
            capability.limits.max_active_instances == 1
                && capability.limits.max_queue_items == 1
                && capability.limits.max_queue_bytes == crate::SIGNAL_ENCODED_LEN
        }));
    }
}
