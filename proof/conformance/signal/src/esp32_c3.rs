//! Build-time Signal planning fixture for the physically inspected ESP32-C3.
//!
//! The shared ESP32 implementation shape is retained, while every
//! architecture- and artifact-bearing identity is replaced before planning.

use conduit_core::{ArtifactId, BootId, CapabilityId, HostId, HostProfileId, ImplementationId};

use crate::{esp32_wroom_build_fixture_advertisement, PULSE_KIND, SHOW_KIND};

pub const ESP32_C3_BUILD_FIXTURE_HOST_ID: &str = "fixture/build/esp32/c3/usb-dcf8355d";
pub const ESP32_C3_BUILD_FIXTURE_BOOT_ID: &str = "fixture/build/esp32/c3/no-boot";
pub const ESP32_C3_BUILD_FIXTURE_TIMER_POOL_ID: &str = "fixture/build/esp32/c3/systimer";
pub const ESP32_C3_BUILD_FIXTURE_PRESENTATION_POOL_ID: &str = "fixture/build/esp32/c3/uart0-sign";

/// Supplies exact finite planner input during ESP32-C3 IMAGE construction.
pub fn esp32_c3_build_fixture_advertisement() -> conduit_core::HostAdvertisement {
    let mut offer = esp32_wroom_build_fixture_advertisement();
    offer.host_id = HostId::from(ESP32_C3_BUILD_FIXTURE_HOST_ID);
    offer.boot_id = BootId::from(ESP32_C3_BUILD_FIXTURE_BOOT_ID);
    offer.profile = HostProfileId::from("esp32-c3-signal-kernel");
    offer.resources[0].pool_id = ESP32_C3_BUILD_FIXTURE_TIMER_POOL_ID.into();
    offer.resources[1].pool_id = ESP32_C3_BUILD_FIXTURE_PRESENTATION_POOL_ID.into();
    for capability in &mut offer.capabilities {
        match capability.kind_id.as_str() {
            PULSE_KIND => {
                capability.capability_id = CapabilityId::from("esp32-c3-pulse-1");
                capability.implementation.implementation_id =
                    ImplementationId::from("esp32-c3/kernel-pulse-systimer-v1");
                capability.implementation.artifact_id =
                    ArtifactId::from("conduit-signal/esp32-c3-pulse-v1");
            }
            SHOW_KIND => {
                capability.capability_id = CapabilityId::from("esp32-c3-uart-show-1");
                capability.implementation.implementation_id =
                    ImplementationId::from("esp32-c3/kernel-uart0-show-signal-v1");
                capability.implementation.artifact_id =
                    ArtifactId::from("conduit-signal/esp32-c3-uart-show-v1");
            }
            _ => unreachable!("the shared ESP32 fixture has only Signal capabilities"),
        }
    }
    offer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c3_fixture_replaces_every_wroom_identity() {
        let offer = esp32_c3_build_fixture_advertisement();
        let encoded = format!("{offer:?}");
        assert_eq!(offer.host_id.as_str(), ESP32_C3_BUILD_FIXTURE_HOST_ID);
        assert_eq!(offer.boot_id.as_str(), ESP32_C3_BUILD_FIXTURE_BOOT_ID);
        assert!(!encoded.contains("wroom"));
        assert!(offer
            .capabilities
            .iter()
            .all(|capability| capability.capability_id.as_str().contains("esp32-c3")));
    }
}
