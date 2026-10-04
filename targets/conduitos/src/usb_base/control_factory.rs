//! Finite scheduler Back for the exact selected class-neutral control Kind.
use super::control_contract::{CONTROL_CALL, CONTROL_MAXIMUM_BYTES, ControlContract};
use alloc::{boxed::Box, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{
    HostedValueStore,
    scheduler::{HostCallBack, StepBack},
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub const CONTROL_IMPLEMENTATION: &str = "conduitos/usb-control@1";
pub const CONTROL_PROFILE: &str = "conduitos/usb-control-cooperative-bounded@1";
pub const CONTROL_BASE: &str = "conduitos.base/usb-controller@1";
pub const CONTROL_ATTACHMENT: &str = "machine/usb/control-attachment";
pub const CONTROL_AUTHORITY: &str = "conduitos.authority/usb-control@1";

/// Kernel preparation does not issue possession or select a physical device.
/// Native effects must still pass the independently bound ControlCallOwner.
pub struct ControlOperationFactory {
    implementation: ImplementationId,
    contract: ControlContract,
}
impl ControlOperationFactory {
    pub fn prepare_contract() -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            implementation: CONTROL_IMPLEMENTATION.into(),
            contract: ControlContract::prepare()?,
        })
    }
    fn validate(&self, gear: &PlannedGear) -> Result<(), String> {
        let kind = self.contract.kind();
        let base = gear
            .base
            .as_ref()
            .ok_or("USB control lacks a selected Base")?;
        let [call] = gear.host_calls.as_slice() else {
            return Err("USB control requires exactly one Host Call".into());
        };
        if gear.implementation_id != self.implementation
            || gear.execution_profile_id.as_str() != CONTROL_PROFILE
            || gear.capability_id.as_str() != CONTROL_IMPLEMENTATION
            || gear.kind_id != kind.kind_id
            || gear.kind_contract_revision != kind.kind_contract_revision
            || gear.inputs != kind.inputs
            || gear.outputs != kind.outputs
            || gear.semantic_contract != kind.semantic_contract()
            || gear.limits != kind.limits
            || !gear.configuration.is_empty()
            || base.implementation_id.as_str() != CONTROL_BASE
            || base.mechanism_family.as_str() != CONTROL_ATTACHMENT
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || call.contract_id.as_str() != CONTROL_CALL
            || call.target_kind.as_ref() != Some(&kind.kind_id)
            || call.maximum_in_flight != 1
            || call.maximum_input_bytes != CONTROL_MAXIMUM_BYTES
            || call.maximum_output_bytes != CONTROL_MAXIMUM_BYTES
            || !gear.resources.iter().any(|resource| {
                resource.class_id.as_str() == CONTROL_ATTACHMENT && resource.units == 1
            })
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == CONTROL_AUTHORITY
                    && authority.host_call_contract_id.as_str() == CONTROL_CALL
                    && authority.subject_kind == kind.kind_id
                    && authority.host_id == gear.host_id
                    && authority.boot_id == gear.boot_id
                    && authority.capability_id == gear.capability_id
            })
        {
            return Err("USB control differs from the selected finite realization".into());
        }
        Ok(())
    }
}
impl KernelOperationFactory for ControlOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.validate(gear)?;
        Ok(KernelOperationBudget {
            value_items: 4,
            value_bytes: CONTROL_MAXIMUM_BYTES * 4,
            maximum_value_bytes: CONTROL_MAXIMUM_BYTES,
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
        Ok(Box::new(HostCallBack::new(CONTROL_MAXIMUM_BYTES)))
    }
}

#[cfg(test)]
mod tests;
