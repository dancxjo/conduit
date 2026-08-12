//! Describe-only projection of one pinned Netherwick robot configuration.

mod create_speaker;
mod planning;
mod profile;
mod projection;

pub use create_speaker::*;
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
