//! Frozen pre-borrowed validator oracle; test-only, not a production fallback.
//! Source Git blobs: capability_offer dcfd61194c97e61d98fd792f32012fa5595bef94;
//! initialized_state 85d4213018394642bd9a94800b6104e6b6ed9540;
//! realization_properties 789447974a1a7f2d2a314e81d9059a25ee2c4e31.
//! Every owned validation in the unit suite checks this independent oracle and
//! an actual borrowed-port projection. Keep the oracle independent of Ref helpers.
use super::*;
pub(super) fn legacy_validate(kind: &Kind) -> Result<(), KindValidationError> {
    legacy_requirements(kind)?;
    legacy_initialized(kind)?;
    if kind.kind_id.as_str().is_empty() {
        return Err(KindValidationError::EmptyId);
    }
    if kind.kind_contract_revision.as_str().is_empty() {
        return Err(KindValidationError::EmptyIdentity);
    }
    if kind.inputs.iter().chain(&kind.outputs).any(|port| {
        port.abnormal_kind
            .as_ref()
            .is_some_and(|kind| kind.as_str().is_empty())
    }) {
        return Err(KindValidationError::EmptyAbnormalTerminalKind);
    }
    let mut port_symbols = BTreeSet::new();
    if kind
        .inputs
        .iter()
        .chain(&kind.outputs)
        .any(|port| !port_symbols.insert(port.port_id.as_str()))
    {
        return Err(KindValidationError::DuplicatePortSymbol);
    }
    let mut keys = BTreeSet::new();
    for field in &kind.configuration {
        if !keys.insert(field.key.as_str()) {
            return Err(KindValidationError::DuplicateConfigurationKey);
        }
        let Some(front) = kind
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
    for law in &kind.semantic_laws {
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
                validate_terminal_transduction(kind, profile)?;
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
                validate_resource_ports(kind, ports)?;
            }
            KindSemanticLaw::ValueContracts(contracts) => {
                if value_contracts.replace(contracts).is_some() {
                    return Err(KindValidationError::DuplicateValueBounds);
                }
                validate_value_contracts(kind, contracts)?;
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
                validate_bounded_collect(kind, contract)?;
            }
            KindSemanticLaw::FlowSelect(contract) => {
                if flow_select.replace(contract).is_some() {
                    return Err(KindValidationError::DuplicateFlowSelect);
                }
                let input = kind
                    .inputs
                    .iter()
                    .find(|port| port.port_id == contract.input_port_id);
                let output = kind
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
                let input = kind
                    .inputs
                    .iter()
                    .find(|port| port.port_id == contract.input_port_id);
                let output = kind
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
                let input = kind
                    .inputs
                    .iter()
                    .find(|port| port.port_id == contract.input_port_id);
                let output = kind
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
                let input = kind
                    .inputs
                    .iter()
                    .find(|port| port.port_id == contract.input_port_id);
                let output = kind
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
fn validate_bounded_collect(
    kind: &Kind,
    contract: &crate::BoundedCollectSemanticLaw,
) -> Result<(), KindValidationError> {
    let input = kind
        .inputs
        .iter()
        .find(|port| port.port_id == contract.input_port_id)
        .ok_or(KindValidationError::InvalidBoundedCollect)?;
    let output = kind
        .outputs
        .iter()
        .find(|port| port.port_id == contract.output_port_id)
        .ok_or(KindValidationError::InvalidBoundedCollect)?;
    if contract.maximum_items == 0
        || contract.element.validate_definition().is_err()
        || contract.collection.validate_definition().is_err()
        || contract.overflow_disposition.validate_definition().is_err()
        || input.direction != crate::PortDirection::Input
        || input.temporal != (crate::PortTemporal::Flow { closes: true })
        || input.value_kind != contract.element.value_kind
        || input.abnormal_kind.is_some()
        || output.direction != crate::PortDirection::Output
        || output.temporal != crate::PortTemporal::Value
        || output.value_kind != contract.collection.value_kind
        || output.abnormal_kind.as_ref() != Some(&contract.overflow_disposition.value_kind)
    {
        return Err(KindValidationError::InvalidBoundedCollect);
    }
    let encoder = crate::PreparedLeafSequenceEncoder::new(
        contract.element.value_kind.clone(),
        contract.element.maximum_bytes,
        contract.maximum_items,
    )
    .map_err(|_| KindValidationError::InvalidBoundedCollect)?;
    let value_type = encoder
        .value_type()
        .map_err(|_| KindValidationError::InvalidBoundedCollect)?;
    let profile = value_type
        .profile()
        .map_err(|_| KindValidationError::InvalidBoundedCollect)?;
    if profile.value_kind() != &contract.collection.value_kind
        || encoder.maximum_bytes() != contract.collection.maximum_bytes
    {
        return Err(KindValidationError::InvalidBoundedCollect);
    }
    let required = [
        (
            crate::FrontValueLocation::Input(contract.input_port_id.clone()),
            &contract.element,
        ),
        (
            crate::FrontValueLocation::Output(contract.output_port_id.clone()),
            &contract.collection,
        ),
        (
            crate::FrontValueLocation::OutputAbnormal(contract.output_port_id.clone()),
            &contract.overflow_disposition,
        ),
    ];
    if required.iter().any(|(location, expected)| {
        !kind
            .value_contracts()
            .iter()
            .any(|actual| actual.location == *location && actual.contract == **expected)
    }) {
        return Err(KindValidationError::InvalidBoundedCollect);
    }
    Ok(())
}

fn validate_value_contracts(
    kind: &Kind,
    contracts: &[crate::FrontValueContract],
) -> Result<(), KindValidationError> {
    let mut locations = BTreeSet::new();
    for value_contract in contracts {
        if !locations.insert(value_contract.location.clone()) {
            return Err(KindValidationError::InvalidValueBound);
        }
        let expected_kind = match &value_contract.location {
            crate::FrontValueLocation::Startup(name) => kind
                .startup_parameters
                .iter()
                .find(|parameter| &parameter.name == name)
                .map(|parameter| &parameter.value_type),
            crate::FrontValueLocation::Input(port) => kind
                .inputs
                .iter()
                .find(|candidate| &candidate.port_id == port)
                .map(|candidate| &candidate.value_kind),
            crate::FrontValueLocation::Output(port) => kind
                .outputs
                .iter()
                .find(|candidate| &candidate.port_id == port)
                .map(|candidate| &candidate.value_kind),
            crate::FrontValueLocation::InputAbnormal(port) => kind
                .inputs
                .iter()
                .find(|candidate| &candidate.port_id == port)
                .and_then(|candidate| candidate.abnormal_kind.as_ref()),
            crate::FrontValueLocation::OutputAbnormal(port) => kind
                .outputs
                .iter()
                .find(|candidate| &candidate.port_id == port)
                .and_then(|candidate| candidate.abnormal_kind.as_ref()),
        };
        let Some(expected_kind) = expected_kind else {
            return Err(KindValidationError::UnknownValueBoundLocation);
        };
        if expected_kind != &value_contract.contract.value_kind {
            return Err(KindValidationError::InvalidValueBound);
        }
    }
    Ok(())
}

fn validate_resource_ports(
    kind: &Kind,
    contracts: &[crate::ResourcePortContract],
) -> Result<(), KindValidationError> {
    let mut ids = BTreeSet::new();
    for contract in contracts {
        if contract.class_id.as_str().is_empty() {
            return Err(KindValidationError::EmptyResourcePortClass);
        }
        if !ids.insert(contract.port_id.as_str()) {
            return Err(KindValidationError::DuplicateResourcePort);
        }
        if !kind
            .inputs
            .iter()
            .chain(&kind.outputs)
            .any(|port| port.port_id == contract.port_id)
        {
            return Err(KindValidationError::UnknownResourcePort);
        }
    }
    Ok(())
}

fn validate_terminal_transduction(
    kind: &Kind,
    profile: &crate::TerminalTransductionProfile,
) -> Result<(), KindValidationError> {
    validate_terminal_transduction_ports(&kind.inputs, &kind.outputs, profile)
}

pub(crate) fn validate_terminal_transduction_ports(
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
    profile: &crate::TerminalTransductionProfile,
) -> Result<(), KindValidationError> {
    validate_terminal_transduction_ports_ref(
        crate::PortDescriptorRefs::Owned(inputs),
        crate::PortDescriptorRefs::Owned(outputs),
        profile,
    )
}
pub(crate) fn validate_terminal_transduction_ports_ref(
    inputs: crate::PortDescriptorRefs<'_>,
    outputs: crate::PortDescriptorRefs<'_>,
    profile: &crate::TerminalTransductionProfile,
) -> Result<(), KindValidationError> {
    use crate::{AbnormalTerminalTransduction, CancellationTransduction, NormalCloseTransduction};
    let input = inputs
        .iter()
        .find(|port| port.port_id == profile.input_port_id.as_str())
        .ok_or(KindValidationError::UnknownTerminalTransductionInput)?;
    let output = outputs
        .iter()
        .find(|port| port.port_id == profile.output_port_id.as_str())
        .ok_or(KindValidationError::UnknownTerminalTransductionOutput)?;
    if !matches!(&profile.normal_close, NormalCloseTransduction::NotAccepted)
        && (!matches!(input.temporal, crate::PortTemporal::Flow { closes: true })
            || !matches!(output.temporal, crate::PortTemporal::Flow { closes: true }))
    {
        return Err(KindValidationError::TerminalCloseModalityMismatch);
    }
    if !matches!(&profile.abnormal, AbnormalTerminalTransduction::NotAccepted)
        && input.abnormal_kind.is_none()
    {
        return Err(KindValidationError::TerminalAbnormalKindMismatch);
    }
    if matches!(
        &profile.abnormal,
        AbnormalTerminalTransduction::PropagateAfterDrain
            | AbnormalTerminalTransduction::FinalizeThenPropagate(_)
    ) && input.abnormal_kind != output.abnormal_kind
    {
        return Err(KindValidationError::TerminalAbnormalKindMismatch);
    }
    let close_bound = match &profile.normal_close {
        NormalCloseTransduction::FlushThenPropagate(bound)
        | NormalCloseTransduction::FlushThenPropagateWhenAllClose(bound) => Some(bound),
        _ => None,
    };
    let abnormal_bound = match &profile.abnormal {
        AbnormalTerminalTransduction::FinalizeThenPropagate(bound) => Some(bound),
        _ => None,
    };
    if close_bound
        .into_iter()
        .chain(abnormal_bound)
        .any(|bound| bound.maximum_items == 0 || bound.maximum_bytes == 0)
    {
        return Err(KindValidationError::EmptyTerminalEmissionBound);
    }
    let law_is_empty = match &profile.normal_close {
        NormalCloseTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    } || match &profile.abnormal {
        AbnormalTerminalTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    } || match &profile.cancellation {
        CancellationTransduction::DomainSpecific { law } => law.as_str().is_empty(),
        _ => false,
    };
    if law_is_empty {
        return Err(KindValidationError::EmptyTerminalLawIdentity);
    }
    if let CancellationTransduction::Request { disposition_kind } = &profile.cancellation {
        let cancellation_inputs = inputs
            .iter()
            .filter(|port| port.value_kind == crate::CANCELLATION_REQUEST_INFO_ID)
            .count();
        if cancellation_inputs != 1 {
            return Err(KindValidationError::CancellationControlMismatch);
        }
        if output.abnormal_kind != Some(disposition_kind.as_str()) {
            return Err(KindValidationError::CancellationDispositionMismatch);
        }
    }
    Ok(())
}
fn legacy_requirements(kind: &crate::Kind) -> Result<(), crate::KindValidationError> {
    use alloc::collections::BTreeSet;
    let mut fields = BTreeSet::new();
    for law in &kind.semantic_laws {
        let (property_profile, configuration_key, exact) = match law {
            crate::KindSemanticLaw::RealizationRequirement {
                property_profile,
                configuration_key,
            } => (property_profile, configuration_key, false),
            crate::KindSemanticLaw::ExactRealizationProperty(value) => {
                (&value.property_profile, &value.configuration_key, true)
            }
            _ => continue,
        };
        if property_profile.as_str().is_empty()
            || configuration_key.is_empty()
            || !fields.insert(configuration_key.as_str())
            || !kind.configuration.iter().any(|field| {
                field.key == *configuration_key
                    && match &field.rule {
                        crate::KindConfigurationRule::Structured { profile } => {
                            !exact || profile == property_profile
                        }
                        crate::KindConfigurationRule::Quantity if exact => {
                            property_profile == &crate::quantity_realization_profile()
                        }
                        _ => false,
                    }
            })
        {
            return Err(crate::KindValidationError::InvalidRealizationRequirement);
        }
    }
    if fields.len() > crate::MAXIMUM_REALIZATION_PROPERTIES {
        return Err(crate::KindValidationError::InvalidRealizationRequirement);
    }
    Ok(())
}

use crate::{
    InitializedStateLaw, KindSemanticContract, PortDirection, PortTemporal, TemporalStateBehavior,
};
fn initialized_state_boundary<'a>(
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
    semantic: &'a KindSemanticContract,
) -> Result<Option<&'a InitializedStateLaw>, &'static str> {
    let mut laws = semantic.laws.iter().filter_map(|law| match law {
        KindSemanticLaw::InitializedState(value) => Some(value),
        _ => None,
    });
    let Some(law) = laws.next() else {
        return Ok(None);
    };
    if laws.next().is_some() || inputs.len() != 1 || outputs.len() != 1 {
        return Err("initialized state requires one exact update/current boundary");
    }
    let mut state = semantic
        .laws
        .iter()
        .filter(|law| matches!(law, KindSemanticLaw::TemporalState(_)));
    if state.next()
        != Some(&KindSemanticLaw::TemporalState(
            TemporalStateBehavior::Retained,
        ))
        || state.next().is_some()
    {
        return Err("initialized state must declare retained temporal semantics");
    }
    let input = &inputs[0];
    let output = &outputs[0];
    if input.port_id != law.update_port_id
        || input.direction != PortDirection::Input
        || input.abnormal_kind.is_some()
        || input.temporal != (PortTemporal::Flow { closes: false })
        || output.port_id != law.current_port_id
        || output.direction != PortDirection::Output
        || output.abnormal_kind.is_some()
        || !matches!(
            output.temporal,
            PortTemporal::Current | PortTemporal::Flow { closes: false }
        )
        || output.value_kind != law.current.value_kind
        || law.current.maximum_bytes == 0
        || law.current.validate_definition().is_err()
        || law.current.validate(&law.initial).is_err()
    {
        return Err("initialized state has invalid ports, initial value or finite bound");
    }
    Ok(Some(law))
}

fn legacy_initialized(kind: &crate::Kind) -> Result<(), crate::KindValidationError> {
    // Ordinary Kinds may carry large structured configuration. Do not clone
    // their entire semantic contract merely to discover this law is absent.
    if !kind
        .semantic_laws
        .iter()
        .any(|law| matches!(law, KindSemanticLaw::InitializedState(_)))
    {
        return Ok(());
    }
    // Checker projections carry placeholder realization limits. Check meaning
    // here; the planner separately checks the selected Back's finite storage.
    initialized_state_boundary(&kind.inputs, &kind.outputs, &kind.semantic_contract())
        .map(|_| ())
        .map_err(|_| crate::KindValidationError::InvalidInitializedState)
}

pub(super) fn check(kind: &Kind, result: Result<(), KindValidationError>) {
    assert_eq!(result, legacy_validate(kind), "frozen owning oracle");
    let inputs: Vec<_> = kind
        .inputs
        .iter()
        .map(crate::PortDescriptorRef::from)
        .collect();
    let outputs: Vec<_> = kind
        .outputs
        .iter()
        .map(crate::PortDescriptorRef::from)
        .collect();
    let mut view = crate::KindDefinitionRef::from(kind);
    view.inputs = crate::PortDescriptorRefs::Borrowed(&inputs);
    view.outputs = crate::PortDescriptorRefs::Borrowed(&outputs);
    assert_eq!(
        view.validate(),
        result,
        "independent borrowed descriptor projection"
    );
}

#[test]
fn initialized_state_and_realization_requirements_keep_exact_refusals() {
    use crate::*;
    let mut kind = fixture();
    kind.inputs[0].temporal = PortTemporal::Flow { closes: false };
    kind.outputs[0].temporal = PortTemporal::Current;
    let current =
        CheckedValueContract::new(kind.outputs[0].value_kind.clone(), 1, alloc::vec![]).unwrap();
    kind.semantic_laws = alloc::vec![
        KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
        KindSemanticLaw::InitializedState(InitializedStateLaw {
            update_port_id: port_id("in"),
            current_port_id: port_id("out"),
            current,
            initial: alloc::vec![0],
        })
    ];
    assert_eq!(kind.validate(), Ok(()));
    for mutation in 0..10 {
        let mut changed = kind.clone();
        let KindSemanticLaw::InitializedState(law) = &mut changed.semantic_laws[1] else {
            unreachable!()
        };
        match mutation {
            0 => law.update_port_id = port_id("missing"),
            1 => law.current_port_id = port_id("missing"),
            2 => law.current.value_kind = kind_id("missing"),
            3 => law.current.maximum_bytes = 0,
            4 => law.initial = alloc::vec![2],
            5 => changed.inputs[0].temporal = PortTemporal::Value,
            6 => changed.outputs[0].temporal = PortTemporal::Value,
            7 => {
                changed.semantic_laws.remove(0);
            }
            8 => changed.semantic_laws.push(changed.semantic_laws[1].clone()),
            _ => changed.inputs.push(changed.inputs[0].clone()),
        }
        assert_eq!(
            changed.validate(),
            Err(KindValidationError::InvalidInitializedState)
        );
    }
    let kind = fixture();
    for law in [
        KindSemanticLaw::RealizationRequirement {
            property_profile: kind_id("profile"),
            configuration_key: "absent".into(),
        },
        KindSemanticLaw::RealizationRequirement {
            property_profile: kind_id(""),
            configuration_key: "absent".into(),
        },
        KindSemanticLaw::ExactRealizationProperty(ExactRealizationProperty {
            property_profile: kind_id("profile"),
            configuration_key: "absent".into(),
        }),
    ] {
        let mut changed = kind.clone();
        changed.semantic_laws.push(law);
        assert_eq!(
            changed.validate(),
            Err(KindValidationError::InvalidRealizationRequirement)
        );
        // Realization refusal precedes an otherwise independently invalid id.
        changed.kind_id = kind_id("");
        assert_eq!(
            changed.validate(),
            Err(KindValidationError::InvalidRealizationRequirement)
        );
    }
}

fn fixture() -> Kind {
    use crate::*;
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id("parity/kind"),
        kind_contract_revision: "parity/v1".into(),
        inputs: alloc::vec![PortDescriptor {
            port_id: port_id("in"),
            direction: PortDirection::Input,
            value_kind: kind_id(BOOL_INFO_ID),
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None
        }],
        outputs: alloc::vec![PortDescriptor {
            port_id: port_id("out"),
            direction: PortDirection::Output,
            value_kind: kind_id(BOOL_INFO_ID),
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None
        }],
        configuration: Vec::new(),
        semantic_laws: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 64,
        },
    }
}

#[test]
fn every_structured_flow_law_uses_the_same_complete_borrowed_validator() {
    use crate::*;
    let value = CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, alloc::vec![]).unwrap();
    let each = KindSemanticLaw::FlowEach(FlowEachSemanticLaw {
        input_port_id: port_id("in"),
        output_port_id: port_id("out"),
        maximum_items: 1,
    });
    let select = KindSemanticLaw::FlowSelect(FlowSelectSemanticLaw {
        input_port_id: port_id("in"),
        output_port_id: port_id("out"),
        predicate_input_kind: kind_id(BOOL_INFO_ID),
        predicate_output_kind: kind_id(BOOL_INFO_ID),
        maximum_active: 1,
        maximum_queued: 1,
        maximum_items: 1,
        invocation: FlowSelectInvocation::OncePerAcceptedInput,
        retained_input: FlowSelectRetainedInput::UntilPredicateCompletion,
        true_disposition: FlowSelectTrueDisposition::EmitRetainedInputExactlyOnce,
        false_disposition: FlowSelectFalseDisposition::EmitNothing,
    });
    let fold = KindSemanticLaw::FlowFold(FlowFoldSemanticLaw {
        input_port_id: port_id("in"),
        output_port_id: port_id("out"),
        item: value.clone(),
        accumulator: value.clone(),
        initial_accumulator: alloc::vec![0],
        combine_accumulator_port_id: port_id("accumulator"),
        combine_item_port_id: port_id("item"),
        combine_output_port_id: port_id("combined"),
        maximum_active: 1,
        maximum_queued: 1,
        maximum_items: 1,
        invocation: FlowFoldInvocation::OncePerAcceptedInput,
        close: FlowFoldCloseDisposition::DrainThenEmitAccumulatorExactlyOnce,
        abnormal: FlowFoldAbnormalDisposition::DiscardAccumulatorAndPropagateExact,
        cancellation: FlowFoldCancellationDisposition::DiscardAccumulatorWithoutEmission,
    });
    let scan = KindSemanticLaw::FlowScan(FlowScanSemanticLaw {
        input_port_id: port_id("in"),
        output_port_id: port_id("out"),
        item: value.clone(),
        accumulator: value.clone(),
        initial_accumulator: alloc::vec![0],
        combine_accumulator_port_id: port_id("accumulator"),
        combine_item_port_id: port_id("item"),
        combine_output_port_id: port_id("combined"),
        maximum_active: 1,
        maximum_queued: 1,
        maximum_items: 1,
        invocation: FlowScanInvocation::OncePerAcceptedInput,
        progression: FlowScanProgression::EmitCombinedAccumulatorExactlyOnceInInputOrder,
        empty: FlowScanEmptyDisposition::EmitNothing,
        close: FlowScanCloseDisposition::DrainThenCloseWithoutExtraEmission,
        abnormal: FlowScanAbnormalDisposition::DiscardAccumulatorAndPropagateExact,
        cancellation: FlowScanCancellationDisposition::DiscardAccumulatorWithoutEmission,
    });
    let join = KindSemanticLaw::KeyedJoin(KeyedJoinSemanticLaw {
        key: value.clone(),
        left_value: value.clone(),
        right_value: value.clone(),
        maximum_pending_per_side: 1,
        pairing: KeyedJoinPairing::OldestWithOldest,
        output_order: KeyedJoinOutputOrder::MatchCompletionArrival,
        capacity: KeyedJoinCapacityBehavior::BackpressureUnmatched,
        unmatched_on_close: KeyedJoinUnmatchedCloseBehavior::DiscardWhenMatchBecomesImpossible,
    });
    for law in [each, select, fold, scan, join] {
        let mut kind = fixture();
        if matches!(law, KindSemanticLaw::FlowFold(_)) {
            kind.outputs[0].temporal = PortTemporal::Value;
        }
        kind.semantic_laws.push(law);
        assert_eq!(kind.validate(), Ok(()));
        let mut duplicated = kind.clone();
        duplicated
            .semantic_laws
            .push(duplicated.semantic_laws[0].clone());
        assert!(duplicated.validate().is_err());
        for mutation in 0..8 {
            let mut changed = kind.clone();
            match &mut changed.semantic_laws[0] {
                KindSemanticLaw::FlowEach(v) => match mutation {
                    0 => v.maximum_items = 0,
                    1 => v.input_port_id = port_id("missing"),
                    _ => v.output_port_id = port_id("missing"),
                },
                KindSemanticLaw::FlowSelect(v) => match mutation {
                    0 => v.maximum_items = 0,
                    1 => v.maximum_active = 2,
                    2 => v.maximum_queued = 2,
                    3 => v.predicate_input_kind = kind_id("wrong"),
                    4 => v.predicate_output_kind = kind_id("wrong"),
                    5 => v.input_port_id = port_id("missing"),
                    _ => v.output_port_id = port_id("missing"),
                },
                KindSemanticLaw::FlowFold(v) => match mutation {
                    0 => v.maximum_items = 0,
                    1 => v.maximum_active = 2,
                    2 => v.maximum_queued = 2,
                    3 => {
                        v.item.constraints = alloc::vec![ValueConstraint::ByteLength {
                            minimum: 2,
                            maximum: 1
                        }]
                    }
                    4 => v.initial_accumulator = alloc::vec![2],
                    5 => v.combine_output_port_id = port_id("wrong"),
                    6 => v.input_port_id = port_id("missing"),
                    _ => v.output_port_id = port_id("missing"),
                },
                KindSemanticLaw::FlowScan(v) => match mutation {
                    0 => v.maximum_items = 0,
                    1 => v.maximum_active = 2,
                    2 => v.maximum_queued = 2,
                    3 => {
                        v.item.constraints = alloc::vec![ValueConstraint::ByteLength {
                            minimum: 2,
                            maximum: 1
                        }]
                    }
                    4 => v.initial_accumulator = alloc::vec![2],
                    5 => v.combine_output_port_id = port_id("wrong"),
                    6 => v.input_port_id = port_id("missing"),
                    _ => v.output_port_id = port_id("missing"),
                },
                KindSemanticLaw::KeyedJoin(v) => match mutation {
                    0 => v.maximum_pending_per_side = 0,
                    1 => {
                        v.key.constraints = alloc::vec![ValueConstraint::ByteLength {
                            minimum: 2,
                            maximum: 1
                        }]
                    }
                    2 => {
                        v.left_value.constraints = alloc::vec![ValueConstraint::ByteLength {
                            minimum: 2,
                            maximum: 1
                        }]
                    }
                    _ => {
                        v.right_value.constraints = alloc::vec![ValueConstraint::ByteLength {
                            minimum: 2,
                            maximum: 1
                        }]
                    }
                },
                _ => unreachable!(),
            }
            assert!(
                changed.validate().is_err(),
                "mutation {mutation}: {:?}",
                changed.semantic_laws
            );
        }
    }
}

#[test]
fn bounded_collect_checks_exact_ports_profiles_and_all_value_locations() {
    use crate::*;
    let mut kind = fixture();
    let element = CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, alloc::vec![]).unwrap();
    let encoder = PreparedLeafSequenceEncoder::new(element.value_kind.clone(), 1, 2).unwrap();
    let collection = CheckedValueContract::new(
        encoder
            .value_type()
            .unwrap()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        encoder.maximum_bytes(),
        alloc::vec![],
    )
    .unwrap();
    kind.outputs[0].temporal = PortTemporal::Value;
    kind.outputs[0].value_kind = collection.value_kind.clone();
    kind.outputs[0].abnormal_kind = Some(element.value_kind.clone());
    let values = alloc::vec![
        FrontValueContract {
            location: FrontValueLocation::Input(port_id("in")),
            contract: element.clone()
        },
        FrontValueContract {
            location: FrontValueLocation::Output(port_id("out")),
            contract: collection.clone()
        },
        FrontValueContract {
            location: FrontValueLocation::OutputAbnormal(port_id("out")),
            contract: element.clone()
        },
    ];
    kind.semantic_laws = alloc::vec![
        KindSemanticLaw::ValueContracts(values),
        KindSemanticLaw::BoundedCollect(BoundedCollectSemanticLaw {
            input_port_id: port_id("in"),
            output_port_id: port_id("out"),
            element: element.clone(),
            collection,
            maximum_items: 2,
            overflow_disposition: element
        })
    ];
    assert_eq!(kind.validate(), Ok(()));
    for mutation in 0..13 {
        let mut changed = kind.clone();
        let KindSemanticLaw::BoundedCollect(law) = &mut changed.semantic_laws[1] else {
            unreachable!()
        };
        match mutation {
            0 => law.maximum_items = 0,
            1 => law.input_port_id = port_id("wrong"),
            2 => law.output_port_id = port_id("wrong"),
            3 => {
                law.element.constraints = alloc::vec![ValueConstraint::ByteLength {
                    minimum: 2,
                    maximum: 1
                }]
            }
            4 => law.collection.maximum_bytes += 1,
            5 => law.collection.value_kind = kind_id("wrong"),
            6 => changed.inputs[0].temporal = PortTemporal::Value,
            7 => changed.outputs[0].temporal = PortTemporal::Flow { closes: true },
            8 => changed.outputs[0].abnormal_kind = None,
            9 => {
                let KindSemanticLaw::ValueContracts(values) = &mut changed.semantic_laws[0] else {
                    unreachable!()
                };
                values.remove(0);
            }
            10 => {
                let KindSemanticLaw::ValueContracts(values) = &mut changed.semantic_laws[0] else {
                    unreachable!()
                };
                values.remove(1);
            }
            11 => {
                let KindSemanticLaw::ValueContracts(values) = &mut changed.semantic_laws[0] else {
                    unreachable!()
                };
                values.remove(2);
            }
            _ => changed.semantic_laws.push(changed.semantic_laws[1].clone()),
        }
        assert!(changed.validate().is_err(), "collect mutation {mutation}");
    }
}

#[test]
fn realization_property_profiles_duplicates_and_field_limit_match_owning_oracle() {
    use crate::*;
    for exact in [false, true] {
        let mut kind = fixture();
        kind.startup_parameters = alloc::vec![FrontStartupParameter {
            name: "property".into(),
            value_type: kind_id(BOOL_INFO_ID),
            has_default: true
        }];
        kind.configuration = alloc::vec![KindConfigurationField {
            key: "property".into(),
            default_value: ConfigurationValue::Bool(false),
            rule: KindConfigurationRule::Structured {
                profile: kind_id("profile")
            }
        }];
        let law = if exact {
            KindSemanticLaw::ExactRealizationProperty(ExactRealizationProperty {
                property_profile: kind_id("profile"),
                configuration_key: "property".into(),
            })
        } else {
            KindSemanticLaw::RealizationRequirement {
                property_profile: kind_id("profile"),
                configuration_key: "property".into(),
            }
        };
        kind.semantic_laws.push(law);
        assert_eq!(kind.validate(), Ok(()));
        let mut duplicate = kind.clone();
        duplicate
            .semantic_laws
            .push(duplicate.semantic_laws[0].clone());
        assert_eq!(
            duplicate.validate(),
            Err(KindValidationError::InvalidRealizationRequirement)
        );
        let mut missing = kind.clone();
        missing.configuration.clear();
        assert_eq!(
            missing.validate(),
            Err(KindValidationError::InvalidRealizationRequirement)
        );
        let mut wrong = kind.clone();
        wrong.configuration[0].rule = KindConfigurationRule::Any;
        assert_eq!(
            wrong.validate(),
            Err(KindValidationError::InvalidRealizationRequirement)
        );
        if exact {
            let mut changed = kind.clone();
            changed.configuration[0].rule = KindConfigurationRule::Structured {
                profile: kind_id("other"),
            };
            assert_eq!(
                changed.validate(),
                Err(KindValidationError::InvalidRealizationRequirement)
            );
        }
        let mut full = kind;
        for i in 0..MAXIMUM_REALIZATION_PROPERTIES {
            let key = alloc::format!("field-{i}");
            full.configuration.push(KindConfigurationField {
                key: key.clone(),
                default_value: ConfigurationValue::Bool(false),
                rule: KindConfigurationRule::Structured {
                    profile: kind_id("profile"),
                },
            });
            full.startup_parameters.push(FrontStartupParameter {
                name: key.clone(),
                value_type: kind_id(BOOL_INFO_ID),
                has_default: true,
            });
            full.semantic_laws.push(if exact {
                KindSemanticLaw::ExactRealizationProperty(ExactRealizationProperty {
                    property_profile: kind_id("profile"),
                    configuration_key: key,
                })
            } else {
                KindSemanticLaw::RealizationRequirement {
                    property_profile: kind_id("profile"),
                    configuration_key: key,
                }
            });
        }
        assert_eq!(
            full.validate(),
            Err(KindValidationError::InvalidRealizationRequirement)
        );
    }
}
