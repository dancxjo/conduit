use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::{
    ActivePlayId, ArtifactId, AuthorityBinding, AuthorityGrantId, BootId, CapabilityId,
    CheckedFace, CheckedFormId, GearId, HostId, ImplementationId, LineId, PlacementId, PlanId,
};
use serde::{Deserialize, Serialize};

macro_rules! identity {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(String);

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

identity!(DurableSystemId);
identity!(RoleId);
identity!(TransitionId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInstance {
    pub host_id: HostId,
    pub boot_id: BootId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleRequirement {
    pub role_id: RoleId,
    pub gear_id: GearId,
    pub checked_face: CheckedFace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactAssignment {
    pub role_id: RoleId,
    pub placement_id: PlacementId,
    pub host: HostInstance,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub checked_face: CheckedFace,
}

/// An external authority fact consumed by continuity. This crate never issues it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedTransitionGrant {
    pub grant_id: AuthorityGrantId,
    pub controller: HostInstance,
    pub subject: HostInstance,
    pub capability_id: CapabilityId,
    pub selected_line_id: LineId,
    pub maximum_transitions: u16,
    pub proof_window_ticks: u16,
    pub clue_sequence_base: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemRecord {
    pub system_id: DurableSystemId,
    pub checked_form_id: CheckedFormId,
    pub members: Vec<HostInstance>,
    pub requirements: Vec<RoleRequirement>,
    pub assignments: Vec<ExactAssignment>,
    pub observed_links: Vec<conduit_core::LinkBindingId>,
    pub boot_scoped_authority: Vec<AuthorityBinding>,
    pub transition_grants: Vec<DelegatedTransitionGrant>,
    pub plan_id: PlanId,
    pub play_ids: Vec<ActivePlayId>,
    pub clue_ids: Vec<conduit_core::ClueId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuityError {
    InvalidSnapshot(String),
    MissingPlan,
    AmbiguousPlan,
    CheckedFormMismatch,
    MissingRole(String),
    AmbiguousRole(String),
    MissingMember(String),
    MissingHostReport(String),
    HostUnavailable(String),
    CapabilityUnavailable(String),
    SelectedRealizationMismatch(String),
    CheckedFaceMismatch(String),
    MissingPlay(String),
    UnknownSubject,
    MissingTransitionGrant,
    TransitionGrantMismatch,
    TransitionGrantExhausted,
    ReplacementHostMismatch,
    ReplacementBootReused,
    ReplacementUnavailable,
    ReplanStillUsesOldPlan,
    ReplanChangedCheckedForm,
    ReplanChangedSystem,
    ReplanChangedRoles,
    ReplanMissingReplacement(String),
    ReplanInheritedStaleAuthority,
    ReplanReusedPlay,
}
