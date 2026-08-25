//! Installed production-kernel operations for the bounded Lenia family.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ConfigurationValue, LeniaBoundary, LeniaFieldView, LeniaParameters, PlannedGear,
    LENIA_MAXIMUM_FIELD_BYTES, LENIA_Q16_ONE,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) static ORBIUM_SEED_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::ORBIUM_SEED_IMPLEMENTATION,
    budget: seed_budget,
    prepare: prepare_seed,
};

pub(super) static LENIA_STEP_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::LENIA_STEP_IMPLEMENTATION,
    budget: lenia_budget,
    prepare: prepare_lenia,
};

pub(super) static SCALAR_FIELD_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::SCALAR_FIELD_PRESENTATION_IMPLEMENTATION,
    budget: presentation_budget,
    prepare: prepare_presentation,
};

pub(super) struct OrbiumSeedOperation {
    value: ValueRef,
    emitted: bool,
}

pub(super) struct LeniaStepOperation {
    initialized: bool,
    initial_closed: bool,
    pending: Option<Pending>,
    next_tick: u32,
}

pub(super) struct ScalarFieldPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Pending {
    Initialize(RequestId),
    Step(RequestId),
}

impl OrbiumSeedOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Emit {
            port: PortId(0),
            value: self.value,
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            InstalledOperation::fail(180)
        } else {
            self.emitted = true;
            OperationAction::Complete
        }
    }
}

impl LeniaStepOperation {
    fn new() -> Self {
        Self {
            initialized: false,
            initial_closed: false,
            pending: None,
            next_tick: 0,
        }
    }

    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        if port == PortId(0) && !self.initialized && self.pending.is_none() {
            if LeniaFieldView::decode(canonical).is_err() {
                return fail(FailureCode::InvalidInput, 181);
            }
            let request = RequestId(0);
            self.pending = Some(Pending::Initialize(request));
            return request_action(
                request,
                HostOperationId(0),
                value,
                LENIA_MAXIMUM_FIELD_BYTES,
            );
        }
        if port == PortId(1)
            && self.initialized
            && self.initial_closed
            && self.pending.is_none()
            && self.next_tick < u32::from(conduit_std_catalog::MAXIMUM_PRESENTED_FIELDS)
        {
            let Ok(sequence) = super::contract::decode_tick(canonical) else {
                return fail(FailureCode::InvalidInput, 182);
            };
            if sequence != u64::from(self.next_tick) {
                return fail(FailureCode::InvalidInput, 183);
            }
            let request_id = RequestId(self.next_tick + 1);
            self.pending = Some(Pending::Step(request_id));
            return request_action(
                request_id,
                HostOperationId(1),
                value,
                conduit_std_catalog::TICK_ENCODED_LEN,
            );
        }
        fail(FailureCode::InvalidLifecycle, 184)
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(Pending::Initialize(request)) =>
            {
                self.pending = None;
                if outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none()
                {
                    self.initialized = true;
                    OperationAction::Await
                } else {
                    host_failure(outcome.failure, 185)
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(Pending::Step(request)) =>
            {
                self.pending = None;
                if outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none()
                {
                    let Some(output) = outcome.output else {
                        return fail(FailureCode::HostOperationFailed, 186);
                    };
                    self.next_tick += 1;
                    OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    }
                } else {
                    host_failure(outcome.failure, 187)
                }
            }
            OperationInput::Closed { port: PortId(1) }
                if self.initialized && self.initial_closed && self.pending.is_none() =>
            {
                OperationAction::Complete
            }
            OperationInput::Closed { port: PortId(0) }
                if self.initialized && !self.initial_closed && self.pending.is_none() =>
            {
                self.initial_closed = true;
                OperationAction::Await
            }
            _ => fail(FailureCode::InvalidLifecycle, 188),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

impl ScalarFieldPresentationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none()
                && self.next < u32::from(conduit_std_catalog::MAXIMUM_PRESENTED_FIELDS) =>
            {
                let request = RequestId(self.next);
                self.pending = Some(request);
                request_action(
                    request,
                    HostOperationId(0),
                    value,
                    LENIA_MAXIMUM_FIELD_BYTES,
                )
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.next += 1;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            OperationInput::HostOperationCompleted { outcome, .. } => {
                host_failure(outcome.failure, 189)
            }
            _ => fail(FailureCode::InvalidLifecycle, 190),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

pub(super) fn parameters(placement: &PlannedGear) -> Result<LeniaParameters, String> {
    validate_lenia(placement)?;
    Ok(LeniaParameters {
        kernel_radius: u16::try_from(u64_configuration(
            placement,
            conduit_std_catalog::KERNEL_RADIUS_KEY,
        )?)
        .map_err(|_| "Lenia kernel radius exceeds u16".to_string())?,
        kernel_mu_q16: scalar_q16(placement, conduit_std_catalog::KERNEL_MU_KEY)?,
        kernel_sigma_q16: scalar_q16(placement, conduit_std_catalog::KERNEL_SIGMA_KEY)?,
        growth_mu_q16: scalar_q16(placement, conduit_std_catalog::GROWTH_MU_KEY)?,
        growth_sigma_q16: scalar_q16(placement, conduit_std_catalog::GROWTH_SIGMA_KEY)?,
        dt_q16: scalar_q16(placement, conduit_std_catalog::DT_KEY)?,
        boundary: LeniaBoundary::Wrap,
    })
}

fn seed_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_seed(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: LENIA_MAXIMUM_FIELD_BYTES,
        host_requests: 0,
        sign_items: 24,
        maximum_value_bytes: LENIA_MAXIMUM_FIELD_BYTES,
    })
}

fn prepare_seed(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_seed(placement)?;
    let width = u16::try_from(u64_configuration(
        placement,
        conduit_std_catalog::ORBIUM_WIDTH_KEY,
    )?)
    .map_err(|_| "Orbium width exceeds u16".to_string())?;
    let height = u16::try_from(u64_configuration(
        placement,
        conduit_std_catalog::ORBIUM_HEIGHT_KEY,
    )?)
    .map_err(|_| "Orbium height exceeds u16".to_string())?;
    let seed = u64_configuration(placement, conduit_std_catalog::SEED_KEY)?;
    let encoded = conduit_core::orbium_seed(width, height, seed)
        .map_err(|error| format!("construct Orbium seed: {error:?}"))?
        .encode()
        .map_err(|error| format!("encode Orbium seed: {error:?}"))?;
    let value = values
        .store(&encoded)
        .map_err(|error| format!("store Orbium seed: {error:?}"))?;
    Ok(InstalledOperation::OrbiumSeed(OrbiumSeedOperation {
        value,
        emitted: false,
    }))
}

fn lenia_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    parameters(placement)?;
    Ok(OperationBudget {
        value_items: conduit_std_catalog::MAXIMUM_PRESENTED_FIELDS,
        value_bytes: LENIA_MAXIMUM_FIELD_BYTES
            * u32::from(conduit_std_catalog::MAXIMUM_PRESENTED_FIELDS),
        host_requests: usize::from(conduit_std_catalog::MAXIMUM_PRESENTED_FIELDS) + 1,
        sign_items: 192,
        maximum_value_bytes: LENIA_MAXIMUM_FIELD_BYTES,
    })
}

fn prepare_lenia(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    parameters(placement)?;
    Ok(InstalledOperation::LeniaStep(LeniaStepOperation::new()))
}

fn presentation_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_presentation(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: usize::from(conduit_std_catalog::MAXIMUM_PRESENTED_FIELDS),
        sign_items: 96,
        maximum_value_bytes: LENIA_MAXIMUM_FIELD_BYTES,
    })
}

fn prepare_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_presentation(placement)?;
    Ok(InstalledOperation::ScalarFieldPresentation(
        ScalarFieldPresentationOperation {
            pending: None,
            next: 0,
        },
    ))
}

fn validate_seed(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_catalog::orbium_seed_offer();
    if placement.kind_id.as_str() != conduit_std_catalog::ORBIUM_SEED_KIND
        || placement.kind_contract_revision.as_str() != conduit_std_catalog::ORBIUM_SEED_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_catalog::ORBIUM_SEED_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_catalog::ORBIUM_SEED_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_catalog::ORBIUM_SEED_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.configuration.len() != 3
    {
        return Err("planned Orbium seed identity does not match its installation".into());
    }
    Ok(())
}

fn validate_lenia(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_catalog::lenia_step_offer();
    if placement.kind_id.as_str() != conduit_std_catalog::LENIA_STEP_KIND
        || placement.kind_contract_revision.as_str() != conduit_std_catalog::LENIA_STEP_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_catalog::LENIA_STEP_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_catalog::LENIA_STEP_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_catalog::LENIA_STEP_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.configuration.len() != 8
        || text_configuration(placement, conduit_std_catalog::BOUNDARY_KEY)? != "wrap"
        || text_configuration(placement, conduit_std_catalog::NUMERIC_PROFILE_KEY)?
            != "fixed-q16.16"
    {
        return Err("planned Lenia identity does not match its installation".into());
    }
    Ok(())
}

fn validate_presentation(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_catalog::scalar_field_presentation_offer();
    let minimum = scalar_raw(placement, conduit_std_catalog::MINIMUM_KEY)?;
    let maximum = scalar_raw(placement, conduit_std_catalog::MAXIMUM_KEY)?;
    if placement.kind_id.as_str() != conduit_std_catalog::SCALAR_FIELD_PRESENTATION_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_std_catalog::SCALAR_FIELD_PRESENTATION_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_catalog::SCALAR_FIELD_PRESENTATION_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_catalog::SCALAR_FIELD_PRESENTATION_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_catalog::SCALAR_FIELD_PRESENTATION_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.resources.len() != 1
        || placement.configuration.len() != 3
        || text_configuration(placement, conduit_std_catalog::TITLE_KEY)?.len() > 64
        || minimum >= maximum
    {
        return Err(
            "planned scalar-field presentation identity does not match its installation".into(),
        );
    }
    Ok(())
}

fn u64_configuration(placement: &PlannedGear, key: &str) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            (actual, ConfigurationValue::U64(value)) if actual == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("missing or invalid configuration '{key}'"))
}

fn scalar_raw(placement: &PlannedGear, key: &str) -> Result<i64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            (actual, ConfigurationValue::I64(value)) if actual == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("missing or invalid scalar configuration '{key}'"))
}

fn scalar_q16(placement: &PlannedGear, key: &str) -> Result<u32, String> {
    let value = scalar_raw(placement, key)?;
    if !(0..=conduit_core::Scalar::SCALE).contains(&value) {
        return Err(format!("scalar configuration '{key}' is outside [0,1]"));
    }
    u32::try_from(
        (i128::from(value) * i128::from(LENIA_Q16_ONE)
            + i128::from(conduit_core::Scalar::SCALE / 2))
            / i128::from(conduit_core::Scalar::SCALE),
    )
    .map_err(|_| format!("scalar configuration '{key}' overflows Q16.16"))
}

fn text_configuration<'a>(placement: &'a PlannedGear, key: &str) -> Result<&'a str, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            (actual, ConfigurationValue::Text(value)) if actual == key => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(|| format!("missing or invalid text configuration '{key}'"))
}

fn request_action(
    request: RequestId,
    operation: HostOperationId,
    value: ValueRef,
    maximum: u32,
) -> OperationAction {
    match BoundedValueRef::new(value, maximum) {
        Ok(input) => OperationAction::RequestHostOperation {
            request,
            operation,
            input,
        },
        Err(_) => fail(FailureCode::InvalidInput, 191),
    }
}

fn host_failure(failure: Option<Failure>, detail: u16) -> OperationAction {
    OperationAction::Fail(failure.unwrap_or(Failure {
        code: FailureCode::HostOperationFailed,
        detail,
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
#[path = "alife_operations_tests.rs"]
mod tests;
