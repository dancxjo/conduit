//! Ordinary Conduit realizations for Netherwick's Create hardware.

mod capstone;
mod capstone_kernel;
mod capstone_operations;
mod capstone_play;
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
mod create_power_service;
mod create_sensor_lowering;
mod create_speaker;
mod create_speaker_play;
mod imu_calibration_service;
mod imu_observation;
mod imu_play;
mod planning;
mod profile;
mod ssd1306_frame;
mod ssd1306_presenter;

pub use capstone::*;
pub use capstone_play::*;
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
pub use create_power_service::*;
pub use create_sensor_lowering::*;
pub use create_speaker::*;
pub use create_speaker_play::*;
pub use imu_calibration_service::*;
pub use imu_observation::*;
pub use imu_play::*;
pub use planning::*;
pub use profile::*;
pub use ssd1306_frame::*;
pub use ssd1306_presenter::*;

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
