//! Public custody for the existing checked Source `when` implementation.
//! The Source program owns the predicate; the installed host retains its exact
//! original-frame forwarding and closing Flow drop semantics.
use crate::installed_std::pure_expression_back::PureExpressionHost;
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{
    ImplementationId, PlacementId, Plan, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub use conduit_std_offers::{
    pure_filter_std_offer as offer, PURE_FILTER_STD_IMPLEMENTATION as IMPLEMENTATION,
};
use std::{boxed::Box, collections::BTreeMap, format, string::String};

pub struct PreparedPureFilter {
    host: PureExpressionHost,
}
impl PreparedPureFilter {
    pub fn execute(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_plot::PortableExpressionEvaluationRefusal> {
        self.host.execute_filter(input)
    }
}

pub struct PureFilterOperationFactory {
    implementation: ImplementationId,
    placements: BTreeMap<PlacementId, PlannedGear>,
}
impl PureFilterOperationFactory {
    /// Admit every selected filter from a sealed ordinary Plan. No caller
    /// supplied predicate, schema, offer, or weakened placement is accepted.
    pub fn for_plan(plan: &Plan) -> Result<Self, String> {
        if !conduit_core::verify_plan(plan) {
            return Err("pure filter Plan is not sealed".into());
        }
        let implementation =
            ImplementationId::from(conduit_std_offers::PURE_FILTER_STD_IMPLEMENTATION);
        let mut placements = BTreeMap::new();
        for placement in plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .filter(|p| p.implementation_id == implementation)
        {
            if placement.kind_contract_revision.as_str() != conduit_plot::PURE_FILTER_REVISION {
                return Err("selected pure filter has a foreign semantic revision".into());
            }
            validate_offered_placement(placement)?;
            PureExpressionHost::from_placement(placement)?;
            if placements
                .insert(placement.placement_id.clone(), placement.clone())
                .is_some()
            {
                return Err("pure filter placement is ambiguous".into());
            }
        }
        if placements.is_empty() {
            return Err("Plan contains no selected pure filter".into());
        }
        Ok(Self {
            implementation,
            placements,
        })
    }
    fn admitted(&self, placement: &PlannedGear) -> Result<(), String> {
        if self.placements.get(&placement.placement_id) != Some(placement) {
            return Err("pure filter placement differs from the admitted sealed Plan".into());
        }
        Ok(())
    }
    pub fn prepare_host(&self, placement: &PlannedGear) -> Result<PreparedPureFilter, String> {
        self.admitted(placement)?;
        Ok(PreparedPureFilter {
            host: PureExpressionHost::from_placement(placement)?,
        })
    }
}
impl KernelOperationFactory for PureFilterOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.admitted(placement)?;
        Ok(KernelOperationBudget {
            value_items: 8,
            value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
            maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            host_requests: 1,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.admitted(placement)?;
        Ok(Box::new(
            conduit_semantic_catalog::StructuredSelectorBack::new(
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            ),
        ))
    }
}

fn validate_offered_placement(gear: &PlannedGear) -> Result<(), String> {
    let program = crate::installed_std::pure_expression_back::program_from_placement(gear)?;
    let temporal = gear
        .inputs
        .first()
        .ok_or("pure filter input is absent")?
        .temporal;
    let expected =
        offer(&program, temporal).map_err(|error| format!("pure filter offer: {error:?}"))?;
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.artifact_id != expected.implementation.artifact_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.limits != expected.limits
        || gear.semantic_contract != expected.semantic_contract
        || gear.host_calls != expected.host_calls
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || gear.base.is_some()
        || !gear.realization_characteristics.is_empty()
        || !gear.realization_properties.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.terminal_transductions.is_empty()
    {
        return Err("pure filter placement differs from its exact installed offer".into());
    }
    Ok(())
}
