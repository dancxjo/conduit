//! Receipt-backed installation of planned Flow activations in one browser Host.

use crate::installed_browser::{factory, BrowserBack, BrowserInstallation};
use conduit_composite::{
    KernelCompositeHost, KernelOperationBudget, KernelOperationFactory, KernelOperationRegistry,
    PlannedActivationChildPoolHost, PlannedActivationCompositeError, PreparedActivationChildPool,
    PreparedPlannedActivationComposite,
};
use conduit_core::{
    ActivePlayId, HostPreparationRefusal, Plan, PlanFragment, PlanPreparationHost,
    PlannedActivationEntry, PreparationHostIdentity, PreparedFragmentReceipt, PreparedPlan,
};
use conduit_kernel::scheduler::StepBack;
use conduit_kernel::HostedValueStore;
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::collections::BTreeSet;

pub struct BrowserActivationHost {
    identity: PreparationHostIdentity,
    registry: KernelOperationRegistry,
    prepared: Vec<PreparedFragmentReceipt>,
}

impl BrowserActivationHost {
    /// Install only the browser Backs selected by exact activation children.
    pub fn for_plan(identity: PreparationHostIdentity, plan: &Plan) -> Result<Self, String> {
        let mut registry = KernelOperationRegistry::new();
        let mut installed = BTreeSet::new();
        for placement in plan
            .activations
            .iter()
            .map(selected_plan)
            .flat_map(|plan| &plan.fragments)
            .flat_map(|fragment| &fragment.placements)
        {
            if installed.insert(placement.implementation_id.clone()) {
                let installation = factory(&placement.implementation_id).ok_or_else(|| {
                    format!(
                        "planned browser implementation '{}' is not installed",
                        placement.implementation_id.as_str()
                    )
                })?;
                registry.install(BrowserActivationFactory {
                    implementation_id: placement.implementation_id.clone(),
                    installation,
                })?;
            }
        }
        Ok(Self {
            identity,
            registry,
            prepared: Vec::new(),
        })
    }

    pub fn prepared_receipts(&self) -> &[PreparedFragmentReceipt] {
        &self.prepared
    }

    #[cfg(test)]
    fn with_registry(identity: PreparationHostIdentity, registry: KernelOperationRegistry) -> Self {
        Self {
            identity,
            registry,
            prepared: Vec::new(),
        }
    }
}

pub fn install_planned_activation(
    plan: &Plan,
    prepared: &mut PreparedPlan,
    activation_id: &str,
    host: &mut BrowserActivationHost,
) -> Result<PreparedPlannedActivationComposite, Box<PlannedActivationCompositeError>> {
    let children =
        PreparedActivationChildPool::prepare_on_host(plan, prepared, activation_id, host)
            .map_err(Box::new)?;
    PreparedPlannedActivationComposite::prepare(plan, activation_id, children).map_err(Box::new)
}

struct BrowserActivationFactory {
    implementation_id: conduit_core::ImplementationId,
    installation: &'static BrowserInstallation,
}

impl KernelOperationFactory for BrowserActivationFactory {
    fn implementation_id(&self) -> &conduit_core::ImplementationId {
        &self.implementation_id
    }

    fn budget(
        &self,
        placement: &conduit_core::PlannedGear,
    ) -> Result<KernelOperationBudget, String> {
        let maximum_value_bytes = placement
            .inputs
            .iter()
            .chain(&placement.outputs)
            .map(|_| placement.limits.max_queue_bytes)
            .chain(
                placement
                    .host_calls
                    .iter()
                    .flat_map(|call| [call.maximum_input_bytes, call.maximum_output_bytes]),
            )
            .max()
            .unwrap_or(1)
            .max(1);
        let host_requests = placement.host_calls.iter().try_fold(0u16, |total, call| {
            total
                .checked_add(call.maximum_in_flight)
                .ok_or_else(|| "browser activation Host Call bound overflow".to_string())
        })?;
        let retained_items = placement.limits.max_queue_items.saturating_add(8);
        Ok(KernelOperationBudget {
            value_items: retained_items,
            value_bytes: maximum_value_bytes
                .checked_mul(u32::from(retained_items))
                .ok_or_else(|| "browser activation value byte bound overflow".to_string())?,
            maximum_value_bytes,
            host_requests,
            sign_items: 8,
        })
    }

    fn prepare(
        &self,
        placement: &conduit_core::PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        let back: BrowserBack = (self.installation.prepare)(placement, values)?;
        Ok(Box::new(back))
    }
}

impl PlanPreparationHost for BrowserActivationHost {
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
        ActivePlayId::from("browser-activation-play")
    }
}

impl PlannedActivationChildPoolHost for BrowserActivationHost {
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
        let children = (0..maximum_items)
            .map(|_| {
                KernelCompositeHost::prepare(definition.clone(), &self.registry)
                    .map_err(|_| HostPreparationRefusal::ImplementationUnavailable)
            })
            .collect::<Result<Vec<_>, _>>()?;
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

fn selected_plan(entry: &PlannedActivationEntry) -> &Plan {
    match entry {
        PlannedActivationEntry::Unary(value) => &value.selected_plan,
        PlannedActivationEntry::Fold(value) => &value.selected_plan,
        PlannedActivationEntry::Scan(value) => &value.selected_plan,
    }
}

#[cfg(test)]
mod tests;
