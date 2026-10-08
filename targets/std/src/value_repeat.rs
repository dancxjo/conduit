//! Host-owned realization facade over the shared allocation-prepared owner.
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
use conduit_semantic_catalog::operation_owners::value_repeat as common;
use std::sync::Arc;
pub struct PreparedValueRepeat {
    inner: Arc<common::PreparedValueRepeat>,
}
impl PreparedValueRepeat {
    pub fn new(value: CheckedValueContract, schema: StructuredInfoType) -> Result<Self, String> {
        let offer =
            conduit_std_offers::value_repeat_offer(&value, &schema).map_err(String::from)?;
        Ok(Self {
            inner: Arc::new(common::PreparedValueRepeat::new(value, schema, offer)?),
        })
    }
    pub fn capacity2(
        value: CheckedValueContract,
        schema: StructuredInfoType,
    ) -> Result<Self, String> {
        let offer = conduit_std_offers::value_repeat_capacity2_offer(&value, &schema)
            .map_err(String::from)?;
        Ok(Self {
            inner: Arc::new(common::PreparedValueRepeat::new(value, schema, offer)?),
        })
    }
    pub fn offer(&self) -> &CapabilityOffer {
        self.inner.offer()
    }
    pub fn schema(&self) -> &StructuredInfoType {
        self.inner.schema()
    }
}
pub struct ValueRepeatOperationFactory {
    inner: common::ValueRepeatOperationFactory,
}
impl ValueRepeatOperationFactory {
    pub fn for_plan(plan: &Plan, profiles: &[Arc<PreparedValueRepeat>]) -> Result<Self, String> {
        let retained: Vec<_> = profiles
            .iter()
            .map(|profile| profile.inner.clone())
            .collect();
        Ok(Self {
            inner: common::ValueRepeatOperationFactory::for_plan(
                plan,
                &ImplementationId::from(conduit_std_offers::VALUE_REPEAT_IMPLEMENTATION),
                &retained,
            )?,
        })
    }
}
impl KernelOperationFactory for ValueRepeatOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        self.inner.implementation_id()
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.inner.budget(gear)
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        self.inner.prepare(gear, values)
    }
}
