//! One checked expression Gear executed by an existing child kernel.
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{ConfigurationValue, ImplementationId, PlannedGear, PortTemporal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostedValueStore, PortId,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_plot::PortableExpressionProgram;

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
// Keep the installed Std profile's validation and public constructor paths.
use conduit_semantic_catalog::{prepared_expression_bounds as bounds, PreparedExpressionBack};
pub struct PreparedLocalExpressionBack(PreparedExpressionBack);
impl PreparedLocalExpressionBack {
    pub fn prepare(program: &PortableExpressionProgram) -> Result<Self, String> {
        PreparedExpressionBack::prepare(program).map(Self)
    }
    pub(crate) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let program = validate(placement)?;
        PreparedExpressionBack::prepare_with_temporal(&program, placement.inputs[0].temporal)
            .map(Self)
    }
    pub fn output_capacity(&self) -> usize {
        self.0.output_capacity()
    }
}
impl<const P: usize> StepBack<P> for PreparedLocalExpressionBack {
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        self.0.step(io, inputs)
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        <PreparedExpressionBack as StepBack<P>>::prepared_output(&self.0, port)
    }
    fn step_committed(&mut self) {
        <PreparedExpressionBack as StepBack<P>>::step_committed(&mut self.0);
    }
    fn cancel(&mut self) {
        <PreparedExpressionBack as StepBack<P>>::cancel(&mut self.0);
    }
}
