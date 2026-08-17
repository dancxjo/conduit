//! Installed exact structured literal and presentation operations.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, ConfigurationValue, PlannedGear, PortDirection, PortTemporal, StructuredInfoValue,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static LITERAL_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::STRUCTURED_LITERAL_STD_IMPLEMENTATION,
    budget: literal_budget,
    prepare: prepare_literal,
};
pub(super) static PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::STRUCTURED_PRESENTATION_STD_IMPLEMENTATION,
    budget: presentation_budget,
    prepare: prepare_presentation,
};

pub(super) struct StructuredLiteralOperation {
    value: Option<ValueRef>,
}

impl StructuredLiteralOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.value
            .take()
            .map(|value| OperationAction::Emit {
                port: PortId(0),
                value,
            })
            .unwrap_or(OperationAction::Complete)
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }
}

pub(super) struct StructuredPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
}

impl StructuredPresentationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let request = RequestId(self.next);
                let Some(next) = self.next.checked_add(1) else {
                    return InstalledOperation::fail(154);
                };
                self.next = next;
                self.pending = Some(request);
                let Ok(input) =
                    BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
                else {
                    return InstalledOperation::fail(155);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(156),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

fn configured(placement: &PlannedGear) -> Result<&[u8], String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("structured literal requires one exact value".into());
    };
    let ("value", ConfigurationValue::Structured(value)) = (entry.key.as_str(), &entry.value)
    else {
        return Err("structured literal value is not structured Info".into());
    };
    let output = placement
        .outputs
        .first()
        .ok_or_else(|| "structured literal output is missing".to_string())?;
    if value.profile() != &output.value_kind
        || StructuredInfoValue::from_canonical_bytes(value.canonical_value()).is_err()
    {
        return Err("structured literal profile and value disagree".into());
    }
    Ok(value.canonical_value())
}

fn validate_literal(placement: &PlannedGear) -> Result<(), String> {
    let [output] = placement.outputs.as_slice() else {
        return Err("structured literal requires one output".into());
    };
    if placement.kind_id != kind_id(conduit_std_catalog::STRUCTURED_LITERAL_KIND)
        || placement.kind_contract_revision.as_str()
            != conduit_std_catalog::STRUCTURED_LITERAL_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_catalog::STRUCTURED_LITERAL_STD_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_catalog::STRUCTURED_LITERAL_STD_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_catalog::STRUCTURED_LITERAL_STD_ARTIFACT
        || !placement.inputs.is_empty()
        || output.port_id.as_str() != "value"
        || output.direction != PortDirection::Output
        || output.temporal != PortTemporal::Value
        || !placement.host_operations.is_empty()
        || !placement.resources.is_empty()
    {
        return Err("planned structured literal differs from its installation".into());
    }
    configured(placement)?;
    Ok(())
}

fn validate_presentation(placement: &PlannedGear) -> Result<(), String> {
    let [input] = placement.inputs.as_slice() else {
        return Err("structured presentation requires one input".into());
    };
    if placement.kind_id != kind_id(conduit_std_catalog::STRUCTURED_PRESENTATION_KIND)
        || placement.kind_contract_revision.as_str()
            != conduit_std_catalog::STRUCTURED_PRESENTATION_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_catalog::STRUCTURED_PRESENTATION_STD_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_catalog::STRUCTURED_PRESENTATION_STD_IMPLEMENTATION
        || placement.artifact_id.as_str()
            != conduit_std_catalog::STRUCTURED_PRESENTATION_STD_ARTIFACT
        || input.port_id.as_str() != "input"
        || input.direction != PortDirection::Input
        || input.temporal != PortTemporal::Value
        || !placement.outputs.is_empty()
        || placement.host_operations.len() != 1
        || placement.host_operations[0].target_kind.as_ref()
            != Some(&kind_id(
                conduit_std_catalog::STRUCTURED_PRESENTATION_TARGET,
            ))
        || placement.resources.len() != 1
        || !placement.configuration.is_empty()
    {
        return Err("planned structured presentation differs from its installation".into());
    }
    Ok(())
}

fn literal_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_literal(placement)?;
    let maximum = configured(placement)?.len() as u32;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: maximum.saturating_mul(2),
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: maximum,
    })
}

fn presentation_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_presentation(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare_literal(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_literal(placement)?;
    let value = values
        .store(configured(placement)?)
        .map_err(|error| format!("store structured literal: {error:?}"))?;
    Ok(InstalledOperation::StructuredLiteral(
        StructuredLiteralOperation { value: Some(value) },
    ))
}

fn prepare_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_presentation(placement)?;
    Ok(InstalledOperation::StructuredPresentation(
        StructuredPresentationOperation {
            pending: None,
            next: 0,
        },
    ))
}
