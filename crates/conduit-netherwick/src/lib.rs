//! Ordinary Conduit realizations for Netherwick's Create hardware.

mod create_dock;
mod create_dock_kernel_operations;
mod create_dock_plan_validation;
mod create_dock_play;
mod create_drive_kernel_operations;
mod create_drive_lowering;
mod create_drive_offer;
mod create_drive_plan_validation;
mod create_drive_play;
mod create_indicator;
mod create_mode_service;
mod create_observation_execution_report;
mod create_observation_kernel_operations;
mod create_observation_offer;
mod create_observation_plan_validation;
mod create_observation_play;
mod create_observation_session;
mod create_odometry;
mod create_sensor_lowering;
mod create_speaker;
mod create_speaker_play;
mod planning;
mod profile;

pub use conduit_create_oi::*;
pub use create_dock::*;
pub use create_dock_play::*;
pub use create_drive_lowering::*;
pub use create_drive_offer::*;
pub use create_drive_play::*;
pub use create_indicator::*;
pub use create_mode_service::*;
pub use create_observation_execution_report::*;
pub use create_observation_offer::*;
pub use create_observation_play::*;
pub use create_observation_session::*;
pub use create_odometry::*;
pub use create_sensor_lowering::*;
pub use create_speaker::*;
pub use create_speaker_play::*;
pub use planning::*;
pub use profile::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_realization_ttl_profile_matches_portable_robotics_contract() {
        assert_eq!(
            MINIMUM_MOTION_TTL_MS,
            conduit_std_catalog::ROBOTICS_MINIMUM_MOTION_TTL_MS as u32
        );
        assert_eq!(
            MAXIMUM_MOTION_TTL_MS,
            conduit_std_catalog::ROBOTICS_MAXIMUM_MOTION_TTL_MS as u32
        );
    }
}
