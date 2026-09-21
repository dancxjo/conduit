use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, Scalar, SCALAR_ENCODED_LEN};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static MATH_CLAMP_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::MATH_CLAMP_IMPLEMENTATION,
    budget: clamp_budget,
    prepare: prepare_clamp,
};
pub(super) static MATH_SCALE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::MATH_SCALE_IMPLEMENTATION,
    budget: scale_budget,
    prepare: prepare_scale,
};
pub(super) static MATH_DEADBAND_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::MATH_DEADBAND_IMPLEMENTATION,
    budget: deadband_budget,
    prepare: prepare_deadband,
};

pub(super) static QUANTITY_MAP_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::QUANTITY_MAP_IMPLEMENTATION,
    budget: quantity_budget,
    prepare: prepare_quantity,
};

pub(super) static QUANTITY_INFO_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::QUANTITY_INFO_IMPLEMENTATION,
    budget: quantity_info_budget,
    prepare: prepare_quantity_info,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MathTransform {
    Clamp { minimum: Scalar, maximum: Scalar },
    Scale { gain: Scalar },
    Deadband { radius: Scalar },
}

impl<const PORTS: usize> StepBack<PORTS> for MathScalarOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.completed {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return StepOutcome::Fail(math_failure());
            }
            if outcome.disposition == HostCallDisposition::Failed && outcome.output.is_none() {
                io.consume_host_completion()
                    .expect("observed failed math Host Call");
                self.pending = None;
                self.completed = true;
                return StepOutcome::Fail(outcome.failure.unwrap_or_else(math_failure));
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
                return StepOutcome::Fail(math_failure());
            }
            let Some(output) = outcome.output else {
                return StepOutcome::Fail(math_failure());
            };
            if output.admitted_bytes != self.output_bytes
                || output.value.byte_len != self.output_bytes
            {
                return StepOutcome::Fail(math_failure());
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed completed math Host Call");
            io.send(PortId(0), output.value).expect("ready math output");
            self.pending = None;
            self.completed = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || value.byte_len != self.input_bytes {
                return StepOutcome::Fail(math_failure());
            }
            let Ok(input) = BoundedValueRef::new(value, self.input_bytes) else {
                return StepOutcome::Fail(math_failure());
            };
            let request = RequestId(0);
            io.consume(PortId(0)).expect("present math input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("single math Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed math input closure");
            self.completed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.completed = true;
    }
}

fn math_failure() -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail: 25,
    }
}

impl MathTransform {
    pub(super) fn apply(
        self,
        input: Scalar,
    ) -> Result<Scalar, conduit_semantic_catalog::MathScalarError> {
        match self {
            Self::Clamp { minimum, maximum } => {
                conduit_semantic_catalog::clamp_scalar(input, minimum, maximum)
            }
            Self::Scale { gain } => conduit_semantic_catalog::scale_scalar(input, gain),
            Self::Deadband { radius } => conduit_semantic_catalog::deadband_scalar(input, radius),
        }
    }
}

pub(super) struct MathScalarOperation {
    pending: Option<RequestId>,
    completed: bool,
    input_bytes: u32,
    output_bytes: u32,
}

impl MathScalarOperation {}

pub(super) fn transform_for(placement: &PlannedGear) -> Result<MathTransform, String> {
    match placement.kind_id.as_str() {
        conduit_semantic_catalog::MATH_CLAMP_KIND => {
            let minimum =
                scalar_configuration(placement, conduit_semantic_catalog::CLAMP_MINIMUM_KEY)?;
            let maximum =
                scalar_configuration(placement, conduit_semantic_catalog::CLAMP_MAXIMUM_KEY)?;
            conduit_semantic_catalog::clamp_scalar(Scalar::ZERO, minimum, maximum)
                .map_err(|_| "math/clamp minimum exceeds maximum".to_string())?;
            Ok(MathTransform::Clamp { minimum, maximum })
        }
        conduit_semantic_catalog::MATH_SCALE_KIND => Ok(MathTransform::Scale {
            gain: scalar_configuration(placement, conduit_semantic_catalog::SCALE_GAIN_KEY)?,
        }),
        conduit_semantic_catalog::MATH_DEADBAND_KIND => {
            let radius =
                scalar_configuration(placement, conduit_semantic_catalog::DEADBAND_RADIUS_KEY)?;
            conduit_semantic_catalog::deadband_scalar(Scalar::ZERO, radius)
                .map_err(|_| "math/deadband radius must be nonnegative".to_string())?;
            Ok(MathTransform::Deadband { radius })
        }
        _ => Err("unsupported installed scalar transform".into()),
    }
}

pub(super) fn transform_bytes(
    placement: &PlannedGear,
    input: &[u8],
) -> Result<[u8; SCALAR_ENCODED_LEN], String> {
    let input = Scalar::decode(input)
        .map_err(|error| format!("math input is not canonical value/scalar: {error:?}"))?;
    transform_for(placement)?
        .apply(input)
        .map(Scalar::encode)
        .map_err(|error| format!("math scalar transform failed: {error:?}"))
}

fn scalar_configuration(placement: &PlannedGear, key: &str) -> Result<Scalar, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::I64(value)) if found == key => {
                Some(Scalar::from_raw_microunits(*value))
            }
            _ => None,
        })
        .ok_or_else(|| format!("math scalar configuration '{key}' is missing"))
}

fn clamp_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_offers::math_clamp_offer(), 2)?;
    transform_for(placement)?;
    Ok(budget())
}

fn scale_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_offers::math_scale_offer(), 1)?;
    transform_for(placement)?;
    Ok(budget())
}

fn deadband_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_offers::math_deadband_offer(), 1)?;
    transform_for(placement)?;
    Ok(budget())
}

fn budget() -> OperationBudget {
    OperationBudget {
        value_items: 1,
        value_bytes: SCALAR_ENCODED_LEN as u32,
        host_requests: 1,
        sign_items: 64,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    }
}

fn prepare_clamp(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    clamp_budget(placement)?;
    Ok(operation())
}

fn prepare_scale(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    scale_budget(placement)?;
    Ok(operation())
}

fn prepare_deadband(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    deadband_budget(placement)?;
    Ok(operation())
}

fn operation() -> InstalledOperation {
    InstalledOperation::MathScalar(MathScalarOperation {
        pending: None,
        completed: false,
        input_bytes: SCALAR_ENCODED_LEN as u32,
        output_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn quantity_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_offers::quantity_map_offer(), 8)?;
    super::quantity_mapping::configuration(placement)?;
    Ok(OperationBudget {
        value_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
        maximum_value_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
        ..budget()
    })
}

fn prepare_quantity(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    quantity_budget(placement)?;
    Ok(InstalledOperation::MathScalar(MathScalarOperation {
        pending: None,
        completed: false,
        input_bytes: SCALAR_ENCODED_LEN as u32,
        output_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
    }))
}

fn quantity_info_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_offers::quantity_info_offer(), 0)?;
    Ok(OperationBudget {
        value_bytes: conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
        maximum_value_bytes: conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
        ..budget()
    })
}

fn prepare_quantity_info(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    quantity_info_budget(placement)?;
    Ok(InstalledOperation::MathScalar(MathScalarOperation {
        pending: None,
        completed: false,
        input_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
        output_bytes: (conduit_semantic_catalog::quantity_info_prefix().len()
            + conduit_core::QUANTITY_ENCODED_LEN) as u32,
    }))
}

fn validate(
    placement: &PlannedGear,
    offer: &conduit_core::CapabilityOffer,
    configuration: usize,
) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || placement.configuration.len() != configuration
    {
        return Err("planned math executable identity does not match its installation".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "math_operations_tests.rs"]
mod tests;
