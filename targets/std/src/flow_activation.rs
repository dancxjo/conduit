//! Receipt-backed installation of planned Flow activations on one std host.

use conduit_composite::{
    KernelCompositeHost, KernelOperationRegistry, PlannedActivationChildPoolHost,
    PlannedActivationCompositeError, PreparedActivationChildPool,
    PreparedPlannedActivationComposite,
};
use conduit_core::{
    ActivePlayId, HostPreparationRefusal, Plan, PlanFragment, PlanPreparationHost,
    PreparationHostIdentity, PreparedFragmentReceipt, PreparedPlan,
};

mod pure_todo_scan;
mod todo_combine;
mod todo_scan_offer;
pub use pure_todo_scan::install_pure_todo_scan;
pub(crate) use todo_combine::maximum_scan_child_steps;
pub use todo_combine::{offer as todo_combine_offer, TodoCombineFactory};
#[cfg(test)]
pub(crate) use todo_scan_offer::tests::authored_todo_plan;
pub use todo_scan_offer::todo_scan_offer;
pub(crate) use todo_scan_offer::validate_planned_todo_scan;

/// Production child operations that the std activation host can actually
/// prepare. The scan coordinator is selected by the whole Plan, not by this
/// registry of child Backs.
pub fn standard_child_registry() -> Result<KernelOperationRegistry, String> {
    let mut registry = KernelOperationRegistry::new();
    registry.install(TodoCombineFactory)?;
    Ok(registry)
}

/// One current std boot's installed kernel implementations and prepared
/// fragment reservations. This is preparation machinery, not a scheduler.
pub struct StdActivationHost {
    identity: PreparationHostIdentity,
    registry: KernelOperationRegistry,
    prepared: Vec<PreparedFragmentReceipt>,
}

impl StdActivationHost {
    pub fn new(identity: PreparationHostIdentity, registry: KernelOperationRegistry) -> Self {
        Self {
            identity,
            registry,
            prepared: Vec::new(),
        }
    }

    pub fn prepared_receipts(&self) -> &[PreparedFragmentReceipt] {
        &self.prepared
    }
}

/// Install one activation from an already prepared whole Plan. The mutable
/// prepared Plan makes the subordinate receipt transfer single-use.
pub fn install_planned_activation(
    plan: &Plan,
    prepared: &mut PreparedPlan,
    activation_id: &str,
    host: &mut StdActivationHost,
) -> Result<PreparedPlannedActivationComposite, Box<PlannedActivationCompositeError>> {
    let children =
        PreparedActivationChildPool::prepare_on_host(plan, prepared, activation_id, host)
            .map_err(Box::new)?;
    PreparedPlannedActivationComposite::prepare(plan, activation_id, children).map_err(Box::new)
}

impl PlanPreparationHost for StdActivationHost {
    fn preparation_identity(&self) -> PreparationHostIdentity {
        self.identity.clone()
    }

    fn prepare_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<PreparedFragmentReceipt, HostPreparationRefusal> {
        if fragment.host_id != self.identity.host_id {
            return Err(HostPreparationRefusal::WrongHost);
        }
        if fragment.boot_id != self.identity.boot_id {
            return Err(HostPreparationRefusal::StaleBoot);
        }
        if fragment.offer_generation != self.identity.offer_generation {
            return Err(HostPreparationRefusal::StaleOffer);
        }
        let receipt = PreparedFragmentReceipt::new(fragment);
        if self.prepared.contains(&receipt) {
            return Err(HostPreparationRefusal::AlreadyPrepared);
        }
        self.prepared.push(receipt.clone());
        Ok(receipt)
    }

    fn release_fragment(
        &mut self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        let index = self
            .prepared
            .iter()
            .position(|candidate| candidate == receipt)
            .ok_or(HostPreparationRefusal::NotPrepared)?;
        self.prepared.remove(index);
        Ok(())
    }

    fn validate_start(
        &self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        if receipt.host() != &self.identity {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        }
        self.prepared
            .contains(receipt)
            .then_some(())
            .ok_or(HostPreparationRefusal::NotPrepared)
    }

    fn start_fragment(&mut self, _receipt: &PreparedFragmentReceipt) -> ActivePlayId {
        ActivePlayId::from("std-activation-play")
    }
}

impl PlannedActivationChildPoolHost for StdActivationHost {
    fn take_activation_child_pool(
        &mut self,
        receipts: &[PreparedFragmentReceipt],
        definition: &conduit_composite::KernelCompositeDefinition,
        maximum_items: usize,
    ) -> Result<Vec<KernelCompositeHost>, HostPreparationRefusal> {
        if receipts.is_empty()
            || receipts
                .iter()
                .any(|receipt| receipt.host() != &self.identity || !self.prepared.contains(receipt))
        {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        }
        let mut children = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            children.push(
                KernelCompositeHost::prepare(definition.clone(), &self.registry)
                    .map_err(|_| HostPreparationRefusal::ImplementationUnavailable)?,
            );
        }
        for receipt in receipts {
            let index = self
                .prepared
                .iter()
                .position(|candidate| candidate == receipt)
                .ok_or(HostPreparationRefusal::PreparedBindingMismatch)?;
            self.prepared.remove(index);
        }
        Ok(children)
    }
}

#[cfg(test)]
pub(crate) mod tests;
