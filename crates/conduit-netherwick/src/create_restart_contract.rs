//! Public finite contract for the attended Create restart service.

use crate::{CreateModeServiceRequest, CreatePowerServiceRequest, CreatePowerState, OiMode};
use conduit_core::{BootId, HostId, OfferGeneration};

pub const CREATE_RESTART_SERVICE_AUTHORITY: &str = "netherwick.authority/create1-restart-service@1";
pub const CREATE_RESTART_SERVICE_IMPLEMENTATION: &str = "netherwick/create1-restart-service@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRestartBinding<'a> {
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub robot_identity: &'a str,
    pub power_implementation_id: &'a str,
    pub power_attachment_id: &'a str,
    pub mode_implementation_id: &'a str,
    pub serial_base_id: &'a str,
    pub mode_attachment_id: &'a str,
    pub safe_disposition_generation: u32,
    pub power_state: CreatePowerState,
    pub power_observation_generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateRestartRequest<'a> {
    pub request_id: &'a str,
    pub target_mode: OiMode,
    pub deadline_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRestartAuthority<'a> {
    pub grant_id: &'a str,
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub robot_identity: &'a str,
    pub valid_until_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRestartModeObservation<'a> {
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub serial_base_id: &'a str,
    pub robot_identity: &'a str,
    pub service_attachment_id: &'a str,
    pub mode: OiMode,
    pub generation: u32,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRestartPowerStage {
    pub request_id: String,
    pub target: CreatePowerState,
    pub expected_observation_generation: u32,
    pub expected_safe_disposition_generation: u32,
    pub deadline_tick: u64,
}

impl CreateRestartPowerStage {
    pub fn request(&self) -> CreatePowerServiceRequest<'_> {
        CreatePowerServiceRequest {
            request_id: &self.request_id,
            target: self.target,
            expected_observation_generation: self.expected_observation_generation,
            expected_safe_disposition_generation: self.expected_safe_disposition_generation,
            deadline_tick: self.deadline_tick,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRestartModeStage {
    pub request_id: String,
    pub expected_current_mode: OiMode,
    pub expected_mode_observation_generation: u32,
    pub target_mode: OiMode,
    pub deadline_tick: u64,
}

impl CreateRestartModeStage {
    pub fn request(&self) -> CreateModeServiceRequest<'_> {
        CreateModeServiceRequest {
            request_id: &self.request_id,
            expected_current_mode: self.expected_current_mode,
            expected_mode_observation_generation: self.expected_mode_observation_generation,
            target_mode: self.target_mode,
            deadline_tick: self.deadline_tick,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateRestartAction {
    AwaitSafeDisposition { expected_generation: u32 },
    PowerOff(CreateRestartPowerStage),
    PowerOn(CreateRestartPowerStage),
    AwaitFreshModeObservation,
    RestoreMode(CreateRestartModeStage),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateRestartRefusal {
    MissingIdentity,
    UnknownInitialPower,
    MissingSafeDisposition,
    UnsupportedTargetMode,
    MissingRequestIdentity,
    InvalidDeadline,
    MissingAuthority,
    WrongAuthority,
    AuthorityExpired,
    OperationOutlivesAuthority,
    HostMismatch,
    BootMismatch,
    OfferGenerationMismatch,
    RobotIdentityMismatch,
    WrongStage,
    DeadlineExpired,
    StopFailed,
    UnexpectedStopCause,
    SafeDispositionGenerationMismatch,
    DownstreamAuthorityMismatch,
    ImplementationMismatch,
    AttachmentMismatch,
    RequestMismatch,
    PowerPriorStateMismatch,
    PowerStateMismatch,
    PowerGenerationMismatch,
    RequiredPulseMissing,
    InvalidModeFreshness,
    StaleModeObservation,
    ModeOff,
    ModeMismatch,
    ModeGenerationMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRestartSign {
    pub request_id: String,
    pub authority_grant_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub robot_identity: String,
    pub safe_disposition_generation: u32,
    pub power_off_generation: u32,
    pub power_on_generation: u32,
    pub observed_mode: OiMode,
    pub deadline_tick: u64,
}
