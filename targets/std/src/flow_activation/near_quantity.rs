//! Exact installed numerical realization for ordinary Value activation.
use crate::{installed_std, numerical_host::near_step::NearQuantityBack};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory, ReusableKernelOperation};
use conduit_core::{ImplementationId, PlannedGear, QUANTITY_VALUE_MAX_ENCODED_BYTES};
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub struct NearQuantityFactory;
impl KernelOperationFactory for NearQuantityFactory {
    fn implementation_id(&self) -> &ImplementationId {
        static ID: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| installed_std::NEAR_VALUE_IMPLEMENTATION.into())
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        // The same exact installed admission validates Type/law/configuration,
        // numeric profile and sealed scratch/work properties before Play.
        let _ = installed_std::prepare_near_value(placement)?;
        let maximum = NearQuantityBack::output_bound();
        Ok(KernelOperationBudget {
            value_items: 2,
            value_bytes: QUANTITY_VALUE_MAX_ENCODED_BYTES as u32 + maximum,
            maximum_value_bytes: maximum,
            host_requests: 0,
            sign_items: 8,
        })
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        Ok(Box::new(installed_std::prepare_near_value(placement)?))
    }
    fn prepare_reusable_value(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<ReusableKernelOperation, String> {
        Ok(Box::new(installed_std::prepare_near_value(placement)?))
    }
}
