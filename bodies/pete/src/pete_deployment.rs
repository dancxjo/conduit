//! Deployment-role requirements over reusable target-owned Host configurations.

use crate::PeteWorkloadRole;

pub const PETE_FOREBRAIN_PROFILE: &str = "targets/std/profiles/linux-computer.host.conduit";
pub const PETE_MOTHERBRAIN_PROFILE: &str = "targets/std/profiles/linux-computer.host.conduit";
pub const PETE_BRAINSTEM_PROFILE: &str = "targets/std/profiles/linux-serial.host.conduit";
pub const PETE_OPTIONAL_BROWSER_PROFILE: &str =
    "targets/browser/profiles/browser-rich.host.conduit";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeteHostRole {
    Forebrain,
    Motherbrain,
    Brainstem,
    OptionalBrowser,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeteHostRequirement {
    pub role: PeteHostRole,
    pub target_owned_configuration: &'static str,
    pub required_at_birth: bool,
    pub contributes_to: Vec<PeteWorkloadRole>,
}

pub fn reviewed_pete_host_requirements() -> Vec<PeteHostRequirement> {
    vec![
        PeteHostRequirement {
            role: PeteHostRole::Forebrain,
            target_owned_configuration: PETE_FOREBRAIN_PROFILE,
            required_at_birth: true,
            contributes_to: vec![
                PeteWorkloadRole::AutobiographicalMemory,
                PeteWorkloadRole::HistoricalIndex,
                PeteWorkloadRole::Conversation,
                PeteWorkloadRole::Homeostasis,
            ],
        },
        PeteHostRequirement {
            role: PeteHostRole::Motherbrain,
            target_owned_configuration: PETE_MOTHERBRAIN_PROFILE,
            required_at_birth: true,
            contributes_to: vec![
                PeteWorkloadRole::Situation,
                PeteWorkloadRole::AutobiographicalMemory,
                PeteWorkloadRole::HistoricalIndex,
                PeteWorkloadRole::Homeostasis,
            ],
        },
        PeteHostRequirement {
            role: PeteHostRole::Brainstem,
            target_owned_configuration: PETE_BRAINSTEM_PROFILE,
            required_at_birth: true,
            contributes_to: vec![
                PeteWorkloadRole::Situation,
                PeteWorkloadRole::Navigation,
                PeteWorkloadRole::Homeostasis,
            ],
        },
        PeteHostRequirement {
            role: PeteHostRole::OptionalBrowser,
            target_owned_configuration: PETE_OPTIONAL_BROWSER_PROFILE,
            required_at_birth: false,
            contributes_to: vec![PeteWorkloadRole::Conversation],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_is_optional_and_roles_reuse_target_owned_profiles() {
        let requirements = reviewed_pete_host_requirements();
        assert_eq!(requirements.len(), 4);
        assert!(requirements
            .iter()
            .filter(|item| item.required_at_birth)
            .all(|item| item.role != PeteHostRole::OptionalBrowser));
        let browser = requirements
            .iter()
            .find(|item| item.role == PeteHostRole::OptionalBrowser)
            .unwrap();
        assert!(!browser.required_at_birth);
        assert!(requirements.iter().all(|item| {
            item.target_owned_configuration.starts_with("targets/")
                && !item.target_owned_configuration.contains("bodies/pete")
                && !item.contributes_to.is_empty()
        }));
    }
}
