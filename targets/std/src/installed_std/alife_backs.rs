//! Installed production-kernel operations for the bounded Lenia family.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_alife::{
    LeniaBoundary, LeniaFieldView, LeniaParameters, LENIA_MAXIMUM_FIELD_BYTES, LENIA_Q16_ONE,
};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

pub(super) static ORBIUM_SEED_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::ORBIUM_SEED_IMPLEMENTATION,
    budget: seed_budget,
    prepare: prepare_seed,
};

pub(super) static LENIA_STEP_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::LENIA_STEP_IMPLEMENTATION,
    budget: lenia_budget,
    prepare: prepare_lenia,
};

pub(super) static SCALAR_FIELD_PRESENTATION_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::SCALAR_FIELD_PRESENTATION_IMPLEMENTATION,
    budget: presentation_budget,
    prepare: prepare_presentation,
};

pub(super) struct OrbiumSeedBack {
    value: ValueRef,
    emitted: bool,
}

pub(super) struct LeniaStepBack {
    initialized: bool,
    initial_closed: bool,
    pending: Option<Pending>,
    next_tick: u32,
}

pub(super) struct ScalarFieldPresentationBack {
    pending: Option<RequestId>,
    next: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Pending {
    Initialize(RequestId),
    Step(RequestId),
}

impl<const PORTS: usize> StepBack<PORTS> for OrbiumSeedBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), self.value)
            .expect("ready Orbium seed output");
        self.emitted = true;
        StepOutcome::Progress
    }
}

impl<const PORTS: usize> StepBack<PORTS> for LeniaStepBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            match self.pending {
                Some(Pending::Initialize(expected)) if expected == request => {
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                    {
                        return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                            code: FailureCode::HostCallFailed,
                            detail: 185,
                        }));
                    }
                    io.consume_host_completion()
                        .expect("observed Lenia initialization completion");
                    self.pending = None;
                    self.initialized = true;
                    return StepOutcome::Progress;
                }
                Some(Pending::Step(expected)) if expected == request => {
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                    {
                        return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                            code: FailureCode::HostCallFailed,
                            detail: 187,
                        }));
                    }
                    let Some(output) = outcome.output else {
                        return step_fail(FailureCode::HostCallFailed, 186);
                    };
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed Lenia Step completion");
                    io.send(PortId(0), output.value)
                        .expect("ready Lenia field output");
                    self.pending = None;
                    self.next_tick += 1;
                    return StepOutcome::Progress;
                }
                _ => return step_fail(FailureCode::InvalidLifecycle, 188),
            }
        }

        if let Some(value) = io.input(PortId(0)) {
            if self.initialized || self.pending.is_some() {
                return step_fail(FailureCode::InvalidLifecycle, 184);
            }
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return step_fail(FailureCode::InvalidInput, 181);
            };
            if LeniaFieldView::decode(canonical).is_err() {
                return step_fail(FailureCode::InvalidInput, 181);
            }
            let Ok(input) = BoundedValueRef::new(value, LENIA_MAXIMUM_FIELD_BYTES) else {
                return step_fail(FailureCode::InvalidInput, 181);
            };
            let request = RequestId(0);
            io.consume(PortId(0)).expect("present initial Lenia field");
            io.request_host_call(request, HostCallId(0), input)
                .expect("Lenia initialization Host Call");
            self.pending = Some(Pending::Initialize(request));
            return StepOutcome::Progress;
        }

        if let Some(value) = io.input(PortId(1)) {
            if !self.initialized
                || !self.initial_closed
                || self.pending.is_some()
                || self.next_tick >= u32::from(conduit_alife::MAXIMUM_PRESENTED_FIELDS)
            {
                return step_fail(FailureCode::InvalidLifecycle, 184);
            }
            let Some(canonical) = input_bytes.input(PortId(1)) else {
                return step_fail(FailureCode::InvalidInput, 182);
            };
            let Ok(sequence) = super::contract::decode_tick(canonical) else {
                return step_fail(FailureCode::InvalidInput, 182);
            };
            if sequence != u64::from(self.next_tick) {
                return step_fail(FailureCode::InvalidInput, 183);
            }
            let Ok(input) = BoundedValueRef::new(value, conduit_time::TICK_ENCODED_LEN) else {
                return step_fail(FailureCode::InvalidInput, 182);
            };
            let request = RequestId(self.next_tick + 1);
            io.consume(PortId(1)).expect("present Lenia tick");
            io.request_host_call(request, HostCallId(1), input)
                .expect("Lenia Step Host Call");
            self.pending = Some(Pending::Step(request));
            return StepOutcome::Progress;
        }

        if io.input_closed(PortId(0)) && self.initialized && !self.initial_closed {
            io.consume_closed(PortId(0))
                .expect("observed initial Lenia field closure");
            self.initial_closed = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(1))
            && self.initialized
            && self.initial_closed
            && self.pending.is_none()
        {
            io.consume_closed(PortId(1))
                .expect("observed Lenia tick closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl<const PORTS: usize> StepBack<PORTS> for ScalarFieldPresentationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(FailureCode::InvalidLifecycle, 190);
            }
            io.consume_host_completion()
                .expect("observed scalar-field Presentation completion");
            self.pending = None;
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.output.is_some() {
                return step_fail(FailureCode::HostCallFailed, 189);
            }
            self.next += 1;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some()
                || self.next >= u32::from(conduit_alife::MAXIMUM_PRESENTED_FIELDS)
            {
                return step_fail(FailureCode::InvalidLifecycle, 190);
            }
            let Ok(input) = BoundedValueRef::new(value, LENIA_MAXIMUM_FIELD_BYTES) else {
                return step_fail(FailureCode::InvalidInput, 190);
            };
            let request = RequestId(self.next);
            io.consume(PortId(0))
                .expect("present scalar-field Presentation input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("scalar-field Presentation Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed scalar-field Presentation closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl OrbiumSeedBack {}

impl LeniaStepBack {
    fn new() -> Self {
        Self {
            initialized: false,
            initial_closed: false,
            pending: None,
            next_tick: 0,
        }
    }
}

impl ScalarFieldPresentationBack {}

pub(super) fn parameters(placement: &PlannedGear) -> Result<LeniaParameters, String> {
    validate_lenia(placement)?;
    Ok(LeniaParameters {
        kernel_radius: u16::try_from(u64_configuration(
            placement,
            conduit_alife::KERNEL_RADIUS_KEY,
        )?)
        .map_err(|_| "Lenia kernel radius exceeds u16".to_string())?,
        kernel_mu_q16: scalar_q16(placement, conduit_alife::KERNEL_MU_KEY)?,
        kernel_sigma_q16: scalar_q16(placement, conduit_alife::KERNEL_SIGMA_KEY)?,
        growth_mu_q16: scalar_q16(placement, conduit_alife::GROWTH_MU_KEY)?,
        growth_sigma_q16: scalar_q16(placement, conduit_alife::GROWTH_SIGMA_KEY)?,
        dt_q16: scalar_q16(placement, conduit_alife::DT_KEY)?,
        boundary: LeniaBoundary::Wrap,
    })
}

fn seed_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_seed(placement)?;
    Ok(BackBudget {
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
) -> Result<InstalledBack, String> {
    validate_seed(placement)?;
    let width = u16::try_from(u64_configuration(
        placement,
        conduit_alife::ORBIUM_WIDTH_KEY,
    )?)
    .map_err(|_| "Orbium width exceeds u16".to_string())?;
    let height = u16::try_from(u64_configuration(
        placement,
        conduit_alife::ORBIUM_HEIGHT_KEY,
    )?)
    .map_err(|_| "Orbium height exceeds u16".to_string())?;
    let seed = u64_configuration(placement, conduit_alife::SEED_KEY)?;
    let encoded = conduit_alife::orbium_seed(width, height, seed)
        .map_err(|error| format!("construct Orbium seed: {error:?}"))?
        .encode()
        .map_err(|error| format!("encode Orbium seed: {error:?}"))?;
    let value = values
        .store(&encoded)
        .map_err(|error| format!("store Orbium seed: {error:?}"))?;
    Ok(InstalledBack::OrbiumSeed(OrbiumSeedBack {
        value,
        emitted: false,
    }))
}

fn lenia_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    parameters(placement)?;
    Ok(BackBudget {
        value_items: conduit_alife::MAXIMUM_PRESENTED_FIELDS,
        value_bytes: LENIA_MAXIMUM_FIELD_BYTES * u32::from(conduit_alife::MAXIMUM_PRESENTED_FIELDS),
        host_requests: usize::from(conduit_alife::MAXIMUM_PRESENTED_FIELDS) + 1,
        sign_items: 192,
        maximum_value_bytes: LENIA_MAXIMUM_FIELD_BYTES,
    })
}

fn prepare_lenia(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    parameters(placement)?;
    Ok(InstalledBack::LeniaStep(LeniaStepBack::new()))
}

fn presentation_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_presentation(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: usize::from(conduit_alife::MAXIMUM_PRESENTED_FIELDS),
        sign_items: 96,
        maximum_value_bytes: LENIA_MAXIMUM_FIELD_BYTES,
    })
}

fn prepare_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate_presentation(placement)?;
    Ok(InstalledBack::ScalarFieldPresentation(
        ScalarFieldPresentationBack {
            pending: None,
            next: 0,
        },
    ))
}

fn validate_seed(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::orbium_seed_offer();
    if placement.kind_id.as_str() != conduit_alife::ORBIUM_SEED_KIND
        || placement.kind_contract_revision.as_str() != conduit_alife::ORBIUM_SEED_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::ORBIUM_SEED_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::ORBIUM_SEED_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::ORBIUM_SEED_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.configuration.len() != 3
    {
        return Err("planned Orbium seed identity does not match its installation".into());
    }
    Ok(())
}

fn validate_lenia(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::lenia_step_offer();
    if placement.kind_id.as_str() != conduit_alife::LENIA_STEP_KIND
        || placement.kind_contract_revision.as_str() != conduit_alife::LENIA_STEP_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::LENIA_STEP_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::LENIA_STEP_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::LENIA_STEP_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.configuration.len() != 8
        || text_configuration(placement, conduit_alife::BOUNDARY_KEY)? != "wrap"
        || text_configuration(placement, conduit_alife::NUMERIC_PROFILE_KEY)? != "fixed-q16.16"
    {
        return Err("planned Lenia identity does not match its installation".into());
    }
    Ok(())
}

fn validate_presentation(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::scalar_field_presentation_offer();
    let minimum = scalar_raw(placement, conduit_alife::MINIMUM_KEY)?;
    let maximum = scalar_raw(placement, conduit_alife::MAXIMUM_KEY)?;
    if placement.kind_id.as_str() != conduit_alife::SCALAR_FIELD_PRESENTATION_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_alife::SCALAR_FIELD_PRESENTATION_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::SCALAR_FIELD_PRESENTATION_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::SCALAR_FIELD_PRESENTATION_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::SCALAR_FIELD_PRESENTATION_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.resources.len() != 1
        || placement.configuration.len() != 3
        || text_configuration(placement, conduit_alife::TITLE_KEY)?.len() > 64
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

#[cfg(test)]
#[path = "alife_backs_tests.rs"]
mod tests;
