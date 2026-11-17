//! Borrowed semantic validation shared by catalog Kinds and constructed offers.
use super::*;
use alloc::collections::BTreeSet;
mod ports;
pub(crate) use ports::validate_terminal_transduction_ports;
use ports::{
    validate_bounded_collect, validate_resource_ports, validate_terminal_transduction,
    validate_value_contracts,
};

pub(super) struct KindValidation<'a> {
    startup_parameters: &'a [FrontStartupParameter],
    kind_id: &'a KindId,
    kind_contract_revision: &'a KindIdentity,
    inputs: &'a [PortDescriptor],
    outputs: &'a [PortDescriptor],
    configuration: &'a [KindConfigurationField],
    semantic_laws: &'a [KindSemanticLaw],
}
impl<'a> From<&'a Kind> for KindValidation<'a> {
    fn from(kind: &'a Kind) -> Self {
        Self {
            startup_parameters: &kind.startup_parameters,
            kind_id: &kind.kind_id,
            kind_contract_revision: &kind.kind_contract_revision,
            inputs: &kind.inputs,
            outputs: &kind.outputs,
            configuration: &kind.configuration,
            semantic_laws: &kind.semantic_laws,
        }
    }
}
impl<'a> From<&'a CapabilityOffer> for KindValidation<'a> {
    fn from(offer: &'a CapabilityOffer) -> Self {
        Self {
            startup_parameters: &offer.startup_parameters,
            kind_id: &offer.kind_id,
            kind_contract_revision: &offer.kind_contract_revision,
            inputs: &offer.inputs,
            outputs: &offer.outputs,
            configuration: &offer.semantic_contract.configuration,
            semantic_laws: &offer.semantic_contract.laws,
        }
    }
}
impl KindValidation<'_> {
    fn value_contracts(&self) -> &[crate::FrontValueContract] {
        self.semantic_laws
            .iter()
            .find_map(|law| match law {
                KindSemanticLaw::ValueContracts(contracts) => Some(contracts.as_slice()),
                _ => None,
            })
            .unwrap_or_default()
    }
    pub(super) fn validate(&self) -> Result<(), KindValidationError> {
        crate::realization_properties::validate_kind_requirements(
            self.configuration,
            self.semantic_laws,
        )?;
        crate::initialized_state::validate_kind(self.inputs, self.outputs, self.semantic_laws)?;
        if self.kind_id.as_str().is_empty() {
            return Err(KindValidationError::EmptyId);
        }
        if self.kind_contract_revision.as_str().is_empty() {
            return Err(KindValidationError::EmptyIdentity);
        }
        if self.inputs.iter().chain(self.outputs).any(|port| {
            port.abnormal_kind
                .as_ref()
                .is_some_and(|kind| kind.as_str().is_empty())
        }) {
            return Err(KindValidationError::EmptyAbnormalTerminalKind);
        }
        let mut port_symbols = BTreeSet::new();
        if self
            .inputs
            .iter()
            .chain(self.outputs)
            .any(|port| !port_symbols.insert(port.port_id.as_str()))
        {
            return Err(KindValidationError::DuplicatePortSymbol);
        }
        let mut keys = BTreeSet::new();
        for field in self.configuration {
            if !keys.insert(field.key.as_str()) {
                return Err(KindValidationError::DuplicateConfigurationKey);
            }
            let Some(front) = self
                .startup_parameters
                .iter()
                .find(|parameter| parameter.name == field.key)
            else {
                return Err(KindValidationError::ConfigurationMissingFromFront);
            };
            // Configuration owns the canonical value and rule. The callable
            // Front independently owns whether authors may omit it.
            let semantic_kind = match (&field.rule, &field.default_value) {
                (crate::KindConfigurationRule::Any, _) => front.value_type.clone(),
                (
                    crate::KindConfigurationRule::QuantityRange { canonical_unit, .. },
                    crate::ConfigurationValue::Quantity(_),
                ) => crate::kind_id(canonical_unit.dimension().info_id()),
                (
                    crate::KindConfigurationRule::DurationMillis { .. },
                    crate::ConfigurationValue::U64(_),
                ) => crate::kind_id(crate::DURATION_INFO_ID),
                _ => field.default_value.semantic_kind(),
            };
            if front.value_type != semantic_kind
                || (matches!(field.rule, crate::KindConfigurationRule::DataLabel { .. })
                    && front.has_default)
            {
                return Err(KindValidationError::ConfigurationFrontMismatch);
            }
        }
        let mut terminal_inputs = BTreeSet::new();
        let mut previous_terminal_input = None;
        let mut cancellation_behavior = None;
        let mut cancellation_request = None;
        let mut resource_ports = None;
        let mut value_contracts = None;
        let mut keyed_join = None;
        let mut bounded_collect = None;
        let mut flow_select = None;
        let mut flow_fold = None;
        let mut flow_each = None;
        let mut flow_scan = None;
        for law in self.semantic_laws {
            match law {
                KindSemanticLaw::TerminalTransduction(profile) => {
                    if !terminal_inputs.insert(profile.input_port_id.as_str()) {
                        return Err(KindValidationError::DuplicateTerminalTransduction);
                    }
                    if previous_terminal_input
                        .is_some_and(|previous: &str| previous >= profile.input_port_id.as_str())
                    {
                        return Err(KindValidationError::NonCanonicalTerminalTransductionOrder);
                    }
                    previous_terminal_input = Some(profile.input_port_id.as_str());
                    validate_terminal_transduction(self, profile)?;
                    if cancellation_behavior
                        .as_ref()
                        .is_some_and(|established| *established != &profile.cancellation)
                    {
                        return Err(KindValidationError::ConflictingCancellationTransduction);
                    }
                    cancellation_behavior = Some(&profile.cancellation);
                    if matches!(
                        profile.cancellation,
                        CancellationTransduction::Request { .. }
                    ) {
                        let request = (&profile.output_port_id, &profile.cancellation);
                        if cancellation_request
                            .as_ref()
                            .is_some_and(|established| *established != request)
                        {
                            return Err(KindValidationError::ConflictingCancellationTransduction);
                        }
                        cancellation_request = Some(request);
                    }
                }
                KindSemanticLaw::ResourcePorts(ports) => {
                    if resource_ports.replace(ports).is_some() {
                        return Err(KindValidationError::DuplicateResourcePorts);
                    }
                    validate_resource_ports(self, ports)?;
                }
                KindSemanticLaw::ValueContracts(contracts) => {
                    if value_contracts.replace(contracts).is_some() {
                        return Err(KindValidationError::DuplicateValueBounds);
                    }
                    validate_value_contracts(self, contracts)?;
                }
                KindSemanticLaw::KeyedJoin(contract) => {
                    if keyed_join.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateKeyedJoin);
                    }
                    if contract.maximum_pending_per_side == 0
                        || contract.key.validate_definition().is_err()
                        || contract.left_value.validate_definition().is_err()
                        || contract.right_value.validate_definition().is_err()
                    {
                        return Err(KindValidationError::InvalidKeyedJoin);
                    }
                }
                KindSemanticLaw::BoundedCollect(contract) => {
                    if bounded_collect.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateBoundedCollect);
                    }
                    validate_bounded_collect(self, contract)?;
                }
                KindSemanticLaw::FlowSelect(contract) => {
                    if flow_select.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateFlowSelect);
                    }
                    let input = self
                        .inputs
                        .iter()
                        .find(|port| port.port_id == contract.input_port_id);
                    let output = self
                        .outputs
                        .iter()
                        .find(|port| port.port_id == contract.output_port_id);
                    if !matches!((input, output), (Some(input), Some(output))
                        if input.temporal == crate::PortTemporal::Flow { closes: true }
                            && output.temporal == crate::PortTemporal::Flow { closes: true }
                            && input.value_kind == output.value_kind
                            && input.value_kind == contract.predicate_input_kind
                            && contract.predicate_output_kind.as_str() == crate::BOOL_INFO_ID
                            && contract.maximum_active == 1
                            && contract.maximum_queued == 1
                            && contract.maximum_items > 0)
                    {
                        return Err(KindValidationError::InvalidFlowSelect);
                    }
                }
                KindSemanticLaw::FlowFold(contract) => {
                    if flow_fold.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateFlowFold);
                    }
                    let input = self
                        .inputs
                        .iter()
                        .find(|port| port.port_id == contract.input_port_id);
                    let output = self
                        .outputs
                        .iter()
                        .find(|port| port.port_id == contract.output_port_id);
                    if !matches!((input, output), (Some(input), Some(output))
                        if input.temporal == crate::PortTemporal::Flow { closes: true }
                            && output.temporal == crate::PortTemporal::Value
                            && input.value_kind == contract.item.value_kind
                            && output.value_kind == contract.accumulator.value_kind
                            && contract.item.validate_definition().is_ok()
                            && contract.accumulator.validate_definition().is_ok()
                            && contract.accumulator.validate(&contract.initial_accumulator).is_ok()
                            && contract.combine_accumulator_port_id.as_str() == "accumulator"
                            && contract.combine_item_port_id.as_str() == "item"
                            && contract.combine_output_port_id.as_str() == "combined"
                            && contract.maximum_active == 1
                            && contract.maximum_queued == 1
                            && contract.maximum_items > 0)
                    {
                        return Err(KindValidationError::InvalidFlowFold);
                    }
                }
                KindSemanticLaw::FlowScan(contract) => {
                    if flow_scan.replace(contract).is_some() {
                        return Err(KindValidationError::DuplicateFlowScan);
                    }
                    let input = self
                        .inputs
                        .iter()
                        .find(|port| port.port_id == contract.input_port_id);
                    let output = self
                        .outputs
                        .iter()
                        .find(|port| port.port_id == contract.output_port_id);
                    if !matches!((input, output), (Some(input), Some(output))
                        if input.temporal == crate::PortTemporal::Flow { closes: true }
                            && output.temporal == crate::PortTemporal::Flow { closes: true }
                            && input.value_kind == contract.item.value_kind
                            && output.value_kind == contract.accumulator.value_kind
                            && contract.item.validate_definition().is_ok()
                            && contract.accumulator.validate_definition().is_ok()
                            && contract.accumulator.validate(&contract.initial_accumulator).is_ok()
                            && contract.combine_accumulator_port_id.as_str() == "accumulator"
                            && contract.combine_item_port_id.as_str() == "item"
                            && contract.combine_output_port_id.as_str() == "combined"
                            && contract.maximum_active == 1 && contract.maximum_queued == 1
                            && contract.maximum_items > 0)
                    {
                        return Err(KindValidationError::InvalidFlowScan);
                    }
                }
                KindSemanticLaw::FlowEach(contract) => {
                    if flow_each.replace(contract).is_some() || contract.maximum_items == 0 {
                        return Err(KindValidationError::InvalidFlowEach);
                    }
                    let input = self
                        .inputs
                        .iter()
                        .find(|port| port.port_id == contract.input_port_id);
                    let output = self
                        .outputs
                        .iter()
                        .find(|port| port.port_id == contract.output_port_id);
                    if !matches!((input, output), (Some(input), Some(output)) if input.temporal == crate::PortTemporal::Flow { closes: true } && output.temporal == crate::PortTemporal::Flow { closes: true })
                    {
                        return Err(KindValidationError::InvalidFlowEach);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
