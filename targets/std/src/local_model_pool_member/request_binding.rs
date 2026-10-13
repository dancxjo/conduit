//! Checked request preparation before native member resource admission.
//! This binds meaning and bytes, never selects a provider or changes the Plan.
use conduit_core::{CapabilityOffer, CheckedFront, ConfigurationValue, PlannedGear};
use conduit_planner::resource_cost::CostDemand;

pub(super) struct RequestBinding {
    bytes: u32,
    digest: [u8; 32],
}
impl RequestBinding {
    pub(super) fn admit(
        demand: CostDemand<'_>,
        sealed_front: &CheckedFront,
        offer: &CapabilityOffer,
        limits: &conduit_ai::LlmWorkBounds,
        placement: &mut PlannedGear,
    ) -> Result<Self, String> {
        let gear = demand.class().gear();
        if gear.checked_front() != *sealed_front
            || !gear.accepts_realization(offer)
            || gear.validate_constructed_semantic_contract().is_err()
            || gear.inputs.len() != 1
            || demand.inputs().len() != 1
            || demand.inputs()[0].port != &gear.inputs[0].port_id
        {
            return Err("local-model demand differs from the sealed Fore or selected Kind".into());
        }
        // Local-model startup values are five bounded Count coordinates. Check
        // ordinary schema/type rules before retaining any caller-supplied value.
        if gear.configuration.len() != gear.semantic_contract.configuration.len()
            || gear.configuration.len() > 16
        {
            return Err("local-model demand configuration width changed".into());
        }
        for field in &gear.semantic_contract.configuration {
            let mut matching = gear
                .configuration
                .iter()
                .filter(|entry| entry.key == field.key);
            let entry = matching
                .next()
                .ok_or("local-model demand configuration is incomplete")?;
            if matching.next().is_some()
                || !gear.startup_parameters.iter().any(|parameter| {
                    parameter.name == field.key
                        && parameter.value_type == entry.value.semantic_kind()
                })
                || conduit_plot::validate_configuration_value(field, &entry.value).is_err()
            {
                return Err("local-model demand configuration is invalid".into());
            }
        }
        let count = |key: &str| -> Result<u64, String> {
            gear.configuration
                .iter()
                .find_map(|entry| {
                    (entry.key == key)
                        .then_some(&entry.value)
                        .and_then(|value| match value {
                            ConfigurationValue::U64(value) => Some(*value),
                            _ => None,
                        })
                })
                .ok_or_else(|| "local-model demand is missing an exact work bound".into())
        };
        let requested = [
            count("maximum-input-bytes")?,
            count("maximum-context-items")?,
            count("maximum-output-bytes")?,
            count("maximum-work-units")?,
            count("maximum-history-items")?,
        ];
        let offered = [
            limits.maximum_input_bytes(),
            limits.maximum_context_items(),
            limits.maximum_output_bytes(),
            limits.maximum_work_units(),
            limits.maximum_history_items(),
        ];
        if requested
            .iter()
            .zip(offered)
            .any(|(request, capacity)| *request > capacity)
            || demand.input_bytes() as u64 > requested[0]
        {
            return Err("local-model demand exceeds initialized work capacity".into());
        }
        let value = demand.inputs()[0].value;
        let binding = Self {
            bytes: u32::try_from(value.encoded.len())
                .map_err(|_| "local-model demand extent overflowed")?,
            digest: conduit_core::semantic_digest(
                "conduit.std/local-model-completed-input@1",
                &value.encoded,
            ),
        };
        // Retain the checked meaning used by the forecast, not offer defaults.
        placement.configuration.clone_from(&gear.configuration);
        placement
            .semantic_contract
            .clone_from(&gear.semantic_contract);
        placement
            .terminal_transductions
            .clone_from(&gear.terminal_transductions);
        Ok(binding)
    }
    pub(super) fn matches(&self, identity: Option<(u32, [u8; 32])>) -> bool {
        identity == Some((self.bytes, self.digest))
    }
}

#[cfg(test)]
#[path = "request_binding_tests.rs"]
mod tests;
