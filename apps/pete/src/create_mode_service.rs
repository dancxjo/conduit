//! Exact authorized Create OI mode service below portable robot meaning.

use crate::{
    transition_oi_mode, CreateOiFailure, CreateOiModeObservation, CreateOiModeRequest,
    CreateOiModeTransitionStage, CreateUartProvider, OiMode,
};
use conduit_core::{BootId, HostId, OfferGeneration};

pub const CREATE_MODE_SERVICE_AUTHORITY: &str = "pete.authority/create1-mode-service@1";
pub const CREATE_MODE_SERVICE_IMPLEMENTATION: &str = "pete/create1-oi-mode-service@1";
pub const CREATE_MODE_SERVICE_ATTACHMENT: &str = "pete.resource/create1-mode-service@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateModeServiceRequest<'a> {
    pub request_id: &'a str,
    pub expected_current_mode: OiMode,
    pub expected_mode_observation_generation: u32,
    pub target_mode: OiMode,
    pub deadline_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateModeServiceBinding<'a> {
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub serial_base_id: &'a str,
    pub robot_identity: &'a str,
    pub robot_identity_verified: bool,
    pub service_attachment_id: &'a str,
    pub current_mode: OiMode,
    pub mode_observation_generation: u32,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateModeServiceAuthority<'a> {
    pub grant_id: &'a str,
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub robot_identity: &'a str,
    pub service_attachment_id: &'a str,
    pub valid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateModeServiceRefusal {
    MissingBindingIdentity,
    UnverifiedRobotIdentity,
    CurrentModeOff,
    InvalidFreshness,
    StaleModeObservation,
    CurrentModeMismatch,
    ModeObservationGenerationMismatch,
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
    ImplementationMismatch,
    RobotIdentityMismatch,
    ServiceAttachmentMismatch,
    Protocol {
        stage: CreateOiModeTransitionStage,
        failure: CreateOiFailure,
    },
    ModeMismatch {
        requested: OiMode,
        observed: OiMode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateModeServiceSign {
    pub request_id: String,
    pub authority_grant_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: String,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub service_attachment_id: String,
    pub prior_mode: OiMode,
    pub prior_mode_observation_generation: u32,
    pub observed_mode: OiMode,
    pub deadline_tick: u64,
}

pub fn transition_create_mode<P: CreateUartProvider>(
    provider: &mut P,
    binding: CreateModeServiceBinding<'_>,
    request: CreateModeServiceRequest<'_>,
    authority: Option<CreateModeServiceAuthority<'_>>,
    now_tick: u64,
) -> Result<CreateModeServiceSign, CreateModeServiceRefusal> {
    validate(&binding, request, authority.as_ref(), now_tick)?;
    let authority = authority.expect("validated mode-service authority");
    let target = match request.target_mode {
        OiMode::Passive => CreateOiModeRequest::Passive,
        OiMode::Safe => CreateOiModeRequest::Safe,
        OiMode::Full => CreateOiModeRequest::Full,
        OiMode::Off => return Err(CreateModeServiceRefusal::UnsupportedTargetMode),
    };
    let observed = transition_oi_mode(provider, target, request.deadline_tick)
        .map(mode_from_observation)
        .map_err(|failure| CreateModeServiceRefusal::Protocol {
            stage: failure.stage,
            failure: failure.failure,
        })?;
    if observed != request.target_mode {
        return Err(CreateModeServiceRefusal::ModeMismatch {
            requested: request.target_mode,
            observed,
        });
    }
    Ok(CreateModeServiceSign {
        request_id: request.request_id.to_string(),
        authority_grant_id: authority.grant_id.to_string(),
        host_id: binding.host_id.clone(),
        boot_id: binding.boot_id.clone(),
        offer_generation: binding.offer_generation,
        implementation_id: binding.implementation_id.to_string(),
        serial_base_id: binding.serial_base_id.to_string(),
        robot_identity: binding.robot_identity.to_string(),
        service_attachment_id: binding.service_attachment_id.to_string(),
        prior_mode: binding.current_mode,
        prior_mode_observation_generation: binding.mode_observation_generation,
        observed_mode: observed,
        deadline_tick: request.deadline_tick,
    })
}

fn validate(
    binding: &CreateModeServiceBinding<'_>,
    request: CreateModeServiceRequest<'_>,
    authority: Option<&CreateModeServiceAuthority<'_>>,
    now_tick: u64,
) -> Result<(), CreateModeServiceRefusal> {
    if binding.implementation_id.is_empty()
        || binding.serial_base_id.is_empty()
        || binding.robot_identity.is_empty()
        || binding.service_attachment_id.is_empty()
    {
        return Err(CreateModeServiceRefusal::MissingBindingIdentity);
    }
    if !binding.robot_identity_verified {
        return Err(CreateModeServiceRefusal::UnverifiedRobotIdentity);
    }
    if binding.current_mode == OiMode::Off {
        return Err(CreateModeServiceRefusal::CurrentModeOff);
    }
    if binding.mode_observation_generation == 0 || binding.maximum_age_ticks == 0 {
        return Err(CreateModeServiceRefusal::InvalidFreshness);
    }
    if now_tick.saturating_sub(binding.observed_at_tick) > u64::from(binding.maximum_age_ticks) {
        return Err(CreateModeServiceRefusal::StaleModeObservation);
    }
    if request.expected_current_mode != binding.current_mode {
        return Err(CreateModeServiceRefusal::CurrentModeMismatch);
    }
    if request.expected_mode_observation_generation != binding.mode_observation_generation {
        return Err(CreateModeServiceRefusal::ModeObservationGenerationMismatch);
    }
    if request.target_mode == OiMode::Off {
        return Err(CreateModeServiceRefusal::UnsupportedTargetMode);
    }
    if request.request_id.is_empty() {
        return Err(CreateModeServiceRefusal::MissingRequestIdentity);
    }
    if request.deadline_tick <= now_tick {
        return Err(CreateModeServiceRefusal::InvalidDeadline);
    }
    let authority = authority.ok_or(CreateModeServiceRefusal::MissingAuthority)?;
    if authority.grant_id != CREATE_MODE_SERVICE_AUTHORITY {
        return Err(CreateModeServiceRefusal::WrongAuthority);
    }
    if authority.valid_until_tick <= now_tick {
        return Err(CreateModeServiceRefusal::AuthorityExpired);
    }
    if authority.valid_until_tick < request.deadline_tick {
        return Err(CreateModeServiceRefusal::OperationOutlivesAuthority);
    }
    if authority.host_id != binding.host_id {
        return Err(CreateModeServiceRefusal::HostMismatch);
    }
    if authority.boot_id != binding.boot_id {
        return Err(CreateModeServiceRefusal::BootMismatch);
    }
    if authority.offer_generation != binding.offer_generation {
        return Err(CreateModeServiceRefusal::OfferGenerationMismatch);
    }
    if authority.implementation_id != binding.implementation_id {
        return Err(CreateModeServiceRefusal::ImplementationMismatch);
    }
    if authority.robot_identity != binding.robot_identity {
        return Err(CreateModeServiceRefusal::RobotIdentityMismatch);
    }
    if authority.service_attachment_id != binding.service_attachment_id {
        return Err(CreateModeServiceRefusal::ServiceAttachmentMismatch);
    }
    Ok(())
}

const fn mode_from_observation(mode: CreateOiModeObservation) -> OiMode {
    match mode {
        CreateOiModeObservation::Off => OiMode::Off,
        CreateOiModeObservation::Passive => OiMode::Passive,
        CreateOiModeObservation::Safe => OiMode::Safe,
        CreateOiModeObservation::Full => OiMode::Full,
    }
}

#[cfg(test)]
#[path = "create_mode_service_tests.rs"]
mod tests;
