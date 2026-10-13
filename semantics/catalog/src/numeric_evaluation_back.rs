//! A selected closed numerical computation emits its verified prepared output.
use conduit_core::numeric_realization::PreparedNumericOutput;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};

pub struct NumericEvaluationBack {
    output: PreparedNumericOutput,
    emitted: bool,
}
impl NumericEvaluationBack {
    pub fn new(output: PreparedNumericOutput) -> Self {
        Self {
            output,
            emitted: false,
        }
    }
    pub fn evidence(&self) -> &conduit_core::numeric_realization::VerifiedNumericWitness {
        self.output.witness()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for NumericEvaluationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io
            .send_prepared(PortId(0), self.output.canonical().len() as u32)
            .is_err()
        {
            return StepOutcome::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidInput,
                detail: 5392,
            });
        }
        self.emitted = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| self.output.canonical())
    }
    fn cancel(&mut self) {
        self.emitted = true;
    }
}

/// Readmit exactly the graph, requirement, reviewed implementation and selected
/// witness retained by the ordinary Plan before constructing any Play state.
#[cfg(feature = "plot-catalog")]
pub fn prepare_numeric_evaluation(
    placement: &conduit_core::PlannedGear,
) -> Result<PreparedNumericOutput, alloc::string::String> {
    use alloc::format;
    use conduit_core::*;
    if placement.artifact_id != numeric_realization::numeric_engine_artifact_id() {
        return Err("numeric implementation artifact is not the reviewed emitter".into());
    }
    let graph = conduit_plot::planned_gear_numeric_computation(placement)
        .map_err(|error| format!("numeric source custody: {error:?}"))?;
    let requirement = match placement
        .configuration
        .iter()
        .find(|entry| entry.key == "numeric_requirement")
        .map(|entry| &entry.value)
    {
        Some(ConfigurationValue::Structured(value)) => {
            NumericRequirement::from_configuration(value)
                .map_err(|error| format!("numeric requirement: {error:?}"))?
        }
        _ => return Err("numeric requirement capsule is absent".into()),
    };
    let one = |profile: &KindId| {
        let mut values = placement
            .realization_properties
            .iter()
            .filter(|value| value.profile() == profile);
        let value = values.next().ok_or("selected numeric property is absent")?;
        if values.next().is_some() {
            return Err("selected numeric property is duplicated");
        }
        Ok(value)
    };
    let offer =
        NumericRealizationOffer::from_configuration(one(&numeric_realization_offer_profile())?)
            .map_err(|error| format!("numeric offer: {error:?}"))?;
    let witness = verify_numeric_witness_configuration(
        one(&numeric_realization_property_profile())?,
        &graph,
        &requirement,
    )
    .map_err(|error| format!("numeric witness: {error:?}"))?;
    if witness.claim().algorithm != offer.algorithm
        || witness.claim().storage_bytes > offer.budget.storage_bytes
        || witness.claim().work_steps > offer.budget.work_steps
        || witness.required_limb_capacity() > offer.budget.limbs
        || witness.required_symbolic_nodes() > offer.budget.symbolic_nodes
    {
        return Err("selected numeric witness exceeds the admitted algorithm profile".into());
    }
    let reserved = placement
        .resources
        .iter()
        .filter(|binding| binding.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
        .try_fold(0u64, |total, binding| {
            total.checked_add(u64::from(binding.units))
        })
        .ok_or("numeric memory reservation overflow")?;
    let required = witness
        .required_memory_reservation_bytes(&graph)
        .map_err(|error| format!("numeric reservation: {error:?}"))?;
    if reserved < u64::from(required) {
        return Err("numeric Plan memory reservation is insufficient".into());
    }
    PreparedNumericOutput::new(&graph, witness)
        .map_err(|error| format!("numeric output: {error:?}"))
}
