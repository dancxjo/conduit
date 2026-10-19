//! One checked expression Gear executed by an existing child kernel.
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{
    ConfigurationValue, ImplementationId, PlannedGear, PortTemporal,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, HostedValueStore, PortId,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};

pub struct LocalExpressionFactory;
impl KernelOperationFactory for LocalExpressionFactory {
    fn implementation_id(&self) -> &ImplementationId {
        static ID: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| conduit_std_offers::PURE_EXPRESSION_LOCAL_IMPLEMENTATION.into())
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let program = validate(placement)?;
        let (input, output) = bounds(&program)?;
        Ok(KernelOperationBudget {
            value_items: 2,
            value_bytes: input
                .checked_add(output)
                .ok_or("expression value budget overflow")?
                .max(1),
            maximum_value_bytes: input.max(output).max(1),
            host_requests: 0,
            sign_items: 4,
        })
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        Ok(Box::new(PreparedLocalExpressionBack::from_placement(
            placement,
        )?))
    }
}
pub(super) fn validate(placement: &PlannedGear) -> Result<PortableExpressionProgram, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("local expression requires one exact program".into());
    };
    let ("program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err("local expression requires canonical program text".into());
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|e| format!("local expression program: {e:?}"))?;
    let expected = match placement.inputs.first().map(|port| port.temporal) {
        Some(PortTemporal::Value) => conduit_std_offers::pure_expression_local_offer(&program),
        Some(PortTemporal::Flow { closes: true }) => {
            conduit_std_offers::pure_expression_local_flow_offer(&program)
        }
        _ => return Err("local expression requires Value or closing Flow".into()),
    }
    .map_err(|e| format!("local expression Fore: {e:?}"))?;
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || placement.host_calls != expected.host_calls
        || placement.base.is_some()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.realization_characteristics.is_empty()
        || !placement.terminal_transductions.is_empty()
    {
        return Err("planned local expression differs from its exact computational profile".into());
    }
    bounds(&program)?;
    Ok(program)
}
fn bounds(program: &PortableExpressionProgram) -> Result<(u32, u32), String> {
    let input = program
        .maximum_prepared_input_bytes()
        .map_err(|e| format!("expression input bound: {e:?}"))?;
    let output = program
        .maximum_prepared_output_bytes()
        .map_err(|e| format!("expression output bound: {e:?}"))?;
    if input as usize > MAXIMUM_STRUCTURED_CANONICAL_BYTES
        || output as usize > MAXIMUM_STRUCTURED_CANONICAL_BYTES
    {
        return Err("local expression exceeds canonical transport bound".into());
    }
    Ok((input, output))
}

/// Owns the existing prepared evaluator; every allocation occurs in preparation.
/// One committed Value invocation completes this Back; closing Flow invocations reuse its fixed storage until close. The activation owner
/// admits and prepares another instance when another child Play is required.
pub struct PreparedLocalExpressionBack {
    evaluator: PreparedPortableExpressionEvaluator,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
    staged: bool,
    complete: bool,
    flow: bool,
}
impl PreparedLocalExpressionBack {
    pub fn prepare(program: &PortableExpressionProgram) -> Result<Self, String> {
        let (maximum_input_bytes, maximum_output_bytes) = bounds(program)?;
        Ok(Self {
            evaluator: PreparedPortableExpressionEvaluator::new(program)
                .map_err(|e| format!("prepare local expression: {e:?}"))?,
            maximum_input_bytes,
            maximum_output_bytes,
            staged: false,
            complete: false,
            flow: false,
        })
    }
    pub(crate) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let mut back = Self::prepare(&validate(placement)?)?;
        back.flow = matches!(
            placement.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        Ok(back)
    }
    pub fn output_capacity(&self) -> usize {
        self.evaluator.output_capacity()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for PreparedLocalExpressionBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        // Discard only uncommitted intent on a retried transaction.
        self.staged = false;
        if self.complete {
            return StepOutcome::Complete;
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(1640);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if self.flow {
                    if io.consume_closed(PortId(0)).is_err() {
                        return fail(1641);
                    }
                    StepOutcome::Complete
                } else {
                    fail(1641)
                }
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(input) = inputs.input(PortId(0)) else {
            return fail(1642);
        };
        if input.len() != reference.byte_len as usize
            || input.len() > self.maximum_input_bytes as usize
        {
            return fail(1643);
        }
        let Ok(output) = self.evaluator.evaluate(input) else {
            return fail(1644);
        };
        if output.len() > self.maximum_output_bytes as usize {
            return fail(1645);
        }
        if io.send_prepared(PortId(0), output.len() as u32).is_err()
            || io.consume(PortId(0)).is_err()
        {
            return fail(1646);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then(|| self.evaluator.output_bytes())
    }
    fn step_committed(&mut self) {
        self.complete |= self.staged && !self.flow;
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.complete = true;
        self.staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
