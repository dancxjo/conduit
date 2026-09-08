//! Generic last-mile gate for consequential physical effects.

use crate::{BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim};
use alloc::string::String;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsequentialEnvelope {
    pub maximum_magnitude: u32,
    pub maximum_duration_ticks: u32,
    pub maximum_rate: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsequentialProfile {
    pub profile_id: String,
    pub requires_attendance: bool,
    pub envelope: ConsequentialEnvelope,
    pub safe_disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsequentialRequest {
    pub resource_id: String,
    pub resource_generation: u64,
    pub operation_id: String,
    pub magnitude: u32,
    pub duration_ticks: u32,
    pub rate: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafetyReadiness {
    pub resource_id: String,
    pub resource_generation: u64,
    pub observation_generation: u64,
    pub ready: bool,
}

/// Opaque possession issued by an independent attended-authority boundary.
pub struct AttendanceHandle {
    bearer: [u8; 32],
}

pub struct AttendanceGrant {
    pub grant_id: String,
    pub resource_id: String,
    pub resource_generation: u64,
    pub operation_id: String,
    pub valid_from_tick: u64,
    pub valid_until_tick: u64,
    bearer: [u8; 32],
    consumed: bool,
}

impl AttendanceGrant {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        issuer_key: [u8; 32],
        nonce: u64,
        grant_id: String,
        resource_id: String,
        resource_generation: u64,
        operation_id: String,
        valid_from_tick: u64,
        valid_until_tick: u64,
    ) -> Result<(Self, AttendanceHandle), ConsequentialRefusal> {
        if issuer_key == [0; 32]
            || nonce == 0
            || grant_id.is_empty()
            || resource_id.is_empty()
            || resource_generation == 0
            || operation_id.is_empty()
            || valid_from_tick >= valid_until_tick
        {
            return Err(ConsequentialRefusal::InvalidProfile);
        }
        let mut digest = Sha256::new();
        digest.update(issuer_key);
        digest.update(nonce.to_le_bytes());
        digest.update(grant_id.as_bytes());
        digest.update(resource_id.as_bytes());
        digest.update(resource_generation.to_le_bytes());
        digest.update(operation_id.as_bytes());
        digest.update(valid_from_tick.to_le_bytes());
        digest.update(valid_until_tick.to_le_bytes());
        let bearer = digest.finalize().into();
        Ok((
            Self {
                grant_id,
                resource_id,
                resource_generation,
                operation_id,
                valid_from_tick,
                valid_until_tick,
                bearer,
                consumed: false,
            },
            AttendanceHandle { bearer },
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalObservation {
    Observed,
    Uncertain,
}

pub trait ConsequentialEffectProvider {
    fn apply(
        &mut self,
        request: &ConsequentialRequest,
    ) -> Result<PhysicalObservation, ConsequentialProviderFailure>;
    fn safe_disposition(&mut self, disposition: &str) -> Result<(), ConsequentialProviderFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsequentialProviderFailure {
    Refused,
    Ambiguous,
    Lost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsequentialRefusal {
    InvalidProfile,
    MissingAttendance,
    AttendanceExpired,
    AttendanceScope,
    AttendanceReplay,
    ForgedAttendance,
    ResourceStale,
    SafetyNotReady,
    EnvelopeExceeded,
    Capability(BaseCapabilityRefusal),
    Provider(ConsequentialProviderFailure),
    PhysicalOutcomeUncertain,
    SafeDispositionFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsequentialDisposition {
    PhysicalEffectObserved,
    Refused(ConsequentialRefusal),
    Safe,
}

pub struct ConsequentialEffectGate {
    pub profile: ConsequentialProfile,
    pub capability_table: BaseCapabilityTable,
    pub capability_handle: BaseCapabilityHandle,
    pub capability_claim: BaseOperationClaim,
    pub attendance: Option<AttendanceGrant>,
    pub resource_id: String,
    pub resource_generation: u64,
    pub minimum_safety_generation: u64,
    terminal: bool,
}

impl ConsequentialEffectGate {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        profile: ConsequentialProfile,
        capability_table: BaseCapabilityTable,
        capability_handle: BaseCapabilityHandle,
        capability_claim: BaseOperationClaim,
        attendance: Option<AttendanceGrant>,
        resource_id: String,
        resource_generation: u64,
        minimum_safety_generation: u64,
    ) -> Result<Self, ConsequentialRefusal> {
        if profile.profile_id.is_empty()
            || profile.safe_disposition.is_empty()
            || profile.envelope.maximum_duration_ticks == 0
            || profile.envelope.maximum_rate == 0
            || resource_id.is_empty()
            || resource_generation == 0
            || minimum_safety_generation == 0
            || (profile.requires_attendance && attendance.is_none())
        {
            return Err(ConsequentialRefusal::InvalidProfile);
        }
        Ok(Self {
            profile,
            capability_table,
            capability_handle,
            capability_claim,
            attendance,
            resource_id,
            resource_generation,
            minimum_safety_generation,
            terminal: false,
        })
    }

    pub fn attempt<P: ConsequentialEffectProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
        attendance_handle: Option<&AttendanceHandle>,
        safety: &SafetyReadiness,
        request: &ConsequentialRequest,
    ) -> ConsequentialDisposition {
        if self.terminal {
            return ConsequentialDisposition::Refused(ConsequentialRefusal::AttendanceReplay);
        }
        if request.resource_id != self.resource_id
            || request.resource_generation != self.resource_generation
            || safety.resource_id != self.resource_id
            || safety.resource_generation != self.resource_generation
        {
            return ConsequentialDisposition::Refused(ConsequentialRefusal::ResourceStale);
        }
        if !safety.ready || safety.observation_generation < self.minimum_safety_generation {
            return ConsequentialDisposition::Refused(ConsequentialRefusal::SafetyNotReady);
        }
        let envelope = self.profile.envelope;
        if request.magnitude > envelope.maximum_magnitude
            || request.duration_ticks > envelope.maximum_duration_ticks
            || request.rate > envelope.maximum_rate
        {
            return ConsequentialDisposition::Refused(ConsequentialRefusal::EnvelopeExceeded);
        }
        if self.profile.requires_attendance {
            let Some(handle) = attendance_handle else {
                return ConsequentialDisposition::Refused(ConsequentialRefusal::MissingAttendance);
            };
            let Some(grant) = self.attendance.as_mut() else {
                return ConsequentialDisposition::Refused(ConsequentialRefusal::MissingAttendance);
            };
            if grant.consumed {
                return ConsequentialDisposition::Refused(ConsequentialRefusal::AttendanceReplay);
            }
            if !bearer_matches(&grant.bearer, &handle.bearer) {
                return ConsequentialDisposition::Refused(ConsequentialRefusal::ForgedAttendance);
            }
            if now_tick < grant.valid_from_tick || now_tick >= grant.valid_until_tick {
                return ConsequentialDisposition::Refused(ConsequentialRefusal::AttendanceExpired);
            }
            if grant.resource_id != request.resource_id
                || grant.resource_generation != request.resource_generation
                || grant.operation_id != request.operation_id
            {
                return ConsequentialDisposition::Refused(ConsequentialRefusal::AttendanceScope);
            }
            grant.consumed = true;
        }
        self.capability_claim.parameter_bytes = 24;
        let lease = match self
            .capability_table
            .authorize(&self.capability_handle, &self.capability_claim)
        {
            Ok(lease) => lease,
            Err(refusal) => {
                return ConsequentialDisposition::Refused(ConsequentialRefusal::Capability(
                    refusal,
                ));
            }
        };
        self.terminal = true;
        match provider.apply(request) {
            Ok(PhysicalObservation::Observed) => match self.capability_table.complete(lease, 0) {
                Ok(()) => ConsequentialDisposition::PhysicalEffectObserved,
                Err(refusal) => {
                    ConsequentialDisposition::Refused(ConsequentialRefusal::Capability(refusal))
                }
            },
            Ok(PhysicalObservation::Uncertain) => {
                ConsequentialDisposition::Refused(ConsequentialRefusal::PhysicalOutcomeUncertain)
            }
            Err(failure) => {
                ConsequentialDisposition::Refused(ConsequentialRefusal::Provider(failure))
            }
        }
    }

    pub fn lose<P: ConsequentialEffectProvider>(
        &mut self,
        provider: &mut P,
    ) -> ConsequentialDisposition {
        self.terminal = true;
        let _ = self.capability_table.revoke(&self.capability_handle);
        match provider.safe_disposition(&self.profile.safe_disposition) {
            Ok(()) => ConsequentialDisposition::Safe,
            Err(_) => {
                ConsequentialDisposition::Refused(ConsequentialRefusal::SafeDispositionFailed)
            }
        }
    }
}

fn bearer_matches(expected: &[u8; 32], supplied: &[u8; 32]) -> bool {
    expected
        .iter()
        .zip(supplied)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}
