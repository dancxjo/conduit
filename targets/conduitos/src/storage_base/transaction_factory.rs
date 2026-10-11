//! Finite ordinary kernel Backs for independently admitted storage transactions.
use super::{contract::*, transaction_contract::*};
use alloc::{boxed::Box, string::String, vec};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{
    HostedValueStore,
    scheduler::{HostCallBack, StepBack},
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub const AUTHORIZE_IMPLEMENTATION: &str = "conduitos/storage-block-read-authorize@1";
pub const SETTLE_IMPLEMENTATION: &str = "conduitos/storage-block-read-settle@1";
pub const TRANSACTION_PROFILE: &str = "conduitos/storage-block-read-transaction-bounded@1";
pub const TRANSACTION_BASE: &str = "conduitos.base/storage-block-read@1";
pub const TRANSACTION_RESOURCE: &str = "machine/storage/block-read-transaction";

#[derive(Clone, Copy)]
pub enum StorageTransactionRole {
    Authorize,
    Settle,
}
impl StorageTransactionRole {
    pub fn implementation(self) -> &'static str {
        match self {
            Self::Authorize => AUTHORIZE_IMPLEMENTATION,
            Self::Settle => SETTLE_IMPLEMENTATION,
        }
    }
    fn call(self) -> &'static str {
        match self {
            Self::Authorize => AUTHORIZE_CALL,
            Self::Settle => SETTLE_CALL,
        }
    }
    fn kind(self, contract: &BlockReadTransactionContract) -> &Kind {
        match self {
            Self::Authorize => contract.authorize_kind(),
            Self::Settle => contract.settle_kind(),
        }
    }
}
pub struct StorageTransactionOperationFactory {
    role: StorageTransactionRole,
    implementation: ImplementationId,
    contract: BlockReadTransactionContract,
}
impl StorageTransactionOperationFactory {
    pub fn prepare(role: StorageTransactionRole) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            role,
            implementation: role.implementation().into(),
            contract: BlockReadTransactionContract::prepare()?,
        })
    }
    fn validate(&self, gear: &PlannedGear) -> Result<(), String> {
        let kind = self.role.kind(&self.contract);
        let base = gear
            .base
            .as_ref()
            .ok_or("storage transaction requires selected Base")?;
        let [call] = gear.host_calls.as_slice() else {
            return Err("storage transaction requires one Host Call".into());
        };
        if gear.implementation_id != self.implementation
            || gear.execution_profile_id.as_str() != TRANSACTION_PROFILE
            || gear.capability_id.as_str().is_empty()
            || gear.kind_id != kind.kind_id
            || gear.kind_contract_revision != kind.kind_contract_revision
            || gear.inputs != kind.inputs
            || gear.outputs != kind.outputs
            || gear.semantic_contract != kind.semantic_contract()
            || !gear.configuration.is_empty()
            || gear.limits != kind.limits
            || base.implementation_id.as_str() != TRANSACTION_BASE
            || base.mechanism_family.as_str() != TRANSACTION_RESOURCE
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || call.contract_id.as_str() != self.role.call()
            || call.target_kind.as_ref() != Some(&kind.kind_id)
            || call.maximum_in_flight != 1
            || call.maximum_input_bytes != BLOCK_READ_MAXIMUM_BYTES
            || call.maximum_output_bytes != BLOCK_READ_MAXIMUM_BYTES
            || !gear.resources.iter().any(|resource| {
                resource.class_id.as_str() == TRANSACTION_RESOURCE && resource.units == 1
            })
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == BLOCK_READ_AUTHORITY
                    && authority.host_call_contract_id.as_str() == self.role.call()
                    && authority.subject_kind == kind.kind_id
                    && authority.host_id == gear.host_id
                    && authority.boot_id == gear.boot_id
                    && authority.capability_id == gear.capability_id
            })
        {
            return Err("storage transaction differs from exact selected realization".into());
        }
        Ok(())
    }
}
impl KernelOperationFactory for StorageTransactionOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.validate(gear)?;
        Ok(KernelOperationBudget {
            value_items: 4,
            value_bytes: BLOCK_READ_MAXIMUM_BYTES * 4,
            maximum_value_bytes: BLOCK_READ_MAXIMUM_BYTES,
            host_requests: 1,
            sign_items: 64,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.validate(gear)?;
        Ok(Box::new(HostCallBack::new(BLOCK_READ_MAXIMUM_BYTES)))
    }
}
/// Descriptive offer only; Root separately admits actual provider/resources and two scopes.
pub fn offer(
    role: StorageTransactionRole,
    contract: &BlockReadTransactionContract,
    artifact: ArtifactId,
) -> CapabilityOffer {
    let kind = role.kind(contract);
    capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(), startup_parameters: kind.startup_parameters.clone(), shorthand: kind.shorthand.clone(),
        capability_id: role.implementation().into(), kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer { execution_profile_id: TRANSACTION_PROFILE.into(), implementation_id: role.implementation().into(), artifact_id: artifact },
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement { contract_id: role.call().into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: BLOCK_READ_MAXIMUM_BYTES, maximum_output_bytes: BLOCK_READ_MAXIMUM_BYTES }],
        resource_requirements: vec![resource_requirement(TRANSACTION_RESOURCE, 1)],
        authority_requirements: vec![AuthorityRequirement { contract_id: BLOCK_READ_AUTHORITY.into(), host_call_contract_id: role.call().into(), subject_kind: kind.kind_id.clone() }],
        limits: kind.limits.clone(),
    }
}
