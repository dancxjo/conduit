//! Exact Plan-bound hosted owners for explicitly closing-Flow linear Fores.
use crate::{
    fixed_numeric_linear_flow::*, fixed_numeric_preparation::verify_fixed_placement,
    fixed_tensor_resource::AdmittedFixedTensorResource,
};
use alloc::{collections::BTreeMap, sync::Arc};
use alloc::{format, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
type OwnedBack = super::prepared_numeric_back::PreparedNumericBack;
type Resources = BTreeMap<String, Arc<AdmittedFixedTensorResource>>;
macro_rules! choose {
    ($inputs:literal, $outputs:literal, $placement:expr, $resources:expr) => {{
        let offer =
            linear_flow_offer::<$inputs, $outputs>().map_err(|error| format!("{error:?}"))?;
        let back = if let Some(placement) = $placement {
            let get = |name: &str| {
                $resources
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("missing Flow tensor {name}"))
            };
            let back = FixedLinearFlowBack::<$inputs, $outputs>::prepare_planned_owned::<PORTS>(
                placement,
                4,
                get("weights")?,
            )
            .map_err(|error| format!("{error:?}"))?;
            let local = back.local_accounted_heap_bytes();
            Some(OwnedBack::new(back, local))
        } else {
            None
        };
        Ok((offer, back))
    }};
}
fn select(
    kind: &str,
    placement: Option<&PlannedGear>,
    resources: &Resources,
) -> Result<(CapabilityOffer, Option<OwnedBack>), String> {
    match kind {
        "numeric/flow-linear161x18" => choose!(161, 18, placement, resources),
        "numeric/flow-linear18x18" => choose!(18, 18, placement, resources),
        "numeric/flow-linear3x2" => choose!(3, 2, placement, resources),
        "numeric/flow-linear80x1" => choose!(80, 1, placement, resources),
        "numeric/flow-linear328x192" => choose!(328, 192, placement, resources),
        "numeric/flow-linear192x192" => choose!(192, 192, placement, resources),
        "numeric/flow-linear192x4" => choose!(192, 4, placement, resources),
        "numeric/flow-linear160x160" => choose!(160, 160, placement, resources),
        "numeric/flow-linear128x128" => choose!(128, 128, placement, resources),
        "numeric/flow-linear688x128" => choose!(688, 128, placement, resources),
        "numeric/flow-linear128x40" => choose!(128, 40, placement, resources),
        "numeric/flow-linear272x480" => choose!(272, 480, placement, resources),
        "numeric/flow-linear160x480" => choose!(160, 480, placement, resources),
        "numeric/flow-linear240x384" => choose!(240, 384, placement, resources),
        "numeric/flow-linear128x384" => choose!(128, 384, placement, resources),
        "numeric/flow-linear208x384" => choose!(208, 384, placement, resources),
        _ => Err(format!("unsupported explicit linear Flow {kind}")),
    }
}
pub fn fixed_linear_flow_offer(kind: &str) -> Result<CapabilityOffer, String> {
    Ok(select(kind, None, &Resources::new())?.0)
}
pub struct FixedLinearFlowOperationFactory {
    identity: ImplementationId,
    offers: BTreeMap<CapabilityId, CapabilityOffer>,
    resources: BTreeMap<PlacementId, Resources>,
}
impl FixedLinearFlowOperationFactory {
    pub fn for_plan(
        plan: &Plan,
        sources: &BTreeMap<PlacementId, Arc<AdmittedFixedTensorResource>>,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("linear Flow requires one sealed fragment".into());
        }
        let fragment = &plan.fragments[0];
        let mut result = Self {
            identity: ImplementationId::from(FLOW_LINEAR_IMPLEMENTATION),
            offers: BTreeMap::new(),
            resources: BTreeMap::new(),
        };
        for gear in fragment
            .placements
            .iter()
            .filter(|gear| gear.kind_id.as_str().starts_with("numeric/flow-linear"))
        {
            let offer = fixed_linear_flow_offer(gear.kind_id.as_str())?;
            verify_fixed_placement(gear, &offer).map_err(|error| format!("{error:?}"))?;
            let mut bindings = Resources::new();
            for port in gear
                .inputs
                .iter()
                .filter(|p| p.port_id.as_str() == "weights")
            {
                let cord = fragment
                    .connections
                    .iter()
                    .find(|cord| {
                        cord.sink_placement_id == gear.placement_id
                            && cord.sink_port_id == port.port_id
                    })
                    .ok_or("missing linear Flow tensor cord")?;
                let adopted = sources
                    .get(&cord.source_placement_id)
                    .ok_or("missing adopted linear Flow tensor source")?;
                bindings.insert(port.port_id.as_str().into(), adopted.clone());
            }
            result.offers.insert(offer.capability_id.clone(), offer);
            result.resources.insert(gear.placement_id.clone(), bindings);
        }
        Ok(result)
    }
    fn selected(&self, gear: &PlannedGear) -> Result<&Resources, String> {
        let offer = self
            .offers
            .get(&gear.capability_id)
            .ok_or("unselected linear Flow capability")?;
        verify_fixed_placement(gear, offer).map_err(|error| format!("{error:?}"))?;
        self.resources
            .get(&gear.placement_id)
            .ok_or_else(|| "unselected linear Flow placement".into())
    }
}
impl KernelOperationFactory for FixedLinearFlowOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.identity
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.selected(gear)?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: gear.limits.max_queue_bytes,
            maximum_value_bytes: gear.limits.max_queue_bytes,
            host_requests: 0,
            sign_items: 1,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _values: &mut HostedValueStore,
    ) -> Result<alloc::boxed::Box<dyn StepBack<PORTS> + Send>, String> {
        self.prepare_with_inventory(gear)
            .map(|prepared| prepared.into_back())
    }
}

impl FixedLinearFlowOperationFactory {
    pub fn prepare_with_inventory(&self, gear: &PlannedGear) -> Result<OwnedBack, String> {
        select(gear.kind_id.as_str(), Some(gear), self.selected(gear)?)?
            .1
            .ok_or_else(|| "linear Flow did not prepare selected operation".into())
    }
}
