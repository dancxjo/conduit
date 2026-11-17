//! Exact port, value/resource boundary and terminal-law validation.
use super::*;

pub(super) fn validate_bounded_collect(
    kind: &KindValidation<'_>,
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

pub(super) fn validate_value_contracts(
    kind: &KindValidation<'_>,
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

pub(super) fn validate_resource_ports(
    kind: &KindValidation<'_>,
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
            .chain(kind.outputs)
            .any(|port| port.port_id == contract.port_id)
        {
            return Err(KindValidationError::UnknownResourcePort);
        }
    }
    Ok(())
}

pub(super) fn validate_terminal_transduction(
    kind: &KindValidation<'_>,
    profile: &crate::TerminalTransductionProfile,
) -> Result<(), KindValidationError> {
    validate_terminal_transduction_ports(kind.inputs, kind.outputs, profile)
}

pub(crate) fn validate_terminal_transduction_ports(
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
    profile: &crate::TerminalTransductionProfile,
) -> Result<(), KindValidationError> {
    use crate::{AbnormalTerminalTransduction, CancellationTransduction, NormalCloseTransduction};
    let input = inputs
        .iter()
        .find(|port| port.port_id == profile.input_port_id)
        .ok_or(KindValidationError::UnknownTerminalTransductionInput)?;
    let output = outputs
        .iter()
        .find(|port| port.port_id == profile.output_port_id)
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
            .filter(|port| port.value_kind.as_str() == crate::CANCELLATION_REQUEST_INFO_ID)
            .count();
        if cancellation_inputs != 1 {
            return Err(KindValidationError::CancellationControlMismatch);
        }
        if output.abnormal_kind.as_ref() != Some(disposition_kind) {
            return Err(KindValidationError::CancellationDispositionMismatch);
        }
    }
    Ok(())
}
