use conduit_core::CapabilityOffer;
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity, StandardKindContract};

pub const ROBOTICS_EXECUTION_PROFILE: &str = "conduit.std/robotics-prewake-sim-kernel@1";
pub const ROBOTICS_ARTIFACT: &str = "conduit-std-host/robotics-prewake-sim@1";
pub const ROBOTICS_OBSERVE_BUMP_IMPLEMENTATION: &str = "std/kernel-robotics-prewake-observe-bump@1";
pub const ROBOTICS_OBSERVE_IMU_IMPLEMENTATION: &str = "std/kernel-robotics-prewake-observe-imu@1";
pub const ROBOTICS_OBSERVE_RANGE_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-observe-range@1";
pub const ROBOTICS_OBSERVE_ODOMETRY_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-observe-odometry@1";
pub const ROBOTICS_OBSERVE_BATTERY_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-observe-battery@1";
pub const ROBOTICS_VELOCITY_INTENT_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-velocity-intent@1";
pub const ROBOTICS_DRIVE_DIFFERENTIAL_IMPLEMENTATION: &str =
    "std/kernel-robotics-prewake-drive-differential@2";

pub fn robotics_observe_bump_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::robotics_observe_bump_contract(),
        conduit_semantic_catalog::ROBOTICS_OBSERVE_BUMP_REVISION,
        "observe-bump",
        ROBOTICS_OBSERVE_BUMP_IMPLEMENTATION,
    )
}

pub fn robotics_observe_imu_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::robotics_observe_imu_contract(),
        conduit_semantic_catalog::ROBOTICS_OBSERVE_IMU_REVISION,
        "observe-imu",
        ROBOTICS_OBSERVE_IMU_IMPLEMENTATION,
    )
}

pub fn robotics_observe_range_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::robotics_observe_range_contract(),
        conduit_semantic_catalog::ROBOTICS_OBSERVE_RANGE_REVISION,
        "observe-range",
        ROBOTICS_OBSERVE_RANGE_IMPLEMENTATION,
    )
}

pub fn robotics_observe_odometry_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::robotics_observe_odometry_contract(),
        conduit_semantic_catalog::ROBOTICS_OBSERVE_ODOMETRY_REVISION,
        "observe-odometry",
        ROBOTICS_OBSERVE_ODOMETRY_IMPLEMENTATION,
    )
}

pub fn robotics_observe_battery_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::robotics_observe_battery_contract(),
        conduit_semantic_catalog::ROBOTICS_OBSERVE_BATTERY_REVISION,
        "observe-battery",
        ROBOTICS_OBSERVE_BATTERY_IMPLEMENTATION,
    )
}

pub fn robotics_velocity_intent_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::robotics_velocity_intent_contract(),
        conduit_semantic_catalog::ROBOTICS_VELOCITY_INTENT_REVISION,
        "velocity-intent",
        ROBOTICS_VELOCITY_INTENT_IMPLEMENTATION,
    )
}

pub fn robotics_drive_differential_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::robotics_drive_differential_contract(),
        conduit_semantic_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_REVISION,
        "drive-differential",
        ROBOTICS_DRIVE_DIFFERENTIAL_IMPLEMENTATION,
    )
}

fn offer(
    contract: StandardKindContract,
    revision: &str,
    slug: &str,
    implementation: &str,
) -> CapabilityOffer {
    realization_offer(
        contract,
        revision,
        RealizationOfferIdentity {
            capability: &format!("robotics-prewake-sim-{slug}"),
            execution_profile: ROBOTICS_EXECUTION_PROFILE,
            implementation,
            artifact: ROBOTICS_ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_robotics_offers_are_finite_and_authority_free() {
        for offer in [
            robotics_observe_bump_offer(),
            robotics_observe_imu_offer(),
            robotics_observe_range_offer(),
            robotics_observe_odometry_offer(),
            robotics_observe_battery_offer(),
            robotics_velocity_intent_offer(),
            robotics_drive_differential_offer(),
        ] {
            assert_eq!(offer.limits.max_queue_items, 1);
            assert!(offer.host_operations.is_empty());
            assert!(offer.resource_requirements.is_empty());
            assert!(offer.authority_requirements.is_empty());
            assert!(offer
                .implementation
                .implementation_id
                .as_str()
                .contains("prewake"));
        }
    }
}
