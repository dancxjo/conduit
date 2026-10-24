//! Explicit local external Fore projection. No Line, peer or effect authority.
use crate::*;
/// Native binding ABI from schema 6, with separately pinned local Fore records.
pub const ASSIGNED_FORE_NATIVE_PLAN_SCHEMA: u16 = 7;
/// Schema 7 uses endpoint tag 10 for local Fore; earlier versions mean a Line.
pub const ASSIGNED_LOCAL_FORE: u8 = 10;
pub const ASSIGNED_LOCAL_FORE_BYTES: usize = 53;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AssignedForeDirection {
    Input = 0,
    Output = 1,
}
/// Trusted Make material, not a received caller's assertion of a valid ABI.
/// The ABI digest pins the complete checked value/temporal/quantity contract and
/// finite storage limits. Runtime ingress must retain and enforce that contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignedForeRequirement {
    payload: [u8; ASSIGNED_LOCAL_FORE_BYTES],
}
impl AssignedForeRequirement {
    pub fn new(
        endpoint: u16,
        cord: u16,
        direction: AssignedForeDirection,
        front_port: AssignedIdentity,
        abi_digest: [u8; 32],
    ) -> Self {
        let mut payload = [0; ASSIGNED_LOCAL_FORE_BYTES];
        payload[..2].copy_from_slice(&endpoint.to_le_bytes());
        payload[2..4].copy_from_slice(&cord.to_le_bytes());
        payload[4] = direction as u8;
        payload[5..21].copy_from_slice(&front_port.0);
        payload[21..].copy_from_slice(&abi_digest);
        Self { payload }
    }
    pub fn payload(&self) -> &[u8; ASSIGNED_LOCAL_FORE_BYTES] {
        &self.payload
    }
    pub fn endpoint(&self) -> u16 {
        u16::from_le_bytes([self.payload[0], self.payload[1]])
    }
    pub fn cord(&self) -> u16 {
        u16::from_le_bytes([self.payload[2], self.payload[3]])
    }
    pub fn direction(&self) -> AssignedForeDirection {
        if self.payload[4] == 0 {
            AssignedForeDirection::Input
        } else {
            AssignedForeDirection::Output
        }
    }
}
/// Opt-in resident Fore admission. This validates representation and exact
/// resident pins, not kernel storage, Body membership or caller possession.
pub fn decode_assigned_plan_with_fore(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    required: AssignedPlanRequirements<'_>,
    computations: AssignedComputationProfile<'_>,
    fore: &[AssignedForeRequirement],
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    crate::validation::decode_fore(bytes, maxima, required, computations, fore)
}
pub fn decode_assigned_records_with_fore(
    header: &[u8],
    records: &[AssignedRecord<'_>],
    maxima: AssignedPlanMaxima,
    required: AssignedPlanRequirements<'_>,
    computations: AssignedComputationProfile<'_>,
    fore: &[AssignedForeRequirement],
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    crate::validation::decode_records(header, records, maxima, required, computations, Some(fore))
}
pub(crate) struct Validation<'a> {
    required: &'a [AssignedForeRequirement],
    seen: [bool; 16],
}
impl<'a> Validation<'a> {
    pub(crate) fn new(
        required: &'a [AssignedForeRequirement],
    ) -> Result<Self, AssignedPlanRefusal> {
        if required.len() > 16
            || required.iter().enumerate().any(|(i, r)| {
                required[..i]
                    .iter()
                    .any(|p| p.endpoint() == r.endpoint() || p.cord() == r.cord())
            })
        {
            return Err(AssignedPlanRefusal::MalformedRecord);
        }
        Ok(Self {
            required,
            seen: [false; 16],
        })
    }
    pub(crate) fn record(&mut self, payload: &[u8]) -> Result<(), AssignedPlanRefusal> {
        let i = self
            .required
            .iter()
            .enumerate()
            .find(|(i, r)| !self.seen[*i] && r.payload.as_slice() == payload)
            .map(|(i, _)| i)
            .ok_or(AssignedPlanRefusal::UnknownForeEndpoint)?;
        self.seen[i] = true;
        Ok(())
    }
    pub(crate) fn finish(&self) -> Result<(), AssignedPlanRefusal> {
        if self.seen[..self.required.len()].contains(&false) {
            Err(AssignedPlanRefusal::MissingForeEndpoint)
        } else {
            Ok(())
        }
    }
}
