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

mod value_scan;
pub use value_scan::{
    install_value_scan, value_scan_child_steps, value_scan_offer, VALUE_SCAN_ARTIFACT,
    VALUE_SCAN_IMPLEMENTATION, VALUE_SCAN_MAXIMUM_ITEMS, VALUE_SCAN_PROFILE,
};

mod value_pair;
pub use value_pair::PreparedValuePairFactory;

mod local_expression;
mod native_u64_expression;
pub use local_expression::{LocalExpressionFactory, PreparedLocalExpressionBack};
pub use native_u64_expression::PreparedNativeU64ExpressionBack;

mod parsec_combine;
mod parsec_combine_back;
mod parsec_input_instruction_plan;
pub(crate) mod parsec_instruction_source_offer;
mod parsec_scan_offer;
mod parsec_source_plan;
pub use parsec_input_instruction_plan::{
    required_parsec_input_instruction_offer, ParsecInputInstructionSourcePlan,
};
mod spoken_template_plan;
pub(crate) use spoken_template_plan::prepare_wording_for_offer;
pub use spoken_template_plan::{required_spoken_template_offers, SpokenTemplateSourcePlan};
mod pure_parsec_scan;
pub use parsec_combine::{
    command_kind as parsec_command_kind, parsec_combine_kind, parsec_combine_offer,
    result_kind as parsec_result_kind, ParsecCombineFactory,
};
pub(crate) use parsec_scan_offer::validate_planned_parsec_scan;
pub use parsec_scan_offer::{
    install_parsec_scan_catalogs, parsec_initial_bytes, parsec_initial_result, parsec_scan_offer,
    PARSEC_SCAN_SOURCE,
};
pub use parsec_source_plan::{
    required_parsec_input_policy_offer, required_parsec_source_offers, ParsecInputPolicySourcePlan,
    ParsecSourcePlans,
};
pub use pure_parsec_scan::install_pure_parsec_scan;

mod pure_thermostat_scan;
mod thermostat_combine;
mod thermostat_scan_offer;
pub use pure_thermostat_scan::install_pure_thermostat_scan;
pub use thermostat_combine::{offer as thermostat_combine_offer, ThermostatCombineFactory};
#[cfg(test)]
pub(crate) use thermostat_scan_offer::tests::authored_thermostat_plan_on_host;
pub use thermostat_scan_offer::thermostat_scan_offer;
pub(crate) use thermostat_scan_offer::validate_planned_thermostat_scan;
mod pure_todo_scan;
mod todo_combine;
mod todo_scan_offer;
pub use pure_todo_scan::install_pure_todo_scan;
pub(crate) fn maximum_scan_child_steps(
    planned: &conduit_core::PlannedScanActivation,
) -> Result<u32, String> {
    if planned.item_input.value_kind.as_str() == conduit_thermostat_plot::COMMAND_KIND {
        thermostat_combine::maximum_scan_child_steps(planned)
    } else if planned.item_input.value_kind == parsec_command_kind()? {
        parsec_combine::maximum_scan_child_steps(planned)
    } else {
        todo_combine::maximum_scan_child_steps(planned)
    }
}
pub use todo_combine::{offer as todo_combine_offer, TodoCombineFactory};
#[cfg(test)]
pub(crate) use todo_scan_offer::tests::authored_todo_plan;
#[cfg(test)]
pub(crate) use todo_scan_offer::tests::authored_todo_plan_on_host;
pub use todo_scan_offer::todo_scan_offer;
pub(crate) use todo_scan_offer::validate_planned_todo_scan;

/// Production child operations that the std activation host can actually
/// prepare. The scan coordinator is selected by the whole Plan, not by this
/// registry of child Backs.
pub fn standard_child_registry() -> Result<KernelOperationRegistry, String> {
    let mut registry = KernelOperationRegistry::new();
    registry.install(TodoCombineFactory)?;
    registry.install(ThermostatCombineFactory)?;
    registry.install(LocalExpressionFactory)?;
    Ok(registry)
}

/// Scoped registry: Parsec computational source custody must come from the
/// caller's actual Host offers before parent planning/preparation.
pub fn parsec_child_registry(
    sources: std::sync::Arc<ParsecSourcePlans>,
) -> Result<KernelOperationRegistry, String> {
    let mut registry = standard_child_registry()?;
    registry.install(ParsecCombineFactory::new(sources))?;
    Ok(registry)
}

#[cfg(test)]
mod parsec_scan_tests;

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

/// Exact reusable scan handoff; no one-shot fallback or registry lookup at Play.
pub fn install_reusable_planned_scan(
    plan: &Plan,
    prepared: &mut PreparedPlan,
    activation_id: &str,
    host: &mut StdActivationHost,
) -> Result<PreparedPlannedActivationComposite, Box<PlannedActivationCompositeError>> {
    let children =
        PreparedActivationChildPool::prepare_reusable_on_host(plan, prepared, activation_id, host)
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
    fn take_reusable_activation_child(
        &mut self,
        receipts: &[PreparedFragmentReceipt],
        definition: &conduit_composite::KernelCompositeDefinition,
    ) -> Result<KernelCompositeHost, HostPreparationRefusal> {
        let [fragment] = definition.internal_plan.fragments.as_slice() else {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        };
        let expected = PreparedFragmentReceipt::new(fragment);
        if receipts.len() != 1
            || receipts.first() != Some(&expected)
            || expected.host() != &self.identity
            || !self.prepared.contains(&expected)
        {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        }
        let child = KernelCompositeHost::prepare_reusable_value(definition.clone(), &self.registry)
            .map_err(|_| HostPreparationRefusal::ImplementationUnavailable)?;
        for receipt in receipts {
            let index = self
                .prepared
                .iter()
                .position(|candidate| candidate == receipt)
                .ok_or(HostPreparationRefusal::PreparedBindingMismatch)?;
            self.prepared.remove(index);
        }
        Ok(child)
    }

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

mod todo_spoken_template_plan;
pub use todo_spoken_template_plan::{
    required_todo_spoken_template_offers, TodoSpokenTemplateSourcePlan,
};

mod pcm_packing;
pub use pcm_packing::PreparedPcm16PackingBack;

mod pcm_packing_factory;
pub use pcm_packing_factory::Pcm16PackingFactory;
mod value_scan_factory;
pub use value_scan_factory::PreparedValueScanFactory;
