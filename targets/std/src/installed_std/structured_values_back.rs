//! Installed exact structured literal and presentation operations.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    kind_id, ConfigurationValue, PlannedGear, PortDirection, PortTemporal, StructuredInfoValue,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static LITERAL_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::STRUCTURED_LITERAL_STD_IMPLEMENTATION,
    budget: literal_budget,
    prepare: prepare_literal,
};
pub(super) static PRESENTATION_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::STRUCTURED_PRESENTATION_STD_IMPLEMENTATION,
    budget: presentation_budget,
    prepare: prepare_presentation,
};

pub(super) struct StructuredLiteralBack {
    value: Option<ValueRef>,
}

impl<const PORTS: usize> StepBack<PORTS> for StructuredLiteralBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let Some(value) = self.value else {
            return StepOutcome::Complete;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), value)
            .expect("ready structured literal output");
        self.value = None;
        StepOutcome::Progress
    }
}

impl StructuredLiteralBack {}

pub(super) struct StructuredPresentationBack {
    pending: Option<RequestId>,
    next: u32,
}

impl<const PORTS: usize> StepBack<PORTS> for StructuredPresentationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return StepOutcome::Fail(step_failure(156));
            }
            io.consume_host_completion()
                .expect("observed structured Presentation completion");
            self.pending = None;
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.output.is_some() {
                return StepOutcome::Fail(step_failure(156));
            }
            return StepOutcome::Complete;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() {
                return StepOutcome::Fail(step_failure(156));
            }
            let Ok(input) = BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
            else {
                return StepOutcome::Fail(step_failure(155));
            };
            let request = RequestId(self.next);
            let Some(next) = self.next.checked_add(1) else {
                return StepOutcome::Fail(step_failure(154));
            };
            io.consume(PortId(0))
                .expect("present structured Presentation input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("single structured Presentation Host Call");
            self.next = next;
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

fn step_failure(detail: u16) -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    }
}

impl StructuredPresentationBack {}

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
    if placement.kind_id != kind_id(conduit_semantic_catalog::STRUCTURED_LITERAL_KIND)
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::STRUCTURED_LITERAL_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::STRUCTURED_LITERAL_STD_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::STRUCTURED_LITERAL_STD_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::STRUCTURED_LITERAL_STD_ARTIFACT
        || !placement.inputs.is_empty()
        || output.port_id.as_str() != "value"
        || output.direction != PortDirection::Output
        || output.temporal != PortTemporal::Value
        || !placement.host_calls.is_empty()
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
    if placement.kind_id != kind_id(conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND)
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::STRUCTURED_PRESENTATION_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::STRUCTURED_PRESENTATION_STD_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::STRUCTURED_PRESENTATION_STD_IMPLEMENTATION
        || placement.artifact_id.as_str()
            != conduit_std_offers::STRUCTURED_PRESENTATION_STD_ARTIFACT
        || input.port_id.as_str() != "input"
        || input.direction != PortDirection::Input
        || input.temporal != PortTemporal::Value
        || !placement.outputs.is_empty()
        || placement.host_calls.len() != 1
        || placement.host_calls[0].target_kind.as_ref()
            != Some(&kind_id(
                conduit_semantic_catalog::STRUCTURED_PRESENTATION_TARGET,
            ))
        || placement.resources.len() != 1
        || !placement.configuration.is_empty()
    {
        return Err("planned structured presentation differs from its installation".into());
    }
    Ok(())
}

fn literal_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_literal(placement)?;
    let maximum = configured(placement)?.len() as u32;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: maximum.saturating_mul(2),
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: maximum,
    })
}

fn presentation_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_presentation(placement)?;
    Ok(BackBudget {
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
) -> Result<InstalledBack, String> {
    validate_literal(placement)?;
    let value = values
        .store(configured(placement)?)
        .map_err(|error| format!("store structured literal: {error:?}"))?;
    Ok(InstalledBack::StructuredLiteral(StructuredLiteralBack {
        value: Some(value),
    }))
}

fn prepare_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate_presentation(placement)?;
    Ok(InstalledBack::StructuredPresentation(
        StructuredPresentationBack {
            pending: None,
            next: 0,
        },
    ))
}
