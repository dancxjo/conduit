use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, HostOperationOutcome,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_std_catalog::{
    MAX_TEXT_BYTES, MAX_TEXT_VALUES, TEXT_JOIN_ARTIFACT, TEXT_JOIN_CONTRACT_REVISION,
    TEXT_JOIN_EXECUTION_PROFILE, TEXT_JOIN_IMPLEMENTATION, TEXT_JOIN_KIND, TEXT_LITERAL_ARTIFACT,
    TEXT_LITERAL_CONTRACT_REVISION, TEXT_LITERAL_EXECUTION_PROFILE, TEXT_LITERAL_IMPLEMENTATION,
    TEXT_LITERAL_KIND, TEXT_PRESENTATION_ARTIFACT, TEXT_PRESENTATION_CONTRACT_REVISION,
    TEXT_PRESENTATION_EXECUTION_PROFILE, TEXT_PRESENTATION_IMPLEMENTATION, TEXT_PRESENTATION_KIND,
    TEXT_PRESENTATION_VALUE_KIND, TEXT_UPPER_ARTIFACT, TEXT_UPPER_CONTRACT_REVISION,
    TEXT_UPPER_EXECUTION_PROFILE, TEXT_UPPER_IMPLEMENTATION, TEXT_UPPER_KIND,
};

pub(super) static TEXT_LITERAL_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TEXT_LITERAL_IMPLEMENTATION,
    budget: text_literal_budget,
    prepare: prepare_text_literal,
};

pub(super) static TEXT_UPPER_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TEXT_UPPER_IMPLEMENTATION,
    budget: text_upper_budget,
    prepare: prepare_text_upper,
};

pub(super) static TEXT_JOIN_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TEXT_JOIN_IMPLEMENTATION,
    budget: text_join_budget,
    prepare: prepare_text_join,
};

pub(super) static TEXT_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TEXT_PRESENTATION_IMPLEMENTATION,
    budget: text_presentation_budget,
    prepare: prepare_text_presentation,
};

pub(super) struct TextLiteralOperation {
    value: ValueRef,
    emitted: bool,
}

pub(super) struct TextTransformOperation {
    pending: Option<RequestId>,
    next: u32,
    maximum_values: u32,
}

pub(super) struct TextPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
    maximum_values: u32,
}

impl TextLiteralOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Emit {
            port: PortId(0),
            value: self.value,
        }
    }

    pub(super) fn resume(&mut self, _input: OperationInput) -> OperationAction {
        InstalledOperation::fail(7)
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            InstalledOperation::fail(7)
        } else {
            self.emitted = true;
            OperationAction::Complete
        }
    }
}

impl TextTransformOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && self.next < self.maximum_values => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_TEXT_BYTES) {
                        Ok(input) => input,
                        Err(_) => return InstalledOperation::fail(8),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return InstalledOperation::fail(8);
                };
                self.pending = None;
                self.next = self.next.saturating_add(1);
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(8),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

impl TextPresentationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && self.next < self.maximum_values => {
                let request = RequestId(self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_TEXT_BYTES) {
                        Ok(input) => input,
                        Err(_) => return InstalledOperation::fail(5),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.next = self.next.saturating_add(1);
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(5),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

fn text_literal_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_text_literal(placement)?;
    let text = text_configuration(placement, "value", MAX_TEXT_BYTES)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: u32::try_from(text.len()).map_err(|_| "text literal is too large")?,
        host_requests: 0,
        clue_items: 32,
        maximum_value_bytes: MAX_TEXT_BYTES,
    })
}

fn prepare_text_literal(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_text_literal(placement)?;
    let text = text_configuration(placement, "value", MAX_TEXT_BYTES)?;
    let value = values
        .store(text.as_bytes())
        .map_err(|error| format!("store text literal: {error:?}"))?;
    Ok(InstalledOperation::TextLiteral(TextLiteralOperation {
        value,
        emitted: false,
    }))
}

fn text_upper_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_text_upper(placement)?;
    Ok(OperationBudget {
        value_items: MAX_TEXT_VALUES as u16,
        value_bytes: MAX_TEXT_BYTES * MAX_TEXT_VALUES as u32,
        host_requests: MAX_TEXT_VALUES as usize,
        clue_items: 64,
        maximum_value_bytes: MAX_TEXT_BYTES,
    })
}

fn prepare_text_upper(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_text_upper(placement)?;
    Ok(InstalledOperation::TextUpper(TextTransformOperation {
        pending: None,
        next: 0,
        maximum_values: MAX_TEXT_VALUES as u32,
    }))
}

fn text_join_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_text_join(placement)?;
    Ok(OperationBudget {
        value_items: MAX_TEXT_VALUES as u16,
        value_bytes: MAX_TEXT_BYTES * MAX_TEXT_VALUES as u32,
        host_requests: MAX_TEXT_VALUES as usize,
        clue_items: 64,
        maximum_value_bytes: MAX_TEXT_BYTES,
    })
}

fn prepare_text_join(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_text_join(placement)?;
    Ok(InstalledOperation::TextJoin(TextTransformOperation {
        pending: None,
        next: 0,
        maximum_values: MAX_TEXT_VALUES as u32,
    }))
}

fn text_presentation_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_text_presentation(placement)?;
    let maximum_values = maximum_values(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: maximum_values as usize,
        clue_items: 64,
        maximum_value_bytes: MAX_TEXT_BYTES,
    })
}

fn prepare_text_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_text_presentation(placement)?;
    Ok(InstalledOperation::TextPresentation(
        TextPresentationOperation {
            pending: None,
            next: 0,
            maximum_values: maximum_values(placement)? as u32,
        },
    ))
}

fn text_configuration<'a>(
    placement: &'a PlannedGear,
    key: &str,
    maximum: u32,
) -> Result<&'a str, String> {
    placement
        .configuration
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| match &entry.value {
            ConfigurationValue::Text(value) if value.len() <= maximum as usize => {
                Some(value.as_str())
            }
            _ => None,
        })
        .ok_or_else(|| format!("text configuration '{key}' is missing, invalid, or oversized"))
}

pub(super) fn join_prefix(placement: &PlannedGear) -> Result<&str, String> {
    text_configuration(placement, "prefix", MAX_TEXT_BYTES)
}

fn maximum_values(placement: &PlannedGear) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find(|entry| entry.key == "maximum-values")
        .and_then(|entry| match entry.value {
            ConfigurationValue::U64(value) if (1..=MAX_TEXT_VALUES).contains(&value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| "text presentation maximum-values is invalid".to_string())
}

fn validate_text_literal(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        TEXT_LITERAL_KIND,
        TEXT_LITERAL_CONTRACT_REVISION,
        TEXT_LITERAL_EXECUTION_PROFILE,
        TEXT_LITERAL_IMPLEMENTATION,
        TEXT_LITERAL_ARTIFACT,
        0,
        1,
    )?;
    text_configuration(placement, "value", MAX_TEXT_BYTES).map(|_| ())
}

fn validate_text_upper(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        TEXT_UPPER_KIND,
        TEXT_UPPER_CONTRACT_REVISION,
        TEXT_UPPER_EXECUTION_PROFILE,
        TEXT_UPPER_IMPLEMENTATION,
        TEXT_UPPER_ARTIFACT,
        1,
        1,
    )
}

fn validate_text_join(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        TEXT_JOIN_KIND,
        TEXT_JOIN_CONTRACT_REVISION,
        TEXT_JOIN_EXECUTION_PROFILE,
        TEXT_JOIN_IMPLEMENTATION,
        TEXT_JOIN_ARTIFACT,
        1,
        1,
    )?;
    text_configuration(placement, "prefix", MAX_TEXT_BYTES).map(|_| ())
}

fn validate_text_presentation(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        TEXT_PRESENTATION_KIND,
        TEXT_PRESENTATION_CONTRACT_REVISION,
        TEXT_PRESENTATION_EXECUTION_PROFILE,
        TEXT_PRESENTATION_IMPLEMENTATION,
        TEXT_PRESENTATION_ARTIFACT,
        1,
        0,
    )?;
    maximum_values(placement).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn validate_identity(
    placement: &PlannedGear,
    kind: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    inputs: usize,
    outputs: usize,
) -> Result<(), String> {
    if placement.kind_id.as_str() != kind
        || placement.kind_contract_revision.as_str() != revision
        || placement.execution_profile_id.as_str() != profile
        || placement.implementation_id.as_str() != implementation
        || placement.artifact_id.as_str() != artifact
        || placement.inputs.len() != inputs
        || placement.outputs.len() != outputs
        || placement.inputs.iter().any(|port| {
            port.port_id.as_str() != "text"
                || port.value_kind.as_str() != TEXT_PRESENTATION_VALUE_KIND
                || port.direction != PortDirection::Input
        })
        || placement.outputs.iter().any(|port| {
            port.port_id.as_str() != "text"
                || port.value_kind.as_str() != TEXT_PRESENTATION_VALUE_KIND
                || port.direction != PortDirection::Output
        })
    {
        return Err(format!(
            "planned {kind} executable identity does not match its installation"
        ));
    }
    Ok(())
}

pub(super) fn uppercase_utf8(input: &[u8], output: &mut Vec<u8>) -> Result<(), String> {
    output.clear();
    let text = core::str::from_utf8(input).map_err(|_| "text/upper input is not valid UTF-8")?;
    for character in text.chars().flat_map(char::to_uppercase) {
        let mut encoded = [0_u8; 4];
        let bytes = character.encode_utf8(&mut encoded).as_bytes();
        if output.len() + bytes.len() > MAX_TEXT_BYTES as usize {
            output.clear();
            return Err("text/upper output exceeds its admitted byte bound".to_string());
        }
        output.extend_from_slice(bytes);
    }
    Ok(())
}

pub(super) fn prefix_utf8(prefix: &str, input: &[u8], output: &mut Vec<u8>) -> Result<(), String> {
    output.clear();
    core::str::from_utf8(input).map_err(|_| "text/join input is not valid UTF-8")?;
    let combined = prefix
        .len()
        .checked_add(input.len())
        .ok_or_else(|| "text/join output byte length overflow".to_string())?;
    if combined > MAX_TEXT_BYTES as usize {
        return Err("text/join output exceeds its admitted byte bound".to_string());
    }
    output.extend_from_slice(prefix.as_bytes());
    output.extend_from_slice(input);
    Ok(())
}

pub(super) fn completed_with_output(value: ValueRef) -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: Some(
            BoundedValueRef::new(value, MAX_TEXT_BYTES)
                .expect("text transform output was checked against the admitted bound"),
        ),
        failure: None,
    }
}
