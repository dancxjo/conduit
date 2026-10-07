//! Exact Plan-bound hosted owners for explicitly closing-Flow embedding Fores.
use crate::{
    fixed_numeric_embedding_flow::*, fixed_numeric_preparation::verify_fixed_placement,
    fixed_tensor_resource::AdmittedFixedTensorResource,
};
use alloc::{boxed::Box, format, string::String};
use alloc::{collections::BTreeMap, sync::Arc};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
type OwnedBack = Box<dyn StepBack<PORTS> + Send>;
type Resources = BTreeMap<String, Arc<AdmittedFixedTensorResource>>;
macro_rules! choose {
    ($inputs:literal, $outputs:literal, $placement:expr, $resources:expr) => {{
        let offer =
            embedding_flow_offer::<$inputs, $outputs>().map_err(|error| format!("{error:?}"))?;
        let back = if let Some(placement) = $placement {
            let get = |name: &str| {
                $resources
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("missing Flow tensor {name}"))
            };
            Some(Box::new(
                FixedEmbeddingFlowBack::<$inputs, $outputs>::prepare_planned_owned::<PORTS>(
                    placement,
                    4,
                    get("weights")?,
                )
                .map_err(|error| format!("{error:?}"))?,
            ) as OwnedBack)
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
        "numeric/flow-embedding224x12" => choose!(224, 12, placement, resources),
        _ => Err(format!("unsupported explicit embedding Flow {kind}")),
    }
}
pub fn fixed_embedding_flow_offer(kind: &str) -> Result<CapabilityOffer, String> {
    Ok(select(kind, None, &Resources::new())?.0)
}
pub struct FixedEmbeddingFlowOperationFactory {
    identity: ImplementationId,
    offers: BTreeMap<CapabilityId, CapabilityOffer>,
    resources: BTreeMap<PlacementId, Resources>,
}
impl FixedEmbeddingFlowOperationFactory {
    pub fn for_plan(
        plan: &Plan,
        sources: &BTreeMap<PlacementId, Arc<AdmittedFixedTensorResource>>,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("embedding Flow requires one sealed fragment".into());
        }
        let fragment = &plan.fragments[0];
        let mut result = Self {
            identity: ImplementationId::from(FLOW_EMBEDDING_IMPLEMENTATION),
            offers: BTreeMap::new(),
            resources: BTreeMap::new(),
        };
        for gear in fragment
            .placements
            .iter()
            .filter(|gear| gear.kind_id.as_str().starts_with("numeric/flow-embedding"))
        {
            let offer = fixed_embedding_flow_offer(gear.kind_id.as_str())?;
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
                    .ok_or("missing embedding Flow tensor cord")?;
                let adopted = sources
                    .get(&cord.source_placement_id)
                    .ok_or("missing adopted embedding Flow tensor source")?;
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
            .ok_or("unselected embedding Flow capability")?;
        verify_fixed_placement(gear, offer).map_err(|error| format!("{error:?}"))?;
        self.resources
            .get(&gear.placement_id)
            .ok_or_else(|| "unselected embedding Flow placement".into())
    }
}
impl KernelOperationFactory for FixedEmbeddingFlowOperationFactory {
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
    ) -> Result<OwnedBack, String> {
        select(gear.kind_id.as_str(), Some(gear), self.selected(gear)?)?
            .1
            .ok_or_else(|| "embedding Flow did not prepare selected operation".into())
    }
}
