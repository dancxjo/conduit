//! Finite scheduler Back for the exact selected class-neutral isochronous OUT Kind.
use super::contract::{
    ISOCHRONOUS_OUT_CALL, ISOCHRONOUS_OUT_MAXIMUM_BYTES, IsochronousOutContract,
};
use alloc::{boxed::Box, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{
    HostedValueStore,
    scheduler::{HostCallBack, StepBack},
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub const ISOCHRONOUS_OUT_IMPLEMENTATION: &str = "conduitos/usb-isochronous-out@1";
pub const ISOCHRONOUS_OUT_PROFILE: &str = "conduitos/usb-isochronous-out-cooperative-bounded@1";
pub const ISOCHRONOUS_OUT_BASE: &str = "conduitos.base/usb-controller@1";
pub const ISOCHRONOUS_OUT_ATTACHMENT: &str = "machine/usb/isochronous-out-attachment";
pub const ISOCHRONOUS_OUT_AUTHORITY: &str = "conduitos.authority/usb-isochronous-out@1";

/// Kernel preparation does not issue possession or select a physical device.
/// Native effects must still pass the independently bound IsochronousOutCallOwner.
pub struct IsochronousOutOperationFactory {
    implementation: ImplementationId,
    contract: IsochronousOutContract,
}
impl IsochronousOutOperationFactory {
    pub fn prepare_contract() -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            implementation: ISOCHRONOUS_OUT_IMPLEMENTATION.into(),
            contract: IsochronousOutContract::prepare()?,
        })
    }
    fn validate(&self, gear: &PlannedGear) -> Result<(), String> {
        let kind = self.contract.kind();
        let base = gear
            .base
            .as_ref()
            .ok_or("USB isochronous OUT lacks a selected Base")?;
        let [call] = gear.host_calls.as_slice() else {
            return Err("USB isochronous OUT requires exactly one Host Call".into());
        };
        if gear.implementation_id != self.implementation
            || gear.execution_profile_id.as_str() != ISOCHRONOUS_OUT_PROFILE
            || gear.capability_id.as_str().is_empty()
            || gear.kind_id != kind.kind_id
            || gear.kind_contract_revision != kind.kind_contract_revision
            || gear.inputs != kind.inputs
            || gear.outputs != kind.outputs
            || gear.semantic_contract != kind.semantic_contract()
            || gear.limits.max_active_instances != kind.limits.max_active_instances
            || gear.limits.max_queue_items != kind.limits.max_queue_items
            || gear.limits.max_queue_bytes != kind.limits.max_queue_bytes
            || !gear.configuration.is_empty()
            || base.implementation_id.as_str() != ISOCHRONOUS_OUT_BASE
            || base.mechanism_family.as_str() != ISOCHRONOUS_OUT_ATTACHMENT
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || call.contract_id.as_str() != ISOCHRONOUS_OUT_CALL
            || call.target_kind.as_ref() != Some(&kind.kind_id)
            || call.maximum_in_flight != 1
            || call.maximum_input_bytes != ISOCHRONOUS_OUT_MAXIMUM_BYTES
            || call.maximum_output_bytes != ISOCHRONOUS_OUT_MAXIMUM_BYTES
            || !gear.resources.iter().any(|resource| {
                resource.class_id.as_str() == ISOCHRONOUS_OUT_ATTACHMENT && resource.units == 1
            })
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == ISOCHRONOUS_OUT_AUTHORITY
                    && authority.host_call_contract_id.as_str() == ISOCHRONOUS_OUT_CALL
                    && authority.subject_kind == kind.kind_id
                    && authority.host_id == gear.host_id
                    && authority.boot_id == gear.boot_id
                    && authority.capability_id == gear.capability_id
            })
        {
            return Err("USB isochronous OUT differs from the selected finite realization".into());
        }
        Ok(())
    }
}
impl KernelOperationFactory for IsochronousOutOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.validate(gear)?;
        Ok(KernelOperationBudget {
            value_items: 4,
            value_bytes: ISOCHRONOUS_OUT_MAXIMUM_BYTES * 4,
            maximum_value_bytes: ISOCHRONOUS_OUT_MAXIMUM_BYTES,
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
        Ok(Box::new(HostCallBack::new(ISOCHRONOUS_OUT_MAXIMUM_BYTES)))
    }
}
