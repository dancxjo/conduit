//! Exact prepared profile admission for the ordinary PCM packing Gear.
use super::PreparedPcm16PackingBack;
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{ConfigurationValue, ImplementationId, PlannedGear};
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub struct Pcm16PackingFactory;
fn validate(p: &PlannedGear) -> Result<u16, String> {
    let expected = conduit_std_offers::pcm16_packing_offer();
    if p.kind_id != expected.kind_id
        || p.kind_contract_revision != expected.kind_contract_revision
        || p.capability_id != expected.capability_id
        || p.execution_profile_id != expected.implementation.execution_profile_id
        || p.implementation_id != expected.implementation.implementation_id
        || p.artifact_id != expected.implementation.artifact_id
        || p.inputs != expected.inputs
        || p.outputs != expected.outputs
        || p.semantic_contract != expected.semantic_contract
        || p.limits != expected.limits
        || p.base.is_some()
        || !p.host_calls.is_empty()
        || !p.resources.is_empty()
        || !p.authority.is_empty()
        || !p.pool_references.is_empty()
        || !p.realization_characteristics.is_empty()
        || !p.realization_properties.is_empty()
        || !p.terminal_transductions.is_empty()
    {
        return Err("PCM packing placement differs from its exact prepared profile".into());
    }
    let [entry] = p.configuration.as_slice() else {
        return Err("PCM packing requires an exact finite input budget".into());
    };
    match (entry.key.as_str(), &entry.value) {
        ("maximum-blocks", ConfigurationValue::U64(value)) if (1..=64).contains(value) => {
            Ok(*value as u16)
        }
        _ => Err("PCM packing input budget is outside 1..=64".into()),
    }
}
impl KernelOperationFactory for Pcm16PackingFactory {
    fn implementation_id(&self) -> &ImplementationId {
        static ID: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| conduit_std_offers::PCM16_PACKING_IMPLEMENTATION.into())
    }
    fn budget(&self, p: &PlannedGear) -> Result<KernelOperationBudget, String> {
        validate(p)?;
        let input = conduit_semantic_catalog::PCM16_PACKING_MAXIMUM_INPUT_BYTES;
        let output = conduit_audio::PCM16_PACKING_MAXIMUM_OUTPUT_BYTES as u32;
        Ok(KernelOperationBudget {
            value_items: 2,
            value_bytes: input + output,
            maximum_value_bytes: input.max(output),
            host_requests: 0,
            sign_items: 4,
        })
    }
    fn prepare(
        &self,
        p: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        Ok(Box::new(PreparedPcm16PackingBack::prepare(validate(p)?)?))
    }
}
