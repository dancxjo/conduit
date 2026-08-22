//! Exact service boundary for the non-author-wirable Create safety envelope.

use crate::{
    LocalHazard, LocalSafetyEnvelope, SafetyEnvelopeRefusal, SafetyEnvelopeSign, SafetyHazardSet,
};
use conduit_core::{BootId, HostId, OfferGeneration};

pub const CREATE_SAFETY_CLEAR_AUTHORITY: &str = "pete.authority/create1-safety-clear@1";
pub const CREATE_SAFETY_ENVELOPE_IMPLEMENTATION: &str = "pete/create1-local-safety-envelope@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateSafetyServiceBinding<'a> {
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub robot_identity: &'a str,
    pub envelope_id: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateEmergencyStopRequest<'a> {
    pub request_id: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateSafetyClearRequest<'a> {
    pub request_id: &'a str,
    pub hazard: LocalHazard,
    pub expected_latch_generation: u32,
    pub expected_observation_generation: u32,
    pub deadline_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateSafetyClearAuthority<'a> {
    pub grant_id: &'a str,
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: &'a str,
    pub robot_identity: &'a str,
    pub envelope_id: &'a str,
    pub valid_until_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateSafetyServiceRefusal {
    MissingBindingIdentity,
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
    EnvelopeMismatch,
    Envelope(SafetyEnvelopeRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateEmergencyStopSign {
    pub request_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: String,
    pub robot_identity: String,
    pub envelope_id: String,
    pub observed_at_tick: u64,
    pub latch_generation: u32,
    pub latched_hazards: SafetyHazardSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateSafetyClearSign {
    pub request_id: String,
    pub authority_grant_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub implementation_id: String,
    pub robot_identity: String,
    pub envelope_id: String,
    pub hazard: LocalHazard,
    pub observation_generation: u32,
    pub prior_latch_generation: u32,
    pub latch_generation: u32,
    pub remaining_hazards: SafetyHazardSet,
}

pub fn assert_create_emergency_stop(
    envelope: &mut LocalSafetyEnvelope,
    binding: CreateSafetyServiceBinding<'_>,
    request: CreateEmergencyStopRequest<'_>,
    now_tick: u64,
) -> Result<CreateEmergencyStopSign, CreateSafetyServiceRefusal> {
    validate_binding(&binding)?;
    if request.request_id.is_empty() {
        return Err(CreateSafetyServiceRefusal::MissingRequestIdentity);
    }
    let SafetyEnvelopeSign::EmergencyStopLatched {
        latch_generation,
        latched,
    } = envelope
        .assert_emergency_stop()
        .map_err(CreateSafetyServiceRefusal::Envelope)?
    else {
        unreachable!("emergency-stop assertion has one exact Sign")
    };
    Ok(CreateEmergencyStopSign {
        request_id: request.request_id.to_string(),
        host_id: binding.host_id.clone(),
        boot_id: binding.boot_id.clone(),
        offer_generation: binding.offer_generation,
        implementation_id: binding.implementation_id.to_string(),
        robot_identity: binding.robot_identity.to_string(),
        envelope_id: binding.envelope_id.to_string(),
        observed_at_tick: now_tick,
        latch_generation,
        latched_hazards: latched,
    })
}

pub fn clear_create_safety_latch(
    envelope: &mut LocalSafetyEnvelope,
    binding: CreateSafetyServiceBinding<'_>,
    request: CreateSafetyClearRequest<'_>,
    authority: Option<CreateSafetyClearAuthority<'_>>,
    now_tick: u64,
) -> Result<CreateSafetyClearSign, CreateSafetyServiceRefusal> {
    validate_clear(&binding, request, authority.as_ref(), now_tick)?;
    let authority = authority.expect("validated safety-clear authority");
    let SafetyEnvelopeSign::Cleared {
        hazard,
        observation_generation,
        prior_latch_generation,
        latch_generation,
        remaining,
    } = envelope
        .clear(
            request.hazard,
            request.expected_latch_generation,
            request.expected_observation_generation,
            now_tick,
        )
        .map_err(CreateSafetyServiceRefusal::Envelope)?
    else {
        unreachable!("safety clear has one exact Sign")
    };
    Ok(CreateSafetyClearSign {
        request_id: request.request_id.to_string(),
        authority_grant_id: authority.grant_id.to_string(),
        host_id: binding.host_id.clone(),
        boot_id: binding.boot_id.clone(),
        offer_generation: binding.offer_generation,
        implementation_id: binding.implementation_id.to_string(),
        robot_identity: binding.robot_identity.to_string(),
        envelope_id: binding.envelope_id.to_string(),
        hazard,
        observation_generation,
        prior_latch_generation,
        latch_generation,
        remaining_hazards: remaining,
    })
}

fn validate_binding(
    binding: &CreateSafetyServiceBinding<'_>,
) -> Result<(), CreateSafetyServiceRefusal> {
    if binding.implementation_id.is_empty()
        || binding.robot_identity.is_empty()
        || binding.envelope_id.is_empty()
    {
        return Err(CreateSafetyServiceRefusal::MissingBindingIdentity);
    }
    Ok(())
}

fn validate_clear(
    binding: &CreateSafetyServiceBinding<'_>,
    request: CreateSafetyClearRequest<'_>,
    authority: Option<&CreateSafetyClearAuthority<'_>>,
    now_tick: u64,
) -> Result<(), CreateSafetyServiceRefusal> {
    validate_binding(binding)?;
    if request.request_id.is_empty() {
        return Err(CreateSafetyServiceRefusal::MissingRequestIdentity);
    }
    if request.deadline_tick <= now_tick {
        return Err(CreateSafetyServiceRefusal::InvalidDeadline);
    }
    let authority = authority.ok_or(CreateSafetyServiceRefusal::MissingAuthority)?;
    if authority.grant_id != CREATE_SAFETY_CLEAR_AUTHORITY {
        return Err(CreateSafetyServiceRefusal::WrongAuthority);
    }
    if authority.valid_until_tick <= now_tick {
        return Err(CreateSafetyServiceRefusal::AuthorityExpired);
    }
    if authority.valid_until_tick < request.deadline_tick {
        return Err(CreateSafetyServiceRefusal::OperationOutlivesAuthority);
    }
    if authority.host_id != binding.host_id {
        return Err(CreateSafetyServiceRefusal::HostMismatch);
    }
    if authority.boot_id != binding.boot_id {
        return Err(CreateSafetyServiceRefusal::BootMismatch);
    }
    if authority.offer_generation != binding.offer_generation {
        return Err(CreateSafetyServiceRefusal::OfferGenerationMismatch);
    }
    if authority.implementation_id != binding.implementation_id {
        return Err(CreateSafetyServiceRefusal::ImplementationMismatch);
    }
    if authority.robot_identity != binding.robot_identity {
        return Err(CreateSafetyServiceRefusal::RobotIdentityMismatch);
    }
    if authority.envelope_id != binding.envelope_id {
        return Err(CreateSafetyServiceRefusal::EnvelopeMismatch);
    }
    Ok(())
}

#[cfg(test)]
#[path = "create_safety_service_tests.rs"]
mod tests;
