//! Unforgeable, revocable possession for exact Base operations.
//!
//! Public Conduit identities remain descriptive. The only value accepted by
//! this table is [`BaseCapabilityHandle`], whose bearer bytes have no public
//! constructor and are derived from issuer-private key material.

mod bound;
mod capacity;
pub use bound::BoundBaseOperation;

use crate::{
    ActivePlayId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseInstanceId, BootId,
    CapabilityEnvelopeId, CapabilityId, CapabilityPossessionId, HostCallContractId, HostId,
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
    pub operation_contract_id: HostCallContractId,
    pub envelope_id: CapabilityEnvelopeId,
    pub maximum_parameter_bytes: u32,
    pub maximum_result_bytes: u32,
    pub maximum_work_units: u64,
    pub maximum_in_flight: u16,
    pub maximum_operations: u32,
}

/// Borrowed descriptive policy for the same ordinary issuance checks. This is
/// not a grant, possession, or authorization proof; the actual issuing table
/// validates every field before retaining its owned immutable scope.
#[derive(Clone, Copy)]
pub struct BaseCapabilityAuthorityRef<'a> {
    pub grant: crate::AuthorityGrantRef<'a>,
    pub base_instance_id: &'a str,
    pub base_provider_generation: u64,
    pub resource_pool_id: &'a str,
    pub resource_generation_id: &'a str,
    pub operation_contract_id: &'a str,
    pub envelope_id: &'a str,
    pub maximum_parameter_bytes: u32,
    pub maximum_result_bytes: u32,
    pub maximum_work_units: u64,
    pub maximum_in_flight: u16,
    pub maximum_operations: u32,
}
impl<'a> From<&'a BaseCapabilityAuthority> for BaseCapabilityAuthorityRef<'a> {
    fn from(authority: &'a BaseCapabilityAuthority) -> Self {
        Self {
            grant: (&authority.grant).into(),
            base_instance_id: authority.base_instance_id.as_str(),
            base_provider_generation: authority.base_provider_generation,
            resource_pool_id: authority.resource_pool_id.as_str(),
            resource_generation_id: authority.resource_generation_id.0.as_str(),
            operation_contract_id: authority.operation_contract_id.as_str(),
            envelope_id: authority.envelope_id.as_str(),
            maximum_parameter_bytes: authority.maximum_parameter_bytes,
            maximum_result_bytes: authority.maximum_result_bytes,
            maximum_work_units: authority.maximum_work_units,
            maximum_in_flight: authority.maximum_in_flight,
            maximum_operations: authority.maximum_operations,
        }
    }
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
    pub operation_contract_id: HostCallContractId,
    pub subject_kind: KindId,
    pub resource_pool_id: ResourcePoolId,
    pub resource_generation_id: ResourceGenerationId,
    pub envelope_id: CapabilityEnvelopeId,
    pub maximum_parameter_bytes: u32,
    /// Zero permits completion only without result bytes (for example a wait).
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

/// Opaque move-only local bearer capability.
///
/// It is intentionally neither clonable, serializable, nor constructible from
/// an authority ID, Plan, inspection record, or string. Successful operation
/// completion rotates the bearer in place; refusal leaves the same possession
/// available for a later valid attempt.
#[derive(PartialEq, Eq)]
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
    pub operation_contract_id: HostCallContractId,
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
    Capacity,
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

/// Admitted memory only; this value conveys no capability or authority.
pub struct BaseCapabilityIssuanceBacking {
    sequences: Vec<u32>,
    possession: alloc::string::String,
}
impl BaseCapabilityIssuanceBacking {
    pub fn try_new(maximum_in_flight: u16) -> Result<Self, BaseCapabilityRefusal> {
        let mut sequences = Vec::new();
        sequences
            .try_reserve_exact(usize::from(maximum_in_flight))
            .map_err(|_| BaseCapabilityRefusal::Capacity)?;
        let mut possession = alloc::string::String::new();
        possession
            .try_reserve_exact(64)
            .map_err(|_| BaseCapabilityRefusal::Capacity)?;
        Ok(Self {
            sequences,
            possession,
        })
    }
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
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(usize::from(maximum_capabilities))
            .map_err(|_| BaseCapabilityRefusal::Capacity)?;
        Ok(Self {
            host_id,
            boot_id,
            base_instance_id,
            base_provider_generation,
            issuer_key,
            maximum_capabilities,
            next_issuance: 1,
            entries,
        })
    }

    pub fn issue(
        &mut self,
        request: CapabilityIssueRequest,
    ) -> Result<BaseCapabilityHandle, BaseCapabilityRefusal> {
        let CapabilityIssueRequest { scope, authority } = request;
        self.issue_borrowed(scope, (&authority).into())
    }

    /// Same validation, refusal ordering, bearer transcript and immutable scope
    /// retention as `issue`, without cloning phase-local authority metadata.
    pub fn issue_borrowed(
        &mut self,
        scope: BaseCapabilityScope,
        authority: BaseCapabilityAuthorityRef<'_>,
    ) -> Result<BaseCapabilityHandle, BaseCapabilityRefusal> {
        if self.entries.len() == self.maximum_capabilities as usize {
            return Err(BaseCapabilityRefusal::TableFull);
        }
        validate_issue_request(self, &scope, authority)?;
        let backing = BaseCapabilityIssuanceBacking::try_new(scope.maximum_in_flight)?;
        self.issue_validated(scope, backing)
    }

    pub fn issue_borrowed_with_backing(
        &mut self,
        scope: BaseCapabilityScope,
        authority: BaseCapabilityAuthorityRef<'_>,
        backing: BaseCapabilityIssuanceBacking,
    ) -> Result<BaseCapabilityHandle, BaseCapabilityRefusal> {
        if self.entries.len() == self.maximum_capabilities as usize {
            return Err(BaseCapabilityRefusal::TableFull);
        }
        validate_issue_request(self, &scope, authority)?;
        if backing.sequences.capacity() < usize::from(scope.maximum_in_flight)
            || backing.possession.capacity() < 64
        {
            return Err(BaseCapabilityRefusal::Capacity);
        }
        self.issue_validated(scope, backing)
    }

    fn issue_validated(
        &mut self,
        scope: BaseCapabilityScope,
        mut backing: BaseCapabilityIssuanceBacking,
    ) -> Result<BaseCapabilityHandle, BaseCapabilityRefusal> {
        let issuance = self.next_issuance;
        let bearer = bearer_digest(&self.issuer_key, issuance, &scope);
        write_hex_digest(
            &mut backing.possession,
            &inspection_digest(issuance, &scope),
        );
        let possession_id = CapabilityPossessionId::from(backing.possession);
        self.entries.push(CapabilityEntry {
            bearer,
            active_sequences: backing.sequences,
            inspection: CapabilityInspection {
                possession_id,
                scope,
                lifecycle: CapabilityLifecycle::Issued,
                completed_operations: 0,
                in_flight_operations: 0,
            },
            revocation_generation: 1,
            next_operation_sequence: 1,
        });
        self.next_issuance = self.next_issuance.saturating_add(1);
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
        bound::begin_entry(entry)
    }

    pub fn complete(
        &mut self,
        handle: &mut BaseCapabilityHandle,
        lease: BaseOperationLease,
        result_bytes: u32,
    ) -> Result<(), BaseCapabilityRefusal> {
        let issuer_key = self.issuer_key;
        let entry = self.entry_mut(handle)?;
        if !bearer_matches(&entry.bearer, &lease.bearer) {
            return Err(BaseCapabilityRefusal::UnknownLease);
        }
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
        } else if entry.inspection.in_flight_operations == 0 {
            let mut rotation = Sha256::new();
            rotation.update(issuer_key);
            rotation.update(entry.bearer);
            rotation.update(lease.operation_sequence.to_le_bytes());
            rotation.update(entry.revocation_generation.to_le_bytes());
            let bearer = rotation.finalize().into();
            entry.bearer = bearer;
            handle.bearer = bearer;
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
    scope: &BaseCapabilityScope,
    authority: BaseCapabilityAuthorityRef<'_>,
) -> Result<(), BaseCapabilityRefusal> {
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
        || authority.grant.host_id != table.host_id.as_str()
        || authority.grant.boot_id != table.boot_id.as_str()
        || authority.base_instance_id != table.base_instance_id.as_str()
        || authority.base_provider_generation != table.base_provider_generation
        || authority.grant.grant_id != scope.authority_grant_id.as_str()
        || authority.grant.contract_id != scope.authority_contract_id.as_str()
        || authority.grant.capability_id != scope.capability_id.as_str()
        || authority.grant.host_call_contract_id != scope.operation_contract_id.as_str()
        || authority.grant.subject_kind != scope.subject_kind.as_str()
        || authority.operation_contract_id != scope.operation_contract_id.as_str()
        || authority.resource_pool_id != scope.resource_pool_id.as_str()
        || authority.resource_generation_id != scope.resource_generation_id.0.as_str()
        || authority.envelope_id != scope.envelope_id.as_str()
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

fn write_hex_digest(result: &mut alloc::string::String, bytes: &[u8; 32]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 15)]));
    }
}

