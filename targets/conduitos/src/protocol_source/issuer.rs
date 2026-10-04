//! Root-held authority issues possession only for an exact selected operation.
use crate::cryptographic_entropy::{
    CryptographicEntropyBase, CryptographicEntropySource, EntropyRefusal,
};
use conduit_core::*;

#[derive(Debug)]
pub enum NativeProtocolIssueRefusal {
    Entropy(EntropyRefusal),
    Capability(BaseCapabilityRefusal),
    WrongSelection,
}

/// Root-held issuer. Descriptive grant metadata alone cannot authorize admission.
pub struct NativeProtocolIssuer {
    authority: BaseCapabilityAuthority,
    table: BaseCapabilityTable,
}

pub struct NativeProtocolPossession {
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}

impl NativeProtocolIssuer {
    /// Retain independently authorized native authority and a fresh issuer key.
    ///
    /// # Safety
    /// The trusted native Root must already possess the exact authority and
    /// provider/resource generation described here, including firmware handoff,
    /// electrical permission and exclusive ownership. The entropy Base must
    /// retain an actually admitted cryptographic source. Metadata, discovery,
    /// package contents and planner selection do not establish these facts.
    pub unsafe fn admit<S: CryptographicEntropySource, const REQUESTS: u32>(
        authority: BaseCapabilityAuthority,
        entropy: &mut CryptographicEntropyBase<S, REQUESTS>,
    ) -> Result<Self, NativeProtocolIssueRefusal> {
        let table = entropy
            .with_secret::<32, _>(|key, _| {
                BaseCapabilityTable::new(
                    authority.grant.host_id.clone(),
                    authority.grant.boot_id.clone(),
                    authority.base_instance_id.clone(),
                    authority.base_provider_generation,
                    *key,
                    1,
                )
            })
            .map_err(NativeProtocolIssueRefusal::Entropy)?
            .map_err(NativeProtocolIssueRefusal::Capability)?;
        Ok(Self { authority, table })
    }

    /// Publish descriptive grant facts already retained by the admitted Root.
    /// Reading this metadata neither transfers possession nor issues a handle.
    pub fn grant(&self) -> &AuthorityGrant {
        &self.authority.grant
    }

    /// Consume this issuer for one exact planned native operation. The supplied
    /// active identity must be the actual retained fragment Play identity;
    /// this preparation does not create or start a Play.
    pub fn issue(
        mut self,
        plan: &Plan,
        active: &ActivePlayIdentity,
        implementation: &ImplementationId,
        work_units: u64,
    ) -> Result<NativeProtocolPossession, NativeProtocolIssueRefusal> {
        use NativeProtocolIssueRefusal as Error;
        if !verify_plan(plan) || plan.fragments.len() != 1 || work_units == 0 {
            return Err(Error::WrongSelection);
        }
        let fragment = &plan.fragments[0];
        let authority = &self.authority;
        if active.plan_id != plan.plan_id
            || active.host_id != authority.grant.host_id
            || active.boot_id != authority.grant.boot_id
            || fragment.host_id != active.host_id
            || fragment.boot_id != active.boot_id
            || bind_active_play(
                &active.plan_id,
                &active.host_id,
                &active.boot_id,
                active.play_sequence,
            ) != *active
        {
            return Err(Error::WrongSelection);
        }
        let mut matches = fragment
            .placements
            .iter()
            .filter(|gear| &gear.implementation_id == implementation);
        let gear = matches.next().ok_or(Error::WrongSelection)?;
        let base = gear.base.as_ref().ok_or(Error::WrongSelection)?;
        let [call] = gear.host_calls.as_slice() else {
            return Err(Error::WrongSelection);
        };
        if matches.next().is_some()
            || gear.host_id != active.host_id
            || gear.boot_id != active.boot_id
            || gear.kind_id != authority.grant.subject_kind
            || gear.capability_id != authority.grant.capability_id
            || !gear.authority.iter().any(|binding| {
                binding.grant_id == authority.grant.grant_id
                    && binding.contract_id == authority.grant.contract_id
                    && binding.host_call_contract_id == authority.grant.host_call_contract_id
                    && binding.subject_kind == authority.grant.subject_kind
                    && binding.host_id == authority.grant.host_id
                    && binding.boot_id == authority.grant.boot_id
                    && binding.capability_id == authority.grant.capability_id
            })
            || base.provider_instance_id != authority.base_instance_id
            || base.provider_generation != authority.base_provider_generation
            || call.contract_id != authority.operation_contract_id
            || call.target_kind.as_ref() != Some(&gear.kind_id)
            || !gear.resources.iter().any(|resource| {
                resource.pool_id == authority.resource_pool_id && resource.units > 0
            })
        {
            return Err(Error::WrongSelection);
        }
        let scope = BaseCapabilityScope {
            host_id: active.host_id.clone(),
            boot_id: active.boot_id.clone(),
            base_instance_id: authority.base_instance_id.clone(),
            base_provider_generation: authority.base_provider_generation,
            plan_id: active.plan_id.clone(),
            active_play_id: active.active_play_id.clone(),
            authority_grant_id: authority.grant.grant_id.clone(),
            authority_contract_id: authority.grant.contract_id.clone(),
            capability_id: gear.capability_id.clone(),
            implementation_id: implementation.clone(),
            operation_contract_id: call.contract_id.clone(),
            subject_kind: gear.kind_id.clone(),
            resource_pool_id: authority.resource_pool_id.clone(),
            resource_generation_id: authority.resource_generation_id.clone(),
            envelope_id: authority.envelope_id.clone(),
            maximum_parameter_bytes: call.maximum_input_bytes,
            maximum_result_bytes: call.maximum_output_bytes,
            maximum_work_units: work_units,
            maximum_in_flight: call.maximum_in_flight,
            maximum_operations: authority.maximum_operations,
        };
        let claim = BaseOperationClaim {
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            plan_id: scope.plan_id.clone(),
            active_play_id: scope.active_play_id.clone(),
            implementation_id: scope.implementation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            parameter_bytes: scope.maximum_parameter_bytes,
            work_units,
        };
        let handle = self
            .table
            .issue(CapabilityIssueRequest {
                scope,
                authority: self.authority,
            })
            .map_err(Error::Capability)?;
        Ok(NativeProtocolPossession {
            table: self.table,
            handle,
            claim,
        })
    }
}
