//! Exact Plan-bound hosted owners for explicit signed-Q7 tiled matrix Fores.
use crate::{
    fixed_numeric_compact_back::FixedCompactBack, fixed_numeric_compact_catalog::*,
    fixed_numeric_preparation::verify_fixed_placement,
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
    ($inputs:literal, $outputs:literal, $placement:expr, $resources:expr, $biased:expr, $flow:expr) => {{
        let offer = compact_offer($inputs, $outputs, $biased, $flow)
            .map_err(|error| format!("{error:?}"))?;
        let back = if let Some(placement) = $placement {
            let get = |name: &str| {
                $resources
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("missing Flow tensor {name}"))
            };
            let back = FixedCompactBack::<$inputs, $outputs>::prepare_planned_owned::<PORTS>(
                placement,
                5,
                $flow,
                get("weights")?,
                get("scales")?,
                if $biased { Some(get("bias")?) } else { None },
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
    let (suffix, flow) = if let Some(suffix) = kind.strip_prefix("numeric/flow-signed-q7-tiled-") {
        (suffix, true)
    } else if let Some(suffix) = kind.strip_prefix("numeric/signed-q7-tiled-") {
        (suffix, false)
    } else {
        return Err(format!("unsupported compact Fore {kind}"));
    };
    let (shape, biased) = if let Some(shape) = suffix.strip_prefix("affine") {
        (shape, true)
    } else if let Some(shape) = suffix.strip_prefix("linear") {
        (shape, false)
    } else {
        return Err(format!("unsupported compact operation {kind}"));
    };
    match shape {
        "4x40" => choose!(4, 40, placement, resources, biased, flow),
        "192x128" => choose!(192, 128, placement, resources, biased, flow),
        "128x320" => choose!(128, 320, placement, resources, biased, flow),
        "328x192" => choose!(328, 192, placement, resources, biased, flow),
        "192x192" => choose!(192, 192, placement, resources, biased, flow),
        "272x480" => choose!(272, 480, placement, resources, biased, flow),
        "160x480" => choose!(160, 480, placement, resources, biased, flow),
        "240x384" => choose!(240, 384, placement, resources, biased, flow),
        "128x384" => choose!(128, 384, placement, resources, biased, flow),
        "208x384" => choose!(208, 384, placement, resources, biased, flow),
        "160x160" => choose!(160, 160, placement, resources, biased, flow),
        "128x128" => choose!(128, 128, placement, resources, biased, flow),
        "688x128" => choose!(688, 128, placement, resources, biased, flow),
        "128x40" => choose!(128, 40, placement, resources, biased, flow),
        _ => Err(format!("unsupported compact shape {kind}")),
    }
}
pub fn fixed_compact_offer(kind: &str) -> Result<CapabilityOffer, String> {
    Ok(select(kind, None, &Resources::new())?.0)
}
pub struct FixedCompactOperationFactory {
    identity: ImplementationId,
    offers: BTreeMap<CapabilityId, CapabilityOffer>,
    resources: BTreeMap<PlacementId, Resources>,
}
impl FixedCompactOperationFactory {
    pub fn for_plan(
        plan: &Plan,
        sources: &BTreeMap<PlacementId, Arc<AdmittedFixedTensorResource>>,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("compact requires one sealed fragment".into());
        }
        let fragment = &plan.fragments[0];
        let mut result = Self {
            identity: ImplementationId::from(COMPACT_IMPLEMENTATION),
            offers: BTreeMap::new(),
            resources: BTreeMap::new(),
        };
        for gear in fragment
            .placements
            .iter()
            .filter(|gear| gear.implementation_id.as_str() == COMPACT_IMPLEMENTATION)
        {
            let offer = fixed_compact_offer(gear.kind_id.as_str())?;
            verify_fixed_placement(gear, &offer).map_err(|error| format!("{error:?}"))?;
            let mut bindings = Resources::new();
            for port in gear
                .inputs
                .iter()
                .filter(|p| matches!(p.port_id.as_str(), "weights" | "scales" | "bias"))
            {
                let cord = fragment
                    .connections
                    .iter()
                    .find(|cord| {
                        cord.sink_placement_id == gear.placement_id
                            && cord.sink_port_id == port.port_id
                    })
                    .ok_or("missing compact tensor cord")?;
                let adopted = sources
                    .get(&cord.source_placement_id)
                    .ok_or("missing adopted compact tensor source")?;
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
            .ok_or("unselected compact capability")?;
        verify_fixed_placement(gear, offer).map_err(|error| format!("{error:?}"))?;
        self.resources
            .get(&gear.placement_id)
            .ok_or_else(|| "unselected compact placement".into())
    }
}
impl KernelOperationFactory for FixedCompactOperationFactory {
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

impl FixedCompactOperationFactory {
    pub fn prepare_with_inventory(&self, gear: &PlannedGear) -> Result<OwnedBack, String> {
        select(gear.kind_id.as_str(), Some(gear), self.selected(gear)?)?
            .1
            .ok_or_else(|| "compact did not prepare selected operation".into())
    }
}
