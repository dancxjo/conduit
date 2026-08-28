use super::operation::OperationBudget;
use conduit_core::{ConfigurationValue, PlannedGear};

#[derive(Clone, Copy)]
pub(super) struct TimingConfiguration {
    pub(super) duration_ms: u64,
    pub(super) maximum_values: usize,
}

pub(super) fn parse(
    placement: &PlannedGear,
    debounce: bool,
) -> Result<TimingConfiguration, String> {
    let expected = if debounce { 3 } else { 2 };
    if placement.configuration.len() != expected {
        return Err("timing operation has an incomplete exact configuration".to_string());
    }
    let mut duration_ms = None;
    let mut maximum_values = None;
    let mut policy = None;
    for entry in &placement.configuration {
        match (entry.key.as_str(), &entry.value) {
            ("duration-ms", ConfigurationValue::U64(value)) => duration_ms = Some(*value),
            ("maximum-values", ConfigurationValue::U64(value)) => maximum_values = Some(*value),
            ("policy", ConfigurationValue::Text(value)) => policy = Some(value.as_str()),
            _ => return Err("timing operation has an invalid configuration field".to_string()),
        }
    }
    if debounce && policy != Some(conduit_semantic_catalog::TIME_POLICY_TRAILING) {
        return Err("time/debounce supports only exact trailing policy".to_string());
    }
    let duration_ms = duration_ms.ok_or_else(|| "timing duration is missing".to_string())?;
    if duration_ms > conduit_semantic_catalog::TIME_MAXIMUM_DURATION_MS {
        return Err("timing duration exceeds the reviewed maximum".to_string());
    }
    let maximum_values = maximum_values
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| {
            let maximum = if debounce {
                conduit_semantic_catalog::TIME_MAXIMUM_VALUES
            } else {
                conduit_semantic_catalog::TIME_TIMEOUT_MAXIMUM_VALUES
            } as usize;
            *value > 0 && *value <= maximum
        })
        .ok_or_else(|| "timing maximum-values is invalid".to_string())?;
    Ok(TimingConfiguration {
        duration_ms,
        maximum_values,
    })
}

pub(super) fn parse_pacing(
    placement: &PlannedGear,
    policy: Option<&str>,
) -> Result<TimingConfiguration, String> {
    let expected = if policy.is_some() { 3 } else { 2 };
    if placement.configuration.len() != expected {
        return Err("pacing operation has an incomplete exact configuration".to_string());
    }
    let mut duration_ms = None;
    let mut maximum_values = None;
    let mut actual_policy = None;
    for entry in &placement.configuration {
        match (entry.key.as_str(), &entry.value) {
            ("duration-ms", ConfigurationValue::U64(value)) => duration_ms = Some(*value),
            ("maximum-values", ConfigurationValue::U64(value)) => maximum_values = Some(*value),
            ("policy", ConfigurationValue::Text(value)) => actual_policy = Some(value.as_str()),
            _ => return Err("pacing operation has an invalid configuration field".to_string()),
        }
    }
    if actual_policy != policy {
        return Err("pacing operation has an unsupported policy".to_string());
    }
    let duration_ms = duration_ms.ok_or_else(|| "pacing duration is missing".to_string())?;
    if duration_ms > conduit_semantic_catalog::TIME_MAXIMUM_DURATION_MS {
        return Err("pacing duration exceeds the reviewed maximum".to_string());
    }
    let maximum_values = maximum_values
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| {
            *value > 0 && *value <= conduit_semantic_catalog::TIME_MAXIMUM_VALUES as usize
        })
        .ok_or_else(|| "pacing maximum-values is invalid".to_string())?;
    Ok(TimingConfiguration {
        duration_ms,
        maximum_values,
    })
}

pub(super) fn budget(
    requests: usize,
    duration_values: usize,
    output_values: usize,
) -> Result<OperationBudget, String> {
    let items = duration_values
        .checked_add(output_values)
        .and_then(|value| u16::try_from(value.max(1)).ok())
        .ok_or_else(|| "timing value item budget overflow".to_string())?;
    let bytes = usize::from(items)
        .checked_mul(8)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "timing value byte budget overflow".to_string())?;
    let sign_items = requests
        .checked_mul(20)
        .and_then(|value| value.checked_add(64))
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| "timing Sign budget overflow".to_string())?;
    Ok(OperationBudget {
        value_items: items,
        value_bytes: bytes,
        host_requests: requests,
        sign_items,
        maximum_value_bytes: 8,
    })
}
