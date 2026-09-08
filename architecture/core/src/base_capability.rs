//! Unforgeable, revocable possession for exact Base operations.
//!
//! Public Conduit identities remain descriptive. The only value accepted by
//! this table is [`BaseCapabilityHandle`], whose bearer bytes have no public
//! constructor and are derived from issuer-private key material.

use crate::{
    ActivePlayId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseInstanceId, BootId,
    CapabilityEnvelopeId, CapabilityId, CapabilityPossessionId, HostId, HostOperationContractId,
    ImplementationId, KindId, PlanId, ResourceGenerationId, ResourcePoolId,
};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseCapabilityAuthority {
    pub grant: AuthorityGrant,
    pub base_instance_id: BaseInstanceId,
    pub base_provider_generation: u64,
    pub resource_pool_id: ResourcePoolId,
    pub resource_generation_id: ResourceGenerationId,
    pub operation_contract_id: HostOperationContractId,
    pub envelope_id: CapabilityEnvelopeId,
    pub maximum_parameter_bytes: u32,
    pub maximum_result_bytes: u32,
    pub maximum_work_units: u64,
    pub maximum_in_flight: u16,
    pub maximum_operations: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseCapabilityScope {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub base_instance_id: BaseInstanceId,
    pub base_provider_generation: u64,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub authority_grant_id: AuthorityGrantId,
    pub authority_contract_id: AuthorityContractId,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub operation_contract_id: HostOperationContractId,
    pub subject_kind: KindId,
    pub resource_pool_id: ResourcePoolId,
    pub resource_generation_id: ResourceGenerationId,
    pub envelope_id: CapabilityEnvelopeId,
    pub maximum_parameter_bytes: u32,
    pub maximum_result_bytes: u32,
    pub maximum_work_units: u64,
    pub maximum_in_flight: u16,
    pub maximum_operations: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityIssueRequest {
    pub scope: BaseCapabilityScope,
    pub authority: BaseCapabilityAuthority,
}

/// Opaque local bearer capability. It is intentionally neither serializable
/// nor constructible from an authority ID, Plan, inspection record, or string.
#[derive(Clone, PartialEq, Eq)]
pub struct BaseCapabilityHandle {
    bearer: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseOperationClaim {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub base_instance_id: BaseInstanceId,
    pub base_provider_generation: u64,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub implementation_id: ImplementationId,
    pub operation_contract_id: HostOperationContractId,
    pub subject_kind: KindId,
    pub resource_pool_id: ResourcePoolId,
    pub resource_generation_id: ResourceGenerationId,
    pub envelope_id: CapabilityEnvelopeId,
    pub parameter_bytes: u32,
    pub work_units: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct BaseOperationLease {
    bearer: [u8; 32],
    operation_sequence: u32,
    revocation_generation: u64,
}

impl core::fmt::Debug for BaseCapabilityHandle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("BaseCapabilityHandle(<redacted>)")
    }
}

impl core::fmt::Debug for BaseOperationLease {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BaseOperationLease")
            .field("bearer", &"<redacted>")
            .field("operation_sequence", &self.operation_sequence)
            .finish()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityLifecycle {
    Issued,
    Revoked,
    Exhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityInspection {
    pub possession_id: CapabilityPossessionId,
    pub scope: BaseCapabilityScope,
    pub lifecycle: CapabilityLifecycle,
    pub completed_operations: u32,
    pub in_flight_operations: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseCapabilityRefusal {
    InvalidIssuer,
    TableFull,
    EmptyIdentity,
    InvalidBound,
    AuthorityMismatch,
    ScopeBroadening,
    UnknownCapability,
    Revoked,
    Exhausted,
    WrongScope,
    ParameterEnvelope,
    WorkEnvelope,
    InFlightFull,
    UnknownLease,
    StaleCompletion,
    ResultEnvelope,
}

#[derive(Clone)]
struct CapabilityEntry {
    bearer: [u8; 32],
    inspection: CapabilityInspection,
    revocation_generation: u64,
    next_operation_sequence: u32,
    active_sequences: Vec<u32>,
}

pub struct BaseCapabilityTable {
    host_id: HostId,
    boot_id: BootId,
    base_instance_id: BaseInstanceId,
    base_provider_generation: u64,
    issuer_key: [u8; 32],
    maximum_capabilities: u16,
    next_issuance: u64,
    entries: Vec<CapabilityEntry>,
}

impl BaseCapabilityTable {
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        base_instance_id: BaseInstanceId,
        base_provider_generation: u64,
        issuer_key: [u8; 32],
        maximum_capabilities: u16,
    ) -> Result<Self, BaseCapabilityRefusal> {
        if host_id.as_str().is_empty()
            || boot_id.as_str().is_empty()
            || base_instance_id.as_str().is_empty()
            || base_provider_generation == 0
            || issuer_key == [0; 32]
            || maximum_capabilities == 0
        {
            return Err(BaseCapabilityRefusal::InvalidIssuer);
        }
        Ok(Self {
            host_id,
            boot_id,
            base_instance_id,
            base_provider_generation,
            issuer_key,
            maximum_capabilities,
            next_issuance: 1,
            entries: Vec::with_capacity(maximum_capabilities as usize),
        })
    }

    pub fn issue(
        &mut self,
        request: CapabilityIssueRequest,
    ) -> Result<BaseCapabilityHandle, BaseCapabilityRefusal> {
        if self.entries.len() == self.maximum_capabilities as usize {
            return Err(BaseCapabilityRefusal::TableFull);
        }
        validate_issue_request(self, &request)?;
        let issuance = self.next_issuance;
        self.next_issuance = self.next_issuance.saturating_add(1);
        let bearer = bearer_digest(&self.issuer_key, issuance, &request.scope);
        let possession_id =
            CapabilityPossessionId::from(hex_digest(&inspection_digest(issuance, &request.scope)));
        self.entries.push(CapabilityEntry {
            bearer,
            inspection: CapabilityInspection {
                possession_id,
                scope: request.scope,
                lifecycle: CapabilityLifecycle::Issued,
                completed_operations: 0,
                in_flight_operations: 0,
            },
            revocation_generation: 1,
            next_operation_sequence: 1,
            active_sequences: Vec::new(),
        });
        Ok(BaseCapabilityHandle { bearer })
    }

    pub fn authorize(
        &mut self,
        handle: &BaseCapabilityHandle,
        claim: &BaseOperationClaim,
    ) -> Result<BaseOperationLease, BaseCapabilityRefusal> {
        let entry = self.entry_mut(handle)?;
        match entry.inspection.lifecycle {
            CapabilityLifecycle::Revoked => return Err(BaseCapabilityRefusal::Revoked),
            CapabilityLifecycle::Exhausted => return Err(BaseCapabilityRefusal::Exhausted),
            CapabilityLifecycle::Issued => {}
        }
        validate_claim(&entry.inspection.scope, claim)?;
        if claim.parameter_bytes > entry.inspection.scope.maximum_parameter_bytes {
            return Err(BaseCapabilityRefusal::ParameterEnvelope);
        }
        if claim.work_units > entry.inspection.scope.maximum_work_units {
            return Err(BaseCapabilityRefusal::WorkEnvelope);
        }
        if entry.inspection.in_flight_operations == entry.inspection.scope.maximum_in_flight {
            return Err(BaseCapabilityRefusal::InFlightFull);
        }
        if entry.inspection.completed_operations as u64
            + entry.inspection.in_flight_operations as u64
            >= entry.inspection.scope.maximum_operations as u64
        {
            entry.inspection.lifecycle = CapabilityLifecycle::Exhausted;
            return Err(BaseCapabilityRefusal::Exhausted);
        }
        let operation_sequence = entry.next_operation_sequence;
        entry.next_operation_sequence = entry.next_operation_sequence.saturating_add(1);
        entry.active_sequences.push(operation_sequence);
        entry.inspection.in_flight_operations += 1;
        Ok(BaseOperationLease {
            bearer: entry.bearer,
            operation_sequence,
            revocation_generation: entry.revocation_generation,
        })
    }

    pub fn complete(
        &mut self,
        lease: BaseOperationLease,
        result_bytes: u32,
    ) -> Result<(), BaseCapabilityRefusal> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| bearer_matches(&entry.bearer, &lease.bearer))
            .ok_or(BaseCapabilityRefusal::UnknownLease)?;
        if entry.revocation_generation != lease.revocation_generation
            || entry.inspection.lifecycle == CapabilityLifecycle::Revoked
        {
            return Err(BaseCapabilityRefusal::StaleCompletion);
        }
        if result_bytes > entry.inspection.scope.maximum_result_bytes {
            return Err(BaseCapabilityRefusal::ResultEnvelope);
        }
        let position = entry
            .active_sequences
            .iter()
            .position(|sequence| *sequence == lease.operation_sequence)
            .ok_or(BaseCapabilityRefusal::UnknownLease)?;
        entry.active_sequences.swap_remove(position);
        entry.inspection.in_flight_operations -= 1;
        entry.inspection.completed_operations += 1;
        if entry.inspection.completed_operations == entry.inspection.scope.maximum_operations {
            entry.inspection.lifecycle = CapabilityLifecycle::Exhausted;
        }
        Ok(())
    }

    pub fn revoke(&mut self, handle: &BaseCapabilityHandle) -> Result<(), BaseCapabilityRefusal> {
        let entry = self.entry_mut(handle)?;
        entry.inspection.lifecycle = CapabilityLifecycle::Revoked;
        entry.revocation_generation = entry.revocation_generation.saturating_add(1);
        entry.active_sequences.clear();
        entry.inspection.in_flight_operations = 0;
        Ok(())
    }

    pub fn revoke_play(&mut self, active_play_id: &ActivePlayId) {
        self.revoke_matching(|scope| &scope.active_play_id == active_play_id);
    }

    pub fn revoke_plan(&mut self, plan_id: &PlanId) {
        self.revoke_matching(|scope| &scope.plan_id == plan_id);
    }

    pub fn revoke_authority(&mut self, grant_id: &AuthorityGrantId) {
        self.revoke_matching(|scope| &scope.authority_grant_id == grant_id);
    }

    pub fn revoke_resource_generation(
        &mut self,
        pool_id: &ResourcePoolId,
        generation_id: &ResourceGenerationId,
    ) {
        self.revoke_matching(|scope| {
            &scope.resource_pool_id == pool_id && &scope.resource_generation_id == generation_id
        });
    }

    pub fn inspections(&self) -> impl Iterator<Item = &CapabilityInspection> {
        self.entries.iter().map(|entry| &entry.inspection)
    }

    fn entry_mut(
        &mut self,
        handle: &BaseCapabilityHandle,
    ) -> Result<&mut CapabilityEntry, BaseCapabilityRefusal> {
        self.entries
            .iter_mut()
            .find(|entry| bearer_matches(&entry.bearer, &handle.bearer))
            .ok_or(BaseCapabilityRefusal::UnknownCapability)
    }

    fn revoke_matching(&mut self, matches: impl Fn(&BaseCapabilityScope) -> bool) {
        for entry in &mut self.entries {
            if matches(&entry.inspection.scope) {
                entry.inspection.lifecycle = CapabilityLifecycle::Revoked;
                entry.revocation_generation = entry.revocation_generation.saturating_add(1);
                entry.active_sequences.clear();
                entry.inspection.in_flight_operations = 0;
            }
        }
    }
}

fn validate_issue_request(
    table: &BaseCapabilityTable,
    request: &CapabilityIssueRequest,
) -> Result<(), BaseCapabilityRefusal> {
    let scope = &request.scope;
    let authority = &request.authority;
    if scope.plan_id.as_str().is_empty()
        || scope.active_play_id.as_str().is_empty()
        || scope.authority_grant_id.as_str().is_empty()
        || scope.authority_contract_id.as_str().is_empty()
        || scope.capability_id.as_str().is_empty()
        || scope.implementation_id.as_str().is_empty()
        || scope.operation_contract_id.as_str().is_empty()
        || scope.subject_kind.as_str().is_empty()
        || scope.resource_pool_id.as_str().is_empty()
        || scope.resource_generation_id.0.is_empty()
        || scope.envelope_id.as_str().is_empty()
    {
        return Err(BaseCapabilityRefusal::EmptyIdentity);
    }
    if scope.maximum_parameter_bytes == 0
        || scope.maximum_result_bytes == 0
        || scope.maximum_work_units == 0
        || scope.maximum_in_flight == 0
        || scope.maximum_operations == 0
    {
        return Err(BaseCapabilityRefusal::InvalidBound);
    }
    if scope.host_id != table.host_id
        || scope.boot_id != table.boot_id
        || scope.base_instance_id != table.base_instance_id
        || scope.base_provider_generation != table.base_provider_generation
        || authority.grant.host_id != table.host_id
        || authority.grant.boot_id != table.boot_id
        || authority.base_instance_id != table.base_instance_id
        || authority.base_provider_generation != table.base_provider_generation
        || authority.grant.grant_id != scope.authority_grant_id
        || authority.grant.contract_id != scope.authority_contract_id
        || authority.grant.capability_id != scope.capability_id
        || authority.grant.host_operation_contract_id != scope.operation_contract_id
        || authority.grant.subject_kind != scope.subject_kind
        || authority.operation_contract_id != scope.operation_contract_id
        || authority.resource_pool_id != scope.resource_pool_id
        || authority.resource_generation_id != scope.resource_generation_id
        || authority.envelope_id != scope.envelope_id
    {
        return Err(BaseCapabilityRefusal::AuthorityMismatch);
    }
    if scope.maximum_parameter_bytes > authority.maximum_parameter_bytes
        || scope.maximum_result_bytes > authority.maximum_result_bytes
        || scope.maximum_work_units > authority.maximum_work_units
        || scope.maximum_in_flight > authority.maximum_in_flight
        || scope.maximum_operations > authority.maximum_operations
    {
        return Err(BaseCapabilityRefusal::ScopeBroadening);
    }
    Ok(())
}

fn validate_claim(
    scope: &BaseCapabilityScope,
    claim: &BaseOperationClaim,
) -> Result<(), BaseCapabilityRefusal> {
    if claim.host_id != scope.host_id
        || claim.boot_id != scope.boot_id
        || claim.base_instance_id != scope.base_instance_id
        || claim.base_provider_generation != scope.base_provider_generation
        || claim.plan_id != scope.plan_id
        || claim.active_play_id != scope.active_play_id
        || claim.implementation_id != scope.implementation_id
        || claim.operation_contract_id != scope.operation_contract_id
        || claim.subject_kind != scope.subject_kind
        || claim.resource_pool_id != scope.resource_pool_id
        || claim.resource_generation_id != scope.resource_generation_id
        || claim.envelope_id != scope.envelope_id
    {
        return Err(BaseCapabilityRefusal::WrongScope);
    }
    Ok(())
}

fn bearer_digest(key: &[u8; 32], issuance: u64, scope: &BaseCapabilityScope) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit/base-capability/bearer/v1");
    hash.update(key);
    hash.update(issuance.to_le_bytes());
    hash_scope(&mut hash, scope);
    hash.finalize().into()
}

fn inspection_digest(issuance: u64, scope: &BaseCapabilityScope) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit/base-capability/inspection/v1");
    hash.update(issuance.to_le_bytes());
    hash_scope(&mut hash, scope);
    hash.finalize().into()
}

fn hash_scope(hash: &mut Sha256, scope: &BaseCapabilityScope) {
    for value in [
        scope.host_id.as_str(),
        scope.boot_id.as_str(),
        scope.base_instance_id.as_str(),
        scope.plan_id.as_str(),
        scope.active_play_id.as_str(),
        scope.authority_grant_id.as_str(),
        scope.authority_contract_id.as_str(),
        scope.capability_id.as_str(),
        scope.implementation_id.as_str(),
        scope.operation_contract_id.as_str(),
        scope.subject_kind.as_str(),
        scope.resource_pool_id.as_str(),
        scope.resource_generation_id.0.as_str(),
        scope.envelope_id.as_str(),
    ] {
        hash.update((value.len() as u32).to_le_bytes());
        hash.update(value.as_bytes());
    }
    hash.update(scope.base_provider_generation.to_le_bytes());
    hash.update(scope.maximum_parameter_bytes.to_le_bytes());
    hash.update(scope.maximum_result_bytes.to_le_bytes());
    hash.update(scope.maximum_work_units.to_le_bytes());
    hash.update(scope.maximum_in_flight.to_le_bytes());
    hash.update(scope.maximum_operations.to_le_bytes());
}

fn bearer_matches(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn hex_digest(bytes: &[u8; 32]) -> alloc::string::String {
    use alloc::format;
    let mut result = alloc::string::String::with_capacity(64);
    for byte in bytes {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}
