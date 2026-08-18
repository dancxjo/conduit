//! Describe-only projection of one pinned Netherwick robot configuration.

mod create_drive;
mod create_drive_kernel_operations;
mod create_drive_lowering;
mod create_drive_offer;
mod create_drive_plan_validation;
mod create_drive_play;
mod create_observation_execution_report;
mod create_observation_kernel_operations;
mod create_observation_offer;
mod create_observation_plan_validation;
mod create_observation_play;
mod create_observation_session;
mod create_oi;
mod create_sensor_lowering;
mod create_speaker;
mod create_speaker_play;
mod planning;
mod profile;
mod projection;

pub use create_drive::*;
pub use create_drive_lowering::*;
pub use create_drive_offer::*;
pub use create_drive_play::*;
pub use create_observation_execution_report::*;
pub use create_observation_offer::*;
pub use create_observation_play::*;
pub use create_observation_session::*;
pub use create_oi::*;
pub use create_sensor_lowering::*;
pub use create_speaker::*;
pub use create_speaker_play::*;
pub use planning::*;
pub use profile::*;
pub use projection::*;

pub const NETHERWICK_REVISION: &str = "f43ff13846b47b05e133d0321bdbaafffd1bcdbe";

pub fn pinned_profile() -> pete_brainstem::conduit_robotics::DescribeOnlyReport {
    pete_brainstem::conduit_robotics::pico_w_report()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumes_pinned_effect_free_netherwick_profile() {
        let report = pinned_profile();
        assert_eq!(report.body_kind, "create_oi");
        assert!(report.effect_audit.is_effect_free());
        assert!(report.identities.authority.is_none());
        assert_eq!(report.implementation.source_commit, NETHERWICK_REVISION);
    }
}
