//! Exact ordinary text/literal factory, reusing the existing one-shot Step Back.
use crate::text_kernel_backs::{LiteralBack, LiteralState};
use alloc::{boxed::Box, format, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{CapabilityOffer, ConfigurationValue, ImplementationId, PlannedGear};
use conduit_kernel::{HostedValueStore, ValueStorage, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub struct TextLiteralOperationFactory {
    offer: CapabilityOffer,
}
impl Default for TextLiteralOperationFactory {
    fn default() -> Self {
        Self {
            offer: crate::functional_offers::text_literal_offer(),
        }
    }
}
impl TextLiteralOperationFactory {
    /// The existing pure offer, without manufacturing Host Calls or Base authority.
    pub fn offer(&self) -> &CapabilityOffer {
        &self.offer
    }
    fn selected<'a>(&self, gear: &'a PlannedGear) -> Result<&'a str, String> {
        let expected = &self.offer;
        if gear.kind_id != expected.kind_id
            || gear.kind_contract_revision != expected.kind_contract_revision
            || gear.capability_id != expected.capability_id
            || gear.execution_profile_id != expected.implementation.execution_profile_id
            || gear.implementation_id != expected.implementation.implementation_id
            || gear.artifact_id != expected.implementation.artifact_id
            || gear.inputs != expected.inputs
            || gear.outputs != expected.outputs
            || gear.semantic_contract != expected.semantic_contract
            || gear.limits != expected.limits
            || !gear.realization_properties.is_empty()
            || !gear.terminal_transductions.is_empty()
            || !gear.host_calls.is_empty()
            || gear.base.is_some()
            || !gear.resources.is_empty()
            || !gear.authority.is_empty()
            || !gear.pool_references.is_empty()
        {
            return Err("native text literal differs from its exact pure realization".into());
        }
        let [entry] = gear.configuration.as_slice() else {
            return Err("native text literal requires exactly one value configuration".into());
        };
        let ConfigurationValue::Text(value) = &entry.value else {
            return Err("native text literal value must be Text".into());
        };
        if entry.key != "value" || value.len() > conduit_text::MAX_TEXT_BYTES as usize {
            return Err("native text literal configuration exceeds its Text envelope".into());
        }
        Ok(value)
    }
}
impl KernelOperationFactory for TextLiteralOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.offer.implementation.implementation_id
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let bytes = (self.selected(gear)?.len() as u32).max(1);
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: bytes,
            maximum_value_bytes: bytes,
            host_requests: 0,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        let text = values
            .store(self.selected(gear)?.as_bytes())
            .map_err(|error| format!("{error:?}"))?;
        Ok(Box::new(LiteralBack {
            text,
            state: LiteralState::Emitting,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use conduit_core::*;
    fn gear(text: &str) -> PlannedGear {
        let expected = crate::functional_offers::text_literal_offer();
        conduit_core::planned_gear_from_parts! {
            placement_id: "literal-placement".into(),
            gear_id: "literal".into(),
            kind_id: expected.kind_id,
            kind_contract_revision: expected.kind_contract_revision,
            execution_profile_id: expected.implementation.execution_profile_id,
            configuration: vec![ConfigurationEntry {
                key: "value".into(),
                value: ConfigurationValue::Text(text.into()),
            }],
            host_id: "literal-host".into(),
            boot_id: "literal-boot".into(),
            offer_generation: OfferGeneration(1),
            capability_id: expected.capability_id,
            implementation_id: expected.implementation.implementation_id,
            artifact_id: expected.implementation.artifact_id,
            base: None,
            realization_characteristics: vec![],
            limits: expected.limits,
            inputs: expected.inputs,
            outputs: expected.outputs,
            semantic_contract: expected.semantic_contract,
            terminal_transductions: vec![],
            host_calls: vec![],
            resources: vec![],
            authority: vec![],
            pool_references: vec![],
        }
    }
    #[test]
    fn one_prepared_value_has_no_host_calls_and_exact_utf8_bytes() {
        let factory = TextLiteralOperationFactory::default();
        let gear = gear("naïve 🦀");
        let budget = factory.budget(&gear).unwrap();
        assert_eq!(budget.value_items, 1);
        assert_eq!(budget.host_requests, 0);
        assert_eq!(budget.value_bytes, "naïve 🦀".len() as u32);
        let mut values =
            HostedValueStore::new(1, budget.maximum_value_bytes, budget.value_bytes).unwrap();
        let _back = factory.prepare(&gear, &mut values).unwrap();
        assert_eq!(values.used_items(), 1);
        assert_eq!(values.used_bytes(), budget.value_bytes);
        assert!(factory.prepare(&gear, &mut values).is_err());
    }
    #[test]
    fn empty_literal_keeps_a_real_finite_value_slot() {
        let factory = TextLiteralOperationFactory::default();
        let gear = gear("");
        let budget = factory.budget(&gear).unwrap();
        assert_eq!(budget.value_bytes, 1);
        let mut values = HostedValueStore::new(1, 1, 1).unwrap();
        factory.prepare(&gear, &mut values).unwrap();
        assert_eq!(values.used_items(), 1);
        assert_eq!(values.used_bytes(), 0);
    }
    #[test]
    fn changed_front_and_selected_implementation_are_refused_before_storage() {
        let factory = TextLiteralOperationFactory::default();
        let mut changed = gear("x");
        changed.outputs[0].value_kind = kind_id("value/u64");
        assert!(factory.budget(&changed).is_err());
        changed = gear("x");
        changed.artifact_id = "another-artifact".into();
        assert!(factory.budget(&changed).is_err());
        changed = gear("x");
        changed.limits.max_queue_bytes += 1;
        assert!(factory.budget(&changed).is_err());
        let mut values = HostedValueStore::new(1, 1, 1).unwrap();
        assert!(factory.prepare(&changed, &mut values).is_err());
        assert_eq!(values.used_items(), 0);
    }
    #[test]
    fn missing_duplicate_unknown_and_oversize_configuration_refuse() {
        let factory = TextLiteralOperationFactory::default();
        let mut changed = gear("x");
        changed.configuration.clear();
        assert!(factory.budget(&changed).is_err());
        changed = gear("x");
        changed.configuration.push(changed.configuration[0].clone());
        assert!(factory.budget(&changed).is_err());
        changed = gear("x");
        changed.configuration[0].key = "other".into();
        assert!(factory.budget(&changed).is_err());
        assert!(
            factory
                .budget(&gear(
                    &"x".repeat(conduit_text::MAX_TEXT_BYTES as usize + 1)
                ))
                .is_err()
        );
    }
    #[test]
    fn foreign_effect_requirement_cannot_turn_literal_into_a_host_call() {
        let factory = TextLiteralOperationFactory::default();
        let mut changed = gear("x");
        changed.host_calls = crate::functional_offers::text_upper_offer().host_calls;
        assert!(factory.budget(&changed).is_err());
    }
}
