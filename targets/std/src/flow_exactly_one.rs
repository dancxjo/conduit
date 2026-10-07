//! Exact sealed Plan registry for singleton closing Flow to Value.
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::{collections::BTreeMap, sync::Arc};

pub struct PreparedFlowExactlyOne {
    value: CheckedValueContract,
    schema: StructuredInfoType,
    offer: CapabilityOffer,
}
impl PreparedFlowExactlyOne {
    pub fn new(value: CheckedValueContract, schema: StructuredInfoType) -> Result<Self, String> {
        let offer =
            conduit_std_offers::flow_exactly_one_offer(&value, &schema).map_err(str::to_owned)?;
        Ok(Self {
            value,
            schema,
            offer,
        })
    }
    pub fn offer(&self) -> &CapabilityOffer {
        &self.offer
    }
    pub fn schema(&self) -> &StructuredInfoType {
        &self.schema
    }
}
struct Selected {
    placement: PlannedGear,
    profile: Arc<PreparedFlowExactlyOne>,
}
pub struct FlowExactlyOneOperationFactory {
    implementation: ImplementationId,
    selected: BTreeMap<PlacementId, Selected>,
}
fn validate(gear: &PlannedGear, offer: &CapabilityOffer) -> Result<(), String> {
    if gear.kind_id != offer.kind_id
        || gear.kind_contract_revision != offer.kind_contract_revision
        || gear.execution_profile_id != offer.implementation.execution_profile_id
        || gear.capability_id != offer.capability_id
        || gear.implementation_id != offer.implementation.implementation_id
        || gear.artifact_id != offer.implementation.artifact_id
        || gear.inputs != offer.inputs
        || gear.outputs != offer.outputs
        || gear.limits != offer.limits
        || gear.semantic_contract != offer.semantic_contract
        || !gear.host_calls.is_empty()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || gear.base.is_some()
        || !gear.realization_characteristics.is_empty()
        || !gear.realization_properties.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.terminal_transductions.is_empty()
    {
        return Err("flow exactly-one placement differs from its exact offered profile".into());
    }
    if !gear.configuration.is_empty() {
        return Err("flow exactly-one does not accept configuration".into());
    }
    Ok(())
}
impl FlowExactlyOneOperationFactory {
    pub fn for_plan(plan: &Plan, profiles: &[Arc<PreparedFlowExactlyOne>]) -> Result<Self, String> {
        if !verify_plan(plan) {
            return Err("invalid sealed Plan".into());
        }
        let mut selected = BTreeMap::new();
        for gear in plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|gear| {
                gear.implementation_id.as_str()
                    == conduit_std_offers::FLOW_EXACTLY_ONE_IMPLEMENTATION
            })
        {
            let mut matches = profiles
                .iter()
                .filter_map(|profile| validate(gear, profile.offer()).ok().map(|()| profile));
            let profile = matches
                .next()
                .ok_or("no exact checked flow exactly-one profile")?;
            if matches.next().is_some() {
                return Err("ambiguous flow exactly-one profile".into());
            }
            if selected
                .insert(
                    gear.placement_id.clone(),
                    Selected {
                        placement: gear.clone(),
                        profile: profile.clone(),
                    },
                )
                .is_some()
            {
                return Err("duplicate flow exactly-one placement".into());
            }
        }
        Ok(Self {
            implementation: conduit_std_offers::FLOW_EXACTLY_ONE_IMPLEMENTATION.into(),
            selected,
        })
    }
    fn selected(&self, gear: &PlannedGear) -> Result<&Selected, String> {
        let selected = self
            .selected
            .get(&gear.placement_id)
            .ok_or("no selected flow exactly-one placement")?;
        if gear != &selected.placement {
            return Err("flow exactly-one placement changed after admission".into());
        }
        Ok(selected)
    }
}
impl KernelOperationFactory for FlowExactlyOneOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let selected = self.selected(gear)?;
        Ok(KernelOperationBudget {
            value_items: 2,
            value_bytes: gear.limits.max_queue_bytes,
            maximum_value_bytes: selected.profile.value.maximum_bytes,
            host_requests: 0,
            sign_items: 16,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        let selected = self.selected(gear)?;
        Ok(Box::new(
            conduit_data::FlowExactlyOneBack::prepare(
                &selected.profile.value,
                &selected.profile.schema,
            )
            .map_err(|error| format!("flow exactly-one preparation: {error:?}"))?,
        ))
    }
}
