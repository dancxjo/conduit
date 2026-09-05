use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, Scalar, SCALAR_ENCODED_LEN};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
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

impl MathScalarOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none()
                && !self.completed
                && value.byte_len == self.input_bytes =>
            {
                let request = RequestId(0);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, self.input_bytes) {
                        Ok(input) => input,
                        Err(_) => return InstalledOperation::fail(25),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return InstalledOperation::fail(25);
                };
                if output.admitted_bytes != self.output_bytes
                    || output.value.byte_len != self.output_bytes
                {
                    return InstalledOperation::fail(25);
                }
                self.pending = None;
                self.completed = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                self.completed = true;
                OperationAction::Complete
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Failed
                    && outcome.output.is_none() =>
            {
                self.pending = None;
                self.completed = true;
                match outcome.failure {
                    Some(failure) => OperationAction::Fail(failure),
                    None => InstalledOperation::fail(25),
                }
            }
            _ => InstalledOperation::fail(25),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.completed = true;
    }
}

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
        .map_err(|error| format!("math input is not canonical value/scalar@1: {error:?}"))?;
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
        || placement.host_operations != offer.host_operations
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
