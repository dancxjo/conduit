//! Shared-kernel lifecycle for the exact selected native digital transaction.
use super::{
    contract::{DIGITAL_CALL, DIGITAL_MAXIMUM_BYTES, DigitalContract},
    installation::{
        DIGITAL_ATTACHMENT_CLASS, DIGITAL_AUTHORITY, DIGITAL_EXECUTION_PROFILE,
        DIGITAL_IMPLEMENTATION,
    },
};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{
    HostedValueStore,
    scheduler::{HostCallBack, StepBack},
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

/// Preparing the scheduler Back neither grants possession nor touches a controller.
/// Actual dispatch still crosses the exact selected DigitalHostCall owner.
pub struct DigitalOperationFactory {
    implementation: ImplementationId,
    contract: DigitalContract,
    capability_ids: Vec<CapabilityId>,
}
impl DigitalOperationFactory {
    pub fn prepare_contract(
        capability_ids: Vec<CapabilityId>,
    ) -> Result<Self, StructuredInfoRefusal> {
        if capability_ids.is_empty()
            || capability_ids.len() > 64
            || capability_ids
                .iter()
                .enumerate()
                .any(|(index, id)| id.as_str().is_empty() || capability_ids[..index].contains(id))
        {
            return Err(StructuredInfoRefusal::WrongType);
        }
        Ok(Self {
            implementation: ImplementationId::from(DIGITAL_IMPLEMENTATION),
            contract: DigitalContract::prepare()?,
            capability_ids,
        })
    }
    fn validate(&self, gear: &PlannedGear) -> Result<(), String> {
        let expected = self.contract.kind();
        let base = gear
            .base
            .as_ref()
            .ok_or("native digital lacks a selected Base")?;
        let [call] = gear.host_calls.as_slice() else {
            return Err("native digital requires one exact Host Call".into());
        };
        if gear.implementation_id != self.implementation
            || gear.execution_profile_id.as_str() != DIGITAL_EXECUTION_PROFILE
            || !self.capability_ids.contains(&gear.capability_id)
            || gear.kind_id != expected.kind_id
            || gear.kind_contract_revision != expected.kind_contract_revision
            || gear.inputs != expected.inputs
            || gear.outputs != expected.outputs
            || gear.semantic_contract != expected.semantic_contract()
            || gear.limits != expected.limits
            || !gear.configuration.is_empty()
            || base.implementation_id.as_str() != "conduitos.base/digital-controller@1"
            || base.mechanism_family.as_str() != DIGITAL_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || call.contract_id.as_str() != DIGITAL_CALL
            || call.target_kind.as_ref() != Some(&expected.kind_id)
            || call.maximum_in_flight != 1
            || call.maximum_input_bytes != DIGITAL_MAXIMUM_BYTES
            || call.maximum_output_bytes != DIGITAL_MAXIMUM_BYTES
            || !gear.resources.iter().any(|resource| {
                resource.class_id.as_str() == DIGITAL_ATTACHMENT_CLASS && resource.units == 1
            })
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == DIGITAL_AUTHORITY
                    && authority.host_call_contract_id.as_str() == DIGITAL_CALL
                    && authority.subject_kind == expected.kind_id
                    && authority.host_id == gear.host_id
                    && authority.boot_id == gear.boot_id
                    && authority.capability_id == gear.capability_id
            })
        {
            return Err("native digital differs from the exact installed realization".into());
        }
        Ok(())
    }
}
impl KernelOperationFactory for DigitalOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.validate(gear)?;
        Ok(KernelOperationBudget {
            value_items: 4,
            value_bytes: DIGITAL_MAXIMUM_BYTES * 4,
            maximum_value_bytes: DIGITAL_MAXIMUM_BYTES,
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
        Ok(Box::new(HostCallBack::new(DIGITAL_MAXIMUM_BYTES)))
    }
}

#[cfg(test)]
mod tests;
