//! Shared-kernel lifecycle for the exact selected native I2C transaction.
use super::{
    contract::{I2C_CALL, I2C_MAXIMUM_BYTES, I2cContract},
    installation::{
        I2C_ATTACHMENT_CLASS, I2C_AUTHORITY, I2C_EXECUTION_PROFILE, I2C_IMPLEMENTATION,
    },
};
use alloc::{boxed::Box, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{
    HostedValueStore,
    scheduler::{HostCallBack, StepBack},
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

/// Preparing the scheduler Back neither grants possession nor touches a controller.
/// Actual dispatch still crosses the exact selected I2cHostCall owner.
pub struct I2cOperationFactory {
    implementation: ImplementationId,
    contract: I2cContract,
}
impl I2cOperationFactory {
    pub fn prepare_contract() -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            implementation: ImplementationId::from(I2C_IMPLEMENTATION),
            contract: I2cContract::prepare()?,
        })
    }
    fn validate(&self, gear: &PlannedGear) -> Result<(), String> {
        let expected = self.contract.kind();
        let base = gear
            .base
            .as_ref()
            .ok_or("native I2C lacks a selected Base")?;
        let [call] = gear.host_calls.as_slice() else {
            return Err("native I2C requires one exact Host Call".into());
        };
        if gear.implementation_id != self.implementation
            || gear.execution_profile_id.as_str() != I2C_EXECUTION_PROFILE
            || gear.capability_id.as_str() != "conduitos/i2c-transaction@1"
            || gear.kind_id != expected.kind_id
            || gear.kind_contract_revision != expected.kind_contract_revision
            || gear.inputs != expected.inputs
            || gear.outputs != expected.outputs
            || gear.semantic_contract != expected.semantic_contract()
            || gear.limits != expected.limits
            || !gear.configuration.is_empty()
            || base.implementation_id.as_str() != "conduitos.base/i2c-controller@1"
            || base.mechanism_family.as_str() != I2C_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || call.contract_id.as_str() != I2C_CALL
            || call.target_kind.as_ref() != Some(&expected.kind_id)
            || call.maximum_in_flight != 1
            || call.maximum_input_bytes != I2C_MAXIMUM_BYTES
            || call.maximum_output_bytes != I2C_MAXIMUM_BYTES
            || !gear.resources.iter().any(|resource| {
                resource.class_id.as_str() == I2C_ATTACHMENT_CLASS && resource.units == 1
            })
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == I2C_AUTHORITY
                    && authority.host_call_contract_id.as_str() == I2C_CALL
                    && authority.subject_kind == expected.kind_id
                    && authority.host_id == gear.host_id
                    && authority.boot_id == gear.boot_id
                    && authority.capability_id == gear.capability_id
            })
        {
            return Err("native I2C differs from the exact installed realization".into());
        }
        Ok(())
    }
}
impl KernelOperationFactory for I2cOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.validate(gear)?;
        Ok(KernelOperationBudget {
            value_items: 4,
            value_bytes: I2C_MAXIMUM_BYTES * 4,
            maximum_value_bytes: I2C_MAXIMUM_BYTES,
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
        Ok(Box::new(HostCallBack::new(I2C_MAXIMUM_BYTES)))
    }
}

#[cfg(test)]
mod tests;
