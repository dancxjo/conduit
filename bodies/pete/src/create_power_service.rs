//! State-aware Create power-toggle service over an exact translated attachment.

use crate::{
    CreatePowerPulseFailure, CreatePowerPulseProfile, CreatePowerPulseProgress, CreatePowerToggle,
    CreatePowerToggleProvider,
};
use conduit_core::{BootId, HostId, OfferGeneration};

pub const CREATE_POWER_SERVICE_AUTHORITY: &str = "pete.authority/create1-power-toggle@1";
pub const CREATE_POWER_SERVICE_IMPLEMENTATION: &str = "pete/create1-power-toggle-service@1";
pub const CREATE_POWER_SERVICE_ATTACHMENT: &str = "pete.resource/create1-translated-power-toggle@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatePowerState {
    Unknown,
    Off,
    On,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreatePowerObservation {
    pub state: CreatePowerState,
    pub generation: u32,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePowerVerification<'a> {
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub robot_identity: &'a str,
    pub attachment_id: &'a str,
    pub observation: CreatePowerObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreatePowerServiceRequest<'a> {
    pub request_id: &'a str,
    pub target: CreatePowerState,
    pub expected_observation_generation: u32,
    pub expected_safe_disposition_generation: u32,
    pub deadline_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePowerServiceBinding<'a> {
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub robot_identity: &'a str,
    pub attachment_id: &'a str,
    pub translation_path_verified: bool,
    pub translator_enabled: bool,
    pub output_idle_low_observed: bool,
    pub direct_untranslated_connection: bool,
    pub motion_active: bool,
    pub safe_disposition_generation: u32,
    pub power: CreatePowerObservation,
    pub pulse_profile: CreatePowerPulseProfile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePowerServiceAuthority<'a> {
    pub grant_id: &'a str,
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub robot_identity: &'a str,
    pub attachment_id: &'a str,
    pub valid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatePowerServiceRefusal {
    MissingIdentity,
    UnsafeElectricalAttachment,
    TranslatorUnavailable,
    OutputNotObservedIdleLow,
    MotionActive,
    MissingSafeDisposition,
    UnknownPower,
    UnsupportedTarget,
    InvalidPowerFreshness,
    StalePowerObservation,
    PowerObservationGenerationMismatch,
    SafeDispositionGenerationMismatch,
    MissingRequestIdentity,
    InvalidDeadline,
    OperationExceedsDeadline,
    MissingAuthority,
    WrongAuthority,
    AuthorityExpired,
    OperationOutlivesAuthority,
    HostMismatch,
    BootMismatch,
    OfferGenerationMismatch,
    ImplementationMismatch,
    RobotIdentityMismatch,
    AttachmentMismatch,
    Pulse(CreatePowerPulseFailure),
    InvalidServiceState,
    DeadlineExpired,
    VerificationGenerationDidNotAdvance,
    VerificationStale,
    VerificationMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePowerServiceSign {
    pub request_id: String,
    pub authority_grant_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: String,
    pub robot_identity: String,
    pub attachment_id: String,
    pub prior_state: CreatePowerState,
    pub observed_state: CreatePowerState,
    pub prior_observation_generation: u32,
    pub observed_generation: u32,
    pub safe_disposition_generation: u32,
    pub pulse_emitted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatePowerServiceProgress {
    WaitingLowSettle { raise_at_tick: u64 },
    WaitingHighPulse { lower_at_tick: u64 },
    AwaitingFreshVerification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreatePowerServicePhase {
    Pulsing,
    AwaitingVerification,
    Failed,
    Completed,
}

pub struct PreparedCreatePowerService {
    toggle: CreatePowerToggle,
    phase: CreatePowerServicePhase,
    request_id: String,
    authority_grant_id: String,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    implementation_id: String,
    robot_identity: String,
    attachment_id: String,
    prior_state: CreatePowerState,
    target: CreatePowerState,
    prior_observation_generation: u32,
    safe_disposition_generation: u32,
    deadline_tick: u64,
}

pub enum CreatePowerServiceStart {
    NoOp(CreatePowerServiceSign),
    Pulsing {
        execution: PreparedCreatePowerService,
        progress: CreatePowerServiceProgress,
    },
}

pub fn start_create_power_service<P: CreatePowerToggleProvider>(
    provider: &mut P,
    binding: CreatePowerServiceBinding<'_>,
    request: CreatePowerServiceRequest<'_>,
    authority: Option<CreatePowerServiceAuthority<'_>>,
    now_tick: u64,
) -> Result<CreatePowerServiceStart, CreatePowerServiceRefusal> {
    validate(&binding, request, authority.as_ref(), now_tick)?;
    let authority = authority.expect("validated Create power authority");
    if binding.power.state == request.target {
        return Ok(CreatePowerServiceStart::NoOp(sign(
            &binding,
            request,
            authority.grant_id,
            binding.power,
            false,
        )));
    }
    let mut toggle = CreatePowerToggle::new(binding.pulse_profile);
    let progress = toggle
        .start(provider, now_tick)
        .map_err(CreatePowerServiceRefusal::Pulse)?;
    Ok(CreatePowerServiceStart::Pulsing {
        execution: PreparedCreatePowerService {
            toggle,
            phase: CreatePowerServicePhase::Pulsing,
            request_id: request.request_id.to_string(),
            authority_grant_id: authority.grant_id.to_string(),
            host_id: binding.host_id.clone(),
            boot_id: binding.boot_id.clone(),
            offer_generation: binding.offer_generation,
            implementation_id: binding.implementation_id.to_string(),
            robot_identity: binding.robot_identity.to_string(),
            attachment_id: binding.attachment_id.to_string(),
            prior_state: binding.power.state,
            target: request.target,
            prior_observation_generation: binding.power.generation,
            safe_disposition_generation: binding.safe_disposition_generation,
            deadline_tick: request.deadline_tick,
        },
        progress: map_progress(progress),
    })
}

pub fn advance_create_power_service<P: CreatePowerToggleProvider>(
    execution: &mut PreparedCreatePowerService,
    provider: &mut P,
    now_tick: u64,
) -> Result<CreatePowerServiceProgress, CreatePowerServiceRefusal> {
    if execution.phase != CreatePowerServicePhase::Pulsing || now_tick > execution.deadline_tick {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(if now_tick > execution.deadline_tick {
            CreatePowerServiceRefusal::DeadlineExpired
        } else {
            CreatePowerServiceRefusal::InvalidServiceState
        });
    }
    match execution.toggle.advance(provider, now_tick) {
        Ok(CreatePowerPulseProgress::CompletedLow) => {
            execution.phase = CreatePowerServicePhase::AwaitingVerification;
            Ok(CreatePowerServiceProgress::AwaitingFreshVerification)
        }
        Ok(progress) => Ok(map_progress(progress)),
        Err(failure) => {
            execution.phase = CreatePowerServicePhase::Failed;
            Err(CreatePowerServiceRefusal::Pulse(failure))
        }
    }
}

pub fn verify_create_power_service(
    execution: &mut PreparedCreatePowerService,
    verification: CreatePowerVerification<'_>,
    now_tick: u64,
) -> Result<CreatePowerServiceSign, CreatePowerServiceRefusal> {
    if execution.phase != CreatePowerServicePhase::AwaitingVerification {
        return Err(CreatePowerServiceRefusal::InvalidServiceState);
    }
    if now_tick > execution.deadline_tick {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::DeadlineExpired);
    }
    if verification.host_id != &execution.host_id {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::HostMismatch);
    }
    if verification.boot_id != &execution.boot_id {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::BootMismatch);
    }
    if verification.offer_generation != execution.offer_generation {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::OfferGenerationMismatch);
    }
    if verification.implementation_id != execution.implementation_id {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::ImplementationMismatch);
    }
    if verification.robot_identity != execution.robot_identity {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::RobotIdentityMismatch);
    }
    if verification.attachment_id != execution.attachment_id {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::AttachmentMismatch);
    }
    let observation = verification.observation;
    if observation.generation <= execution.prior_observation_generation {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::VerificationGenerationDidNotAdvance);
    }
    if observation.maximum_age_ticks == 0
        || now_tick.saturating_sub(observation.observed_at_tick)
            > u64::from(observation.maximum_age_ticks)
    {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::VerificationStale);
    }
    if observation.state != execution.target {
        execution.phase = CreatePowerServicePhase::Failed;
        return Err(CreatePowerServiceRefusal::VerificationMismatch);
    }
    execution.phase = CreatePowerServicePhase::Completed;
    Ok(CreatePowerServiceSign {
        request_id: execution.request_id.clone(),
        authority_grant_id: execution.authority_grant_id.clone(),
        host_id: execution.host_id.clone(),
        boot_id: execution.boot_id.clone(),
        offer_generation: execution.offer_generation,
        implementation_id: execution.implementation_id.clone(),
        robot_identity: execution.robot_identity.clone(),
        attachment_id: execution.attachment_id.clone(),
        prior_state: execution.prior_state,
        observed_state: observation.state,
        prior_observation_generation: execution.prior_observation_generation,
        observed_generation: observation.generation,
        safe_disposition_generation: execution.safe_disposition_generation,
        pulse_emitted: true,
    })
}

fn validate(
    binding: &CreatePowerServiceBinding<'_>,
    request: CreatePowerServiceRequest<'_>,
    authority: Option<&CreatePowerServiceAuthority<'_>>,
    now_tick: u64,
) -> Result<(), CreatePowerServiceRefusal> {
    if binding.implementation_id.is_empty()
        || binding.robot_identity.is_empty()
        || binding.attachment_id.is_empty()
    {
        return Err(CreatePowerServiceRefusal::MissingIdentity);
    }
    if !binding.translation_path_verified || binding.direct_untranslated_connection {
        return Err(CreatePowerServiceRefusal::UnsafeElectricalAttachment);
    }
    if !binding.translator_enabled {
        return Err(CreatePowerServiceRefusal::TranslatorUnavailable);
    }
    if !binding.output_idle_low_observed {
        return Err(CreatePowerServiceRefusal::OutputNotObservedIdleLow);
    }
    if binding.motion_active {
        return Err(CreatePowerServiceRefusal::MotionActive);
    }
    if binding.safe_disposition_generation == 0 {
        return Err(CreatePowerServiceRefusal::MissingSafeDisposition);
    }
    if binding.power.state == CreatePowerState::Unknown {
        return Err(CreatePowerServiceRefusal::UnknownPower);
    }
    if request.target == CreatePowerState::Unknown {
        return Err(CreatePowerServiceRefusal::UnsupportedTarget);
    }
    if binding.power.generation == 0 || binding.power.maximum_age_ticks == 0 {
        return Err(CreatePowerServiceRefusal::InvalidPowerFreshness);
    }
    if now_tick.saturating_sub(binding.power.observed_at_tick)
        > u64::from(binding.power.maximum_age_ticks)
    {
        return Err(CreatePowerServiceRefusal::StalePowerObservation);
    }
    if request.expected_observation_generation != binding.power.generation {
        return Err(CreatePowerServiceRefusal::PowerObservationGenerationMismatch);
    }
    if request.expected_safe_disposition_generation != binding.safe_disposition_generation {
        return Err(CreatePowerServiceRefusal::SafeDispositionGenerationMismatch);
    }
    if request.request_id.is_empty() {
        return Err(CreatePowerServiceRefusal::MissingRequestIdentity);
    }
    if request.deadline_tick <= now_tick {
        return Err(CreatePowerServiceRefusal::InvalidDeadline);
    }
    let pulse_end = now_tick
        .checked_add(u64::from(binding.pulse_profile.low_settle_ticks))
        .and_then(|tick| tick.checked_add(u64::from(binding.pulse_profile.high_pulse_ticks)))
        .ok_or(CreatePowerServiceRefusal::OperationExceedsDeadline)?;
    if pulse_end >= request.deadline_tick {
        return Err(CreatePowerServiceRefusal::OperationExceedsDeadline);
    }
    let authority = authority.ok_or(CreatePowerServiceRefusal::MissingAuthority)?;
    if authority.grant_id != CREATE_POWER_SERVICE_AUTHORITY {
        return Err(CreatePowerServiceRefusal::WrongAuthority);
    }
    if authority.valid_until_tick <= now_tick {
        return Err(CreatePowerServiceRefusal::AuthorityExpired);
    }
    if authority.valid_until_tick < request.deadline_tick {
        return Err(CreatePowerServiceRefusal::OperationOutlivesAuthority);
    }
    if authority.host_id != binding.host_id {
        return Err(CreatePowerServiceRefusal::HostMismatch);
    }
    if authority.boot_id != binding.boot_id {
        return Err(CreatePowerServiceRefusal::BootMismatch);
    }
    if authority.offer_generation != binding.offer_generation {
        return Err(CreatePowerServiceRefusal::OfferGenerationMismatch);
    }
    if authority.implementation_id != binding.implementation_id {
        return Err(CreatePowerServiceRefusal::ImplementationMismatch);
    }
    if authority.robot_identity != binding.robot_identity {
        return Err(CreatePowerServiceRefusal::RobotIdentityMismatch);
    }
    if authority.attachment_id != binding.attachment_id {
        return Err(CreatePowerServiceRefusal::AttachmentMismatch);
    }
    Ok(())
}

fn sign(
    binding: &CreatePowerServiceBinding<'_>,
    request: CreatePowerServiceRequest<'_>,
    authority_grant_id: &str,
    observation: CreatePowerObservation,
    pulse_emitted: bool,
) -> CreatePowerServiceSign {
    CreatePowerServiceSign {
        request_id: request.request_id.to_string(),
        authority_grant_id: authority_grant_id.to_string(),
        host_id: binding.host_id.clone(),
        boot_id: binding.boot_id.clone(),
        offer_generation: binding.offer_generation,
        implementation_id: binding.implementation_id.to_string(),
        robot_identity: binding.robot_identity.to_string(),
        attachment_id: binding.attachment_id.to_string(),
        prior_state: observation.state,
        observed_state: observation.state,
        prior_observation_generation: observation.generation,
        observed_generation: observation.generation,
        safe_disposition_generation: binding.safe_disposition_generation,
        pulse_emitted,
    }
}

const fn map_progress(progress: CreatePowerPulseProgress) -> CreatePowerServiceProgress {
    match progress {
        CreatePowerPulseProgress::WaitingLowSettle { raise_at_tick } => {
            CreatePowerServiceProgress::WaitingLowSettle { raise_at_tick }
        }
        CreatePowerPulseProgress::WaitingHighPulse { lower_at_tick } => {
            CreatePowerServiceProgress::WaitingHighPulse { lower_at_tick }
        }
        CreatePowerPulseProgress::CompletedLow => {
            CreatePowerServiceProgress::AwaitingFreshVerification
        }
    }
}

#[cfg(test)]
#[path = "create_power_service_tests.rs"]
mod tests;
