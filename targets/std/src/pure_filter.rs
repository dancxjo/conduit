//! Std realization facade for the shared checked Source filter owner.
pub use common::PreparedPureFilter;
use conduit_core::Plan;
use conduit_semantic_catalog::operation_owners::pure_filter as common;
pub use conduit_std_offers::{
    pure_filter_std_offer as offer, PURE_FILTER_STD_IMPLEMENTATION as IMPLEMENTATION,
};
pub struct PureFilterOperationFactory(common::PureFilterOperationFactory);
impl PureFilterOperationFactory {
    pub fn for_plan(plan: &Plan) -> Result<Self, String> {
        common::PureFilterOperationFactory::for_plan(plan, IMPLEMENTATION.into(), offer).map(Self)
    }
    pub fn prepare_host(
        &self,
        gear: &conduit_core::PlannedGear,
    ) -> Result<PreparedPureFilter, String> {
        self.0.prepare_host(gear)
    }
}
impl conduit_composite::KernelOperationFactory for PureFilterOperationFactory {
    fn implementation_id(&self) -> &conduit_core::ImplementationId {
        conduit_composite::KernelOperationFactory::implementation_id(&self.0)
    }
    fn budget(
        &self,
        gear: &conduit_core::PlannedGear,
    ) -> Result<conduit_composite::KernelOperationBudget, String> {
        conduit_composite::KernelOperationFactory::budget(&self.0, gear)
    }
    fn prepare(
        &self,
        gear: &conduit_core::PlannedGear,
        values: &mut conduit_kernel::HostedValueStore,
    ) -> Result<
        Box<
            dyn conduit_kernel::scheduler::StepBack<
                    { conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE },
                > + Send,
        >,
        String,
    > {
        conduit_composite::KernelOperationFactory::prepare(&self.0, gear, values)
    }
}
