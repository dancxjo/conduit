use super::{
    InteractionContract, InteractionCurrentState, InteractionDomain, InteractionFamily,
    InteractionProposalPayload, InteractionValue, MAXIMUM_INTERACTION_ID_BYTES,
    MAXIMUM_INTERACTION_OPTIONS, MAXIMUM_INTERACTION_SELECTIONS, MAXIMUM_INTERACTION_VALUE_BYTES,
    TEXT_INFO_ID,
};
use crate::{
    BoundKind, InteractionApplicationOutcome, InteractionRefusal, InteractionValueKind,
    OptionAvailability,
};
use conduit_core::{InfoBool, Quantity, StructuredInfoValue, BOOL_INFO_ID, QUANTITY_INFO_ID};

pub(super) fn validate_family(family: &InteractionFamily) -> Result<(), InteractionRefusal> {
    match family {
        InteractionFamily::Activate | InteractionFamily::Boolean => Ok(()),
        InteractionFamily::ChooseOne(value) => {
            validate_choice(value.value_kind(), *value.maximum_options(), 1, 1)
        }
        InteractionFamily::ChooseMany(value) => validate_choice(
            value.value_kind(),
            *value.maximum_options(),
            *value.minimum_selections(),
            *value.maximum_selections(),
        ),
        InteractionFamily::Scalar(value)
            if value.minimum() <= value.maximum() && *value.granularity() > 0 =>
        {
            Ok(())
        }
        InteractionFamily::RelativeAdjustment(value)
            if value.minimum_delta() <= value.maximum_delta()
                && *value.granularity() > 0
                && value.unit().declared_role() != conduit_core::QuantityRole::Point =>
        {
            Ok(())
        }
        InteractionFamily::Text(value)
            if *value.maximum_bytes() > 0
                && usize::try_from(*value.maximum_bytes()).unwrap_or(usize::MAX)
                    <= MAXIMUM_INTERACTION_VALUE_BYTES =>
        {
            Ok(())
        }
        InteractionFamily::Structured(value)
            if *value.maximum_bytes() > 0
                && usize::try_from(*value.maximum_bytes()).unwrap_or(usize::MAX)
                    <= MAXIMUM_INTERACTION_VALUE_BYTES =>
        {
            Ok(())
        }
        _ => Err(InteractionRefusal::InvalidContract),
    }
}

fn validate_choice(
    value_kind: &InteractionValueKind,
    maximum_options: u16,
    minimum: u16,
    maximum: u16,
) -> Result<(), InteractionRefusal> {
    validate_identity(value_kind.get())?;
    if maximum_options == 0
        || usize::from(maximum_options) > MAXIMUM_INTERACTION_OPTIONS
        || minimum > maximum
        || maximum > maximum_options
        || usize::from(maximum) > MAXIMUM_INTERACTION_SELECTIONS
    {
        return Err(InteractionRefusal::InvalidContract);
    }
    Ok(())
}

pub(super) fn validate_state(
    contract: &InteractionContract,
    domain: Option<&InteractionDomain>,
    current: &[InteractionValue],
) -> Result<(), InteractionRefusal> {
    match &contract.family {
        InteractionFamily::Activate => {
            if domain.is_some() || !current.is_empty() {
                return Err(InteractionRefusal::InvalidCurrentState);
            }
        }
        InteractionFamily::Boolean => {
            require_no_domain(domain)?;
            require_count(current, 1, 1)?;
            validate_bool(&current[0])?;
        }
        InteractionFamily::ChooseOne(value) => {
            let domain = validate_domain(domain, value.value_kind(), *value.maximum_options())?;
            require_count(current, 0, 1)?;
            validate_present(domain, current)?;
        }
        InteractionFamily::ChooseMany(value) => {
            let domain = validate_domain(domain, value.value_kind(), *value.maximum_options())?;
            require_count(current, 0, usize::from(*value.maximum_selections()))?;
            reject_duplicate_values(current)?;
            validate_present(domain, current)?;
        }
        InteractionFamily::Scalar(_) => {
            require_no_domain(domain)?;
            require_count(current, 1, 1)?;
            validate_quantity(&contract.family, &current[0])?;
        }
        InteractionFamily::RelativeAdjustment(_) => {
            require_no_domain(domain)?;
            require_count(current, 0, 0)?;
        }
        InteractionFamily::Text(_) | InteractionFamily::Structured(_) => {
            require_no_domain(domain)?;
            require_count(current, 0, 1)?;
            if let Some(value) = current.first() {
                validate_value(&contract.family, value)?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_proposal(
    contract: &InteractionContract,
    state: &InteractionCurrentState,
    payload: &InteractionProposalPayload,
) -> Result<(), InteractionRefusal> {
    if state.contract_identity != contract.contract_identity {
        return Err(InteractionRefusal::StaleState);
    }
    match (&contract.family, payload) {
        (InteractionFamily::Activate, InteractionProposalPayload::Activate) => Ok(()),
        (InteractionFamily::RelativeAdjustment(_), InteractionProposalPayload::Relative(value)) => {
            validate_quantity(&contract.family, value.value())
        }
        (_, InteractionProposalPayload::Values(values)) => {
            let values = values.get().as_slice();
            match &contract.family {
                InteractionFamily::Boolean => {
                    require_count(values, 1, 1)?;
                    validate_bool(&values[0])
                }
                InteractionFamily::ChooseOne(family) => {
                    require_count(values, 1, 1)?;
                    require_value_kind(values, family.value_kind().get())?;
                    validate_selected(
                        state
                            .domain
                            .as_ref()
                            .ok_or(InteractionRefusal::InvalidDomain)?,
                        values,
                    )
                }
                InteractionFamily::ChooseMany(family) => {
                    require_count(
                        values,
                        usize::from(*family.minimum_selections()),
                        usize::from(*family.maximum_selections()),
                    )?;
                    reject_duplicate_values(values)?;
                    require_value_kind(values, family.value_kind().get())?;
                    validate_selected(
                        state
                            .domain
                            .as_ref()
                            .ok_or(InteractionRefusal::InvalidDomain)?,
                        values,
                    )
                }
                InteractionFamily::Scalar(_)
                | InteractionFamily::Text(_)
                | InteractionFamily::Structured(_) => {
                    require_count(values, 1, 1)?;
                    validate_value(&contract.family, &values[0])
                }
                _ => Err(InteractionRefusal::WrongValueKind),
            }
        }
        _ => Err(InteractionRefusal::WrongValueKind),
    }
}

fn validate_value(
    family: &InteractionFamily,
    value: &InteractionValue,
) -> Result<(), InteractionRefusal> {
    match family {
        InteractionFamily::Scalar(_) | InteractionFamily::RelativeAdjustment(_) => {
            validate_quantity(family, value)
        }
        InteractionFamily::Text(family) => {
            if value.kind() != TEXT_INFO_ID {
                return Err(InteractionRefusal::WrongValueKind);
            }
            if value.bytes().len() > *family.maximum_bytes() as usize {
                return Err(InteractionRefusal::ValueBoundExceeded);
            }
            if value.bytes().is_empty() && !family.allow_empty() {
                return Err(InteractionRefusal::MalformedValue);
            }
            core::str::from_utf8(value.bytes())
                .map(|_| ())
                .map_err(|_| InteractionRefusal::MalformedValue)
        }
        InteractionFamily::Structured(family) => {
            if value.kind() != family.value_kind().get() {
                return Err(InteractionRefusal::WrongValueKind);
            }
            if value.bytes().len() > *family.maximum_bytes() as usize {
                return Err(InteractionRefusal::ValueBoundExceeded);
            }
            let structured = StructuredInfoValue::from_canonical_bytes(value.bytes())
                .map_err(|_| InteractionRefusal::MalformedValue)?;
            let actual = structured
                .value_type()
                .semantic_digest()
                .map_err(|_| InteractionRefusal::MalformedValue)?;
            if actual.as_slice() != family.type_digest().get() {
                return Err(InteractionRefusal::WrongValueKind);
            }
            Ok(())
        }
        _ => Err(InteractionRefusal::WrongValueKind),
    }
}

fn validate_bool(value: &InteractionValue) -> Result<(), InteractionRefusal> {
    if value.kind() != BOOL_INFO_ID {
        return Err(InteractionRefusal::WrongValueKind);
    }
    InfoBool::decode(value.bytes())
        .map(|_| ())
        .map_err(|_| InteractionRefusal::MalformedValue)
}

fn validate_quantity(
    family: &InteractionFamily,
    value: &InteractionValue,
) -> Result<(), InteractionRefusal> {
    if value.kind() != QUANTITY_INFO_ID {
        return Err(InteractionRefusal::WrongValueKind);
    }
    let quantity =
        Quantity::decode(value.bytes()).map_err(|_| InteractionRefusal::MalformedValue)?;
    let (unit, minimum, minimum_bound, maximum, maximum_bound, granularity) = match family {
        InteractionFamily::Scalar(family) => (
            *family.unit(),
            *family.minimum(),
            *family.minimum_bound(),
            *family.maximum(),
            *family.maximum_bound(),
            *family.granularity(),
        ),
        InteractionFamily::RelativeAdjustment(family) => (
            *family.unit(),
            *family.minimum_delta(),
            BoundKind::Inclusive,
            *family.maximum_delta(),
            BoundKind::Inclusive,
            *family.granularity(),
        ),
        _ => return Err(InteractionRefusal::WrongValueKind),
    };
    if quantity.unit() != unit {
        return Err(InteractionRefusal::WrongValueKind);
    }
    let quantity = quantity
        .to_i64(unit)
        .map_err(|_| InteractionRefusal::UnsupportedGranularity)?;
    let below =
        quantity < minimum || (quantity == minimum && minimum_bound == BoundKind::Exclusive);
    let above =
        quantity > maximum || (quantity == maximum && maximum_bound == BoundKind::Exclusive);
    if below || above {
        return Err(InteractionRefusal::OutOfRange);
    }
    if (i128::from(quantity) - i128::from(minimum)).rem_euclid(i128::from(granularity)) != 0 {
        return Err(InteractionRefusal::UnsupportedGranularity);
    }
    Ok(())
}

fn validate_domain<'a>(
    domain: Option<&'a InteractionDomain>,
    value_kind: &InteractionValueKind,
    maximum: u16,
) -> Result<&'a InteractionDomain, InteractionRefusal> {
    let domain = domain.ok_or(InteractionRefusal::InvalidDomain)?;
    if domain.options.is_empty() || domain.options.len() > usize::from(maximum) {
        return Err(InteractionRefusal::InvalidDomain);
    }
    for (index, option) in domain.options.iter().enumerate() {
        validate_identity(&option.identity)?;
        if option.value.kind() != value_kind.get()
            || option.value.bytes().len() > MAXIMUM_INTERACTION_VALUE_BYTES
            || domain.options[index + 1..].iter().any(|candidate| {
                candidate.identity == option.identity || candidate.value == option.value
            })
        {
            return Err(InteractionRefusal::InvalidDomain);
        }
        if let OptionAvailability::Unavailable(unavailable) = &option.availability {
            validate_identity(unavailable.reason_code())?;
        }
    }
    Ok(domain)
}

fn validate_selected(
    domain: &InteractionDomain,
    selected: &[InteractionValue],
) -> Result<(), InteractionRefusal> {
    for value in selected {
        let option = domain
            .options
            .iter()
            .find(|option| option.value == *value)
            .ok_or(InteractionRefusal::RemovedOption)?;
        if !matches!(option.availability, OptionAvailability::Available) {
            return Err(InteractionRefusal::UnavailableOption);
        }
    }
    Ok(())
}

fn validate_present(
    domain: &InteractionDomain,
    selected: &[InteractionValue],
) -> Result<(), InteractionRefusal> {
    if selected
        .iter()
        .any(|value| !domain.options.iter().any(|option| option.value == *value))
    {
        Err(InteractionRefusal::RemovedOption)
    } else {
        Ok(())
    }
}

fn reject_duplicate_values(values: &[InteractionValue]) -> Result<(), InteractionRefusal> {
    if values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
    {
        Err(InteractionRefusal::InvalidCardinality)
    } else {
        Ok(())
    }
}

fn require_value_kind(
    values: &[InteractionValue],
    expected: &str,
) -> Result<(), InteractionRefusal> {
    if values.iter().any(|value| value.kind() != expected) {
        Err(InteractionRefusal::WrongValueKind)
    } else {
        Ok(())
    }
}

fn require_count(
    values: &[InteractionValue],
    minimum: usize,
    maximum: usize,
) -> Result<(), InteractionRefusal> {
    if values.len() < minimum || values.len() > maximum {
        Err(InteractionRefusal::InvalidCardinality)
    } else {
        Ok(())
    }
}

fn require_no_domain(domain: Option<&InteractionDomain>) -> Result<(), InteractionRefusal> {
    if domain.is_some() {
        Err(InteractionRefusal::InvalidDomain)
    } else {
        Ok(())
    }
}

pub(super) fn validate_outcome(
    outcome: &InteractionApplicationOutcome,
) -> Result<(), InteractionRefusal> {
    match outcome {
        InteractionApplicationOutcome::Accepted(accepted) => {
            validate_identity(accepted.resulting_state_identity())
        }
        InteractionApplicationOutcome::Refused(refused) => validate_identity(refused.reason_code()),
        InteractionApplicationOutcome::Failed(failed) => validate_identity(failed.reason_code()),
        InteractionApplicationOutcome::Cancelled => Ok(()),
    }
}

pub(super) fn validate_identity(value: &str) -> Result<(), InteractionRefusal> {
    if value.is_empty()
        || value.len() > MAXIMUM_INTERACTION_ID_BYTES
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        Err(InteractionRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}
