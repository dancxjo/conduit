//! Hosted preparation of generic fixed numeric operations from exact Plans.
//! Authored source owns all layer ordering, indexing policy, and recurrence.
use conduit_ai::fixed_numeric_preparation::verify_fixed_placement;
use conduit_ai::fixed_tensor_resource::AdmittedFixedTensorResource;
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::{collections::BTreeMap, sync::Arc};
type NumericBack = Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>;
type TensorBindings = BTreeMap<String, Arc<AdmittedFixedTensorResource>>;
struct Selection {
    offer: CapabilityOffer,
    back: Option<NumericBack>,
}
macro_rules! run {
    ($selected:expr,$gear:ident,$fuel:ident,$offer:expr,$back:expr) => {{
        (|| -> Result<Selection, String> {
            let offer = $offer.map_err(|error| format!("{error:?}"))?;
            let back = if let Some(($gear, $fuel)) = $selected {
                Some(Box::new($back.map_err(|error| format!("{error:?}"))?) as NumericBack)
            } else {
                None
            };
            Ok(Selection { offer, back })
        })()
    }};
}
mod dispatch_tensors;
mod dispatch_values;
mod dispatch_vectors;
fn resource(
    bindings: &TensorBindings,
    name: &str,
) -> Result<Arc<AdmittedFixedTensorResource>, String> {
    bindings
        .get(name)
        .cloned()
        .ok_or_else(|| format!("missing admitted numeric tensor port {name}"))
}
fn select(
    kind: &str,
    planned: Option<(&PlannedGear, u16)>,
    bindings: &TensorBindings,
) -> Result<Selection, String> {
    dispatch_tensors::select(kind, planned, bindings)
        .or_else(|| dispatch_vectors::select(kind, planned))
        .or_else(|| dispatch_values::select(kind, planned))
        .ok_or_else(|| format!("unsupported generic numeric operation {kind}"))?
}
pub fn fixed_numeric_offer(kind: &str) -> Result<CapabilityOffer, String> {
    Ok(select(kind, None, &BTreeMap::new())?.offer)
}
/// One owner per exact implementation identity, suitable for the ordinary
/// KernelOperationRegistry. Immutable resources retain their adoption receipts.
pub struct FixedNumericOperationFactory {
    implementation: ImplementationId,
    offers: BTreeMap<CapabilityId, CapabilityOffer>,
    bindings: BTreeMap<PlacementId, TensorBindings>,
}
impl FixedNumericOperationFactory {
    /// Bind tensor inputs by the selected Plan cords and their admitted source
    /// placement identities. No model layer names or execution order are used.
    pub fn for_plan(
        plan: &Plan,
        sources: &BTreeMap<PlacementId, Arc<AdmittedFixedTensorResource>>,
    ) -> Result<Vec<Self>, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("numeric owners require one sealed fragment".into());
        }
        let fragment = &plan.fragments[0];
        let mut owners: BTreeMap<ImplementationId, Self> = BTreeMap::new();
        for gear in &fragment.placements {
            if !gear.kind_id.as_str().starts_with("numeric/") {
                continue;
            }
            let offer = fixed_numeric_offer(gear.kind_id.as_str())?;
            verify_fixed_placement(gear, &offer).map_err(|error| format!("{error:?}"))?;
            let owner = owners
                .entry(gear.implementation_id.clone())
                .or_insert_with(|| Self {
                    implementation: gear.implementation_id.clone(),
                    offers: BTreeMap::new(),
                    bindings: BTreeMap::new(),
                });
            owner.offers.insert(offer.capability_id.clone(), offer);
            let mut bindings = TensorBindings::new();
            for port in gear
                .inputs
                .iter()
                .filter(|port| matches!(port.port_id.as_str(), "weights" | "bias"))
            {
                let cord = fragment
                    .connections
                    .iter()
                    .find(|cord| {
                        cord.sink_placement_id == gear.placement_id
                            && cord.sink_port_id == port.port_id
                    })
                    .ok_or("numeric tensor port has no selected cord")?;
                let adopted = sources
                    .get(&cord.source_placement_id)
                    .ok_or("numeric tensor cord has no admitted source resource")?;
                bindings.insert(port.port_id.as_str().into(), adopted.clone());
            }
            owner.bindings.insert(gear.placement_id.clone(), bindings);
        }
        Ok(owners.into_values().collect())
    }
    fn selected(&self, gear: &PlannedGear) -> Result<&TensorBindings, String> {
        let offer = self
            .offers
            .get(&gear.capability_id)
            .ok_or("numeric owner has no retained offer")?;
        verify_fixed_placement(gear, offer).map_err(|error| format!("{error:?}"))?;
        self.bindings
            .get(&gear.placement_id)
            .ok_or_else(|| "numeric owner has no exact planned tensor bindings".into())
    }
}
impl KernelOperationFactory for FixedNumericOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.selected(gear)?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: 16384,
            maximum_value_bytes: 16384,
            host_requests: 0,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _values: &mut HostedValueStore,
    ) -> Result<NumericBack, String> {
        let bindings = self.selected(gear)?;
        let fuel = u16::try_from(gear.inputs.len() + 1)
            .map_err(|_| "numeric step fuel exceeds finite port budget")?;
        select(gear.kind_id.as_str(), Some((gear, fuel)), bindings)?
            .back
            .ok_or_else(|| "numeric owner did not prepare its selected operation".into())
    }
}
