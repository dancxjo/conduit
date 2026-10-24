//! Bounded, allocation-free validation for one host-assigned Plan projection.

#![no_std]
#[cfg(test)]
extern crate std;
#[cfg(test)]
mod fore_tests;
#[cfg(test)]
mod record_view_tests;

mod computation;
mod execution;
mod fore;
pub use fore::*;
mod native;
mod records;
pub use computation::{AssignedComputationProfile, AssignedComputationRequirement};
pub use native::*;
pub use records::{AssignedPlanRecords, AssignedRecord, AssignedRecordRequirement};
mod sha256;
mod single_source;
mod validation;
use validation::decode;
pub use validation::decode_assigned_records_with_computations;

pub use execution::*;
pub use single_source::*;

pub const ASSIGNED_PLAN_SCHEMA: u16 = 4;
/// Complete local Cord execution limits; admitted only by an explicit profile.
pub const ASSIGNED_KERNEL_PLAN_SCHEMA: u16 = 5;
pub const ASSIGNED_PLAN_HEADER_BYTES: usize = 125;
pub const TINY_HOST_TOTAL_BYTES: u16 = 2_560;
const MAGIC: &[u8; 8] = b"CNDAP001";
pub const ASSIGNED_PLAN_COUNT_KINDS: usize = 13;

pub const ASSIGNED_NODE: u8 = 1;
pub const ASSIGNED_PORT: u8 = 2;
pub const ASSIGNED_CONFIGURATION: u8 = 3;
pub const ASSIGNED_CORD: u8 = 4;
pub const ASSIGNED_ROUTE: u8 = 5;
pub const ASSIGNED_ROUTE_TARGET: u8 = 6;
pub const ASSIGNED_HOST_CALL: u8 = 7;
pub const ASSIGNED_RESOURCE: u8 = 8;
pub const ASSIGNED_SIGN: u8 = 9;
pub const ASSIGNED_REMOTE_ENDPOINT: u8 = 10;
pub const ASSIGNED_STARTUP: u8 = 11;
pub const ASSIGNED_TERMINAL: u8 = 12;
pub const ASSIGNED_RESOURCE_CORD: u8 = 13;

/// Allocation-free execution encoding of the semantic delivery pressure policy.
/// Semantic planning owns the richer contract; assigned Plans and the kernel
/// carry only the exact finite behavior needed during Play.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum AssignedPressurePolicy {
    #[default]
    PreserveOrder = 0,
    CoalesceLatest = 1,
}

/// Allocation-free execution projection of a Cord's semantic track.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum AssignedConnectionTrack {
    #[default]
    Payload = 0,
    NormalClose = 1,
    AbnormalTerminal = 2,
    Quiescence = 3,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[repr(u8)]
pub enum DeliveryPressurePolicy {
    #[default]
    PreserveOrder,
    CoalesceLatest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedIdentity(pub [u8; 16]);

impl AssignedIdentity {
    pub fn from_text(value: &str) -> Self {
        let digest = sha256::digest(value.as_bytes());
        let mut result = [0; 16];
        result.copy_from_slice(&digest[..16]);
        Self(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedRemoteBinding {
    pub line: AssignedIdentity,
    pub local_host: AssignedIdentity,
    pub local_boot: AssignedIdentity,
    pub peer_host: AssignedIdentity,
    pub peer_boot: AssignedIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedPlanMaxima {
    pub encoded_bytes: u16,
    pub runtime_state_bytes: u16,
    pub counts: [u8; ASSIGNED_PLAN_COUNT_KINDS],
}

impl AssignedPlanMaxima {
    /// Exact storage ceiling for the generic one-source tiny-Host profile.
    pub const SINGLE_SOURCE: Self = Self {
        encoded_bytes: 544,
        runtime_state_bytes: 192,
        counts: [1, 1, 0, 0, 0, 0, 1, 3, 4, 0, 1, 2, 0],
    };

    pub const TINY_HOST: Self = Self {
        encoded_bytes: 1_536,
        runtime_state_bytes: 1_024,
        counts: [8, 16, 8, 8, 8, 8, 8, 8, 16, 2, 8, 8, 8],
    };

    pub const fn total_bytes(self) -> u16 {
        self.encoded_bytes + self.runtime_state_bytes
    }
}

#[derive(Clone, Copy)]
pub struct AssignedPlanRequirements<'a> {
    pub host: AssignedIdentity,
    pub boot: AssignedIdentity,
    pub host_calls: &'a [AssignedIdentity],
    pub resources: &'a [u16],
    pub remote_bindings: &'a [AssignedRemoteBinding],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedPlanView {
    pub plan: AssignedIdentity,
    pub fragment: AssignedIdentity,
    pub host: AssignedIdentity,
    pub boot: AssignedIdentity,
    pub encoded_bytes: u16,
    pub runtime_state_bytes: u16,
    pub counts: [u8; ASSIGNED_PLAN_COUNT_KINDS],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignedPlanRefusal {
    WrongLength,
    WrongMagic,
    WrongSchema,
    WrongHost,
    WrongBoot,
    EncodedCapacityExceeded,
    RuntimeCapacityExceeded,
    CountCapacityExceeded(u8),
    DigestMismatch,
    MalformedRecord,
    UnknownRecord(u8),
    UnknownHostCall,
    MissingHostCall,
    MissingBack,
    UnknownResource,
    MissingResource,
    StaleRemoteEndpoint,
    MissingRemoteEndpoint,
    ExtraRecords,
    ComputationCapacityExceeded,
    UnknownComputation,
    MissingComputation,
    UnknownForeEndpoint,
    MissingForeEndpoint,
}

pub fn decode_assigned_plan(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    decode(bytes, maxima, requirements, None)
}

/// Opt-in exact resident computation inventory; ordinary tiny profiles retain
/// their existing configuration ceiling. This does not authorize native effects
/// or validate evaluator storage/work, routing, or the complete kernel image.
pub fn decode_assigned_plan_with_computations(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
    profile: AssignedComputationProfile<'_>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    decode(bytes, maxima, requirements, Some(profile))
}

pub fn assigned_plan_payload_digest(payload: &[u8]) -> [u8; 32] {
    sha256::digest(payload)
}

pub fn assigned_plan_magic() -> [u8; 8] {
    *MAGIC
}

fn read_identity(bytes: &[u8], offset: usize) -> Result<AssignedIdentity, AssignedPlanRefusal> {
    let mut result = [0; 16];
    result.copy_from_slice(
        bytes
            .get(
                offset
                    ..offset
                        .checked_add(16)
                        .ok_or(AssignedPlanRefusal::MalformedRecord)?,
            )
            .ok_or(AssignedPlanRefusal::MalformedRecord)?,
    );
    Ok(AssignedIdentity(result))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, AssignedPlanRefusal> {
    Ok(u16::from_le_bytes(
        bytes
            .get(
                offset
                    ..offset
                        .checked_add(2)
                        .ok_or(AssignedPlanRefusal::MalformedRecord)?,
            )
            .ok_or(AssignedPlanRefusal::MalformedRecord)?
            .try_into()
            .map_err(|_| AssignedPlanRefusal::MalformedRecord)?,
    ))
}
