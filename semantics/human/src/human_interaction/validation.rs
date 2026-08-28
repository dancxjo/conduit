use super::{
    BoundKind, InteractionApplicationOutcome, InteractionContract, InteractionCurrentState,
    InteractionDomain, InteractionFamily, InteractionProposalPayload, InteractionRefusal,
    InteractionValue, OptionAvailability, MAXIMUM_INTERACTION_ID_BYTES,
    MAXIMUM_INTERACTION_OPTIONS, MAXIMUM_INTERACTION_SELECTIONS, MAXIMUM_INTERACTION_VALUE_BYTES,
    TEXT_INFO_ID,
};
use conduit_core::{
    InfoBool, KindId, Quantity, StructuredInfoValue, BOOL_INFO_ID, QUANTITY_INFO_ID,
};

pub(super) fn validate_family(family: &InteractionFamily) -> Result<(), InteractionRefusal> {
    match family {
        InteractionFamily::Activate | InteractionFamily::Boolean => Ok(()),
        InteractionFamily::ChooseOne {
            value_kind,
            maximum_options,
        } => validate_choice(value_kind, *maximum_options, 1, 1),
        InteractionFamily::ChooseMany {
            value_kind,
            maximum_options,
            minimum_selections,
            maximum_selections,
        } => validate_choice(
            value_kind,
            *maximum_options,
            *minimum_selections,
            *maximum_selections,
        ),
        InteractionFamily::Scalar {
            minimum,
            maximum,
            granularity,
            ..
        }
        | InteractionFamily::RelativeAdjustment {
            minimum_delta: minimum,
            maximum_delta: maximum,
            granularity,
            ..
        } if minimum <= maximum && *granularity > 0 => Ok(()),
        InteractionFamily::Text { maximum_bytes, .. }
        | InteractionFamily::Structured { maximum_bytes, .. }
            if *maximum_bytes > 0
                && usize::try_from(*maximum_bytes).unwrap_or(usize::MAX)
                    <= MAXIMUM_INTERACTION_VALUE_BYTES =>
        {
            if let InteractionFamily::Structured { value_kind, .. } = family {
                validate_identity(value_kind.as_str())?;
            }
            Ok(())
        }
        _ => Err(InteractionRefusal::InvalidContract),
    }
}

fn validate_choice(
    value_kind: &KindId,
    maximum_options: u16,
    minimum: u16,
    maximum: u16,
) -> Result<(), InteractionRefusal> {
    validate_identity(value_kind.as_str())?;
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
        InteractionFamily::ChooseOne {
            value_kind,
            maximum_options,
        } => {
            let domain = validate_domain(domain, value_kind, *maximum_options)?;
            require_count(current, 0, 1)?;
            validate_present(domain, current)?;
        }
        InteractionFamily::ChooseMany {
            value_kind,
            maximum_options,
            maximum_selections,
            ..
        } => {
            let domain = validate_domain(domain, value_kind, *maximum_options)?;
            require_count(current, 0, usize::from(*maximum_selections))?;
            reject_duplicate_values(current)?;
            validate_present(domain, current)?;
        }
        InteractionFamily::Scalar { .. } => {
            require_no_domain(domain)?;
            require_count(current, 1, 1)?;
            validate_quantity(&contract.family, &current[0])?;
        }
        InteractionFamily::RelativeAdjustment { .. } => {
            require_no_domain(domain)?;
            require_count(current, 0, 0)?;
        }
        InteractionFamily::Text { .. } | InteractionFamily::Structured { .. } => {
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
        (
            InteractionFamily::RelativeAdjustment { .. },
            InteractionProposalPayload::Relative(value),
        ) => validate_quantity(&contract.family, value),
        (_, InteractionProposalPayload::Values(values)) => match &contract.family {
            InteractionFamily::Boolean => {
                require_count(values, 1, 1)?;
                validate_bool(&values[0])
            }
            InteractionFamily::ChooseOne { value_kind, .. } => {
                require_count(values, 1, 1)?;
                require_value_kind(values, value_kind)?;
                validate_selected(
                    state
                        .domain
                        .as_ref()
                        .ok_or(InteractionRefusal::InvalidDomain)?,
                    values,
                )
            }
            InteractionFamily::ChooseMany {
                value_kind,
                minimum_selections,
                maximum_selections,
                ..
            } => {
                require_count(
                    values,
                    usize::from(*minimum_selections),
                    usize::from(*maximum_selections),
                )?;
                reject_duplicate_values(values)?;
                require_value_kind(values, value_kind)?;
                validate_selected(
                    state
                        .domain
                        .as_ref()
                        .ok_or(InteractionRefusal::InvalidDomain)?,
                    values,
                )
            }
            InteractionFamily::Scalar { .. }
            | InteractionFamily::Text { .. }
            | InteractionFamily::Structured { .. } => {
                require_count(values, 1, 1)?;
                validate_value(&contract.family, &values[0])
            }
            _ => Err(InteractionRefusal::WrongValueKind),
        },
        _ => Err(InteractionRefusal::WrongValueKind),
    }
}

fn validate_value(
    family: &InteractionFamily,
    value: &InteractionValue,
) -> Result<(), InteractionRefusal> {
    match family {
        InteractionFamily::Scalar { .. } | InteractionFamily::RelativeAdjustment { .. } => {
            validate_quantity(family, value)
        }
        InteractionFamily::Text {
            maximum_bytes,
            allow_empty,
        } => {
            if value.value_kind.as_str() != TEXT_INFO_ID {
                return Err(InteractionRefusal::WrongValueKind);
            }
            if value.canonical_bytes.len() > *maximum_bytes as usize {
                return Err(InteractionRefusal::ValueBoundExceeded);
            }
            if value.canonical_bytes.is_empty() && !allow_empty {
                return Err(InteractionRefusal::MalformedValue);
            }
            core::str::from_utf8(&value.canonical_bytes)
                .map(|_| ())
                .map_err(|_| InteractionRefusal::MalformedValue)
        }
        InteractionFamily::Structured {
            value_kind,
            type_digest,
            maximum_bytes,
        } => {
            if &value.value_kind != value_kind {
                return Err(InteractionRefusal::WrongValueKind);
            }
            if value.canonical_bytes.len() > *maximum_bytes as usize {
                return Err(InteractionRefusal::ValueBoundExceeded);
            }
            let structured = StructuredInfoValue::from_canonical_bytes(&value.canonical_bytes)
                .map_err(|_| InteractionRefusal::MalformedValue)?;
            let actual = structured
                .value_type()
                .semantic_digest()
                .map_err(|_| InteractionRefusal::MalformedValue)?;
            if &actual != type_digest {
                return Err(InteractionRefusal::WrongValueKind);
            }
            Ok(())
        }
        _ => Err(InteractionRefusal::WrongValueKind),
    }
}

fn validate_bool(value: &InteractionValue) -> Result<(), InteractionRefusal> {
    if value.value_kind.as_str() != BOOL_INFO_ID {
        return Err(InteractionRefusal::WrongValueKind);
    }
    InfoBool::decode(&value.canonical_bytes)
        .map(|_| ())
        .map_err(|_| InteractionRefusal::MalformedValue)
}

fn validate_quantity(
    family: &InteractionFamily,
    value: &InteractionValue,
) -> Result<(), InteractionRefusal> {
    if value.value_kind.as_str() != QUANTITY_INFO_ID {
        return Err(InteractionRefusal::WrongValueKind);
    }
    let quantity =
        Quantity::decode(&value.canonical_bytes).map_err(|_| InteractionRefusal::MalformedValue)?;
    let (unit, minimum, minimum_bound, maximum, maximum_bound, granularity) = match family {
        InteractionFamily::Scalar {
            unit,
            minimum,
            minimum_bound,
            maximum,
            maximum_bound,
            granularity,
        } => (
            *unit,
            *minimum,
            *minimum_bound,
            *maximum,
            *maximum_bound,
            *granularity,
        ),
        InteractionFamily::RelativeAdjustment {
            unit,
            minimum_delta,
            maximum_delta,
            granularity,
        } => (
            *unit,
            *minimum_delta,
            BoundKind::Inclusive,
            *maximum_delta,
            BoundKind::Inclusive,
            *granularity,
        ),
        _ => return Err(InteractionRefusal::WrongValueKind),
    };
    if quantity.unit() != unit {
        return Err(InteractionRefusal::WrongValueKind);
    }
    let below = quantity.value() < minimum
        || (quantity.value() == minimum && minimum_bound == BoundKind::Exclusive);
    let above = quantity.value() > maximum
        || (quantity.value() == maximum && maximum_bound == BoundKind::Exclusive);
    if below || above {
        return Err(InteractionRefusal::OutOfRange);
    }
    if (i128::from(quantity.value()) - i128::from(minimum)).rem_euclid(i128::from(granularity)) != 0
    {
        return Err(InteractionRefusal::UnsupportedGranularity);
    }
    Ok(())
}

fn validate_domain<'a>(
    domain: Option<&'a InteractionDomain>,
    value_kind: &KindId,
    maximum: u16,
) -> Result<&'a InteractionDomain, InteractionRefusal> {
    let domain = domain.ok_or(InteractionRefusal::InvalidDomain)?;
    if domain.options.is_empty() || domain.options.len() > usize::from(maximum) {
        return Err(InteractionRefusal::InvalidDomain);
    }
    for (index, option) in domain.options.iter().enumerate() {
        validate_identity(&option.identity)?;
        if &option.value.value_kind != value_kind
            || option.value.canonical_bytes.len() > MAXIMUM_INTERACTION_VALUE_BYTES
            || domain.options[index + 1..].iter().any(|candidate| {
                candidate.identity == option.identity || candidate.value == option.value
            })
        {
            return Err(InteractionRefusal::InvalidDomain);
        }
        if let OptionAvailability::Unavailable { reason_code } = &option.availability {
            validate_identity(reason_code)?;
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
    expected: &KindId,
) -> Result<(), InteractionRefusal> {
    if values.iter().any(|value| &value.value_kind != expected) {
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
        InteractionApplicationOutcome::Accepted {
            resulting_state_identity,
        } => validate_identity(resulting_state_identity),
        InteractionApplicationOutcome::Refused { reason_code }
        | InteractionApplicationOutcome::Failed { reason_code } => validate_identity(reason_code),
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
