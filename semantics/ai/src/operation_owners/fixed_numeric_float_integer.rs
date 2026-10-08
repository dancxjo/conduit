//! Exact selected nearest-ties-away numeric conversion owner, without audio policy.
use crate::{
    fixed_numeric_catalog::fixed_numeric_type, fixed_numeric_float_integer::*,
    fixed_numeric_preparation::verify_fixed_placement,
};
use alloc::collections::BTreeMap;
use alloc::{boxed::Box, format, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::bounded_owner_table::{BoundedOwnerTable, OwnerTableRefusal};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub struct FloatIntegerOperationFactory {
    implementation: ImplementationId,
    selected: BoundedOwnerTable<PlacementId, (bool, CapabilityOffer)>,
}
impl FloatIntegerOperationFactory {
    pub fn for_plan(plan: &Plan) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("one sealed numeric fragment required".into());
        }
        let mut selected = BTreeMap::new();
        for gear in &plan.fragments[0].placements {
            if gear.implementation_id.as_str() != FLOAT_INTEGER_IMPLEMENTATION {
                continue;
            }
            let flow = match gear.kind_id.as_str() {
                "numeric/f32-to-i16-nearest-away160" => false,
                "numeric/flow-f32-to-i16-nearest-away160" => true,
                _ => return Err("unsupported selected float integer Kind".into()),
            };
            let offer = float_integer_offer(flow)?;
            verify_fixed_placement(gear, &offer).map_err(|e| format!("{e:?}"))?;
            if selected
                .insert(gear.placement_id.clone(), (flow, offer))
                .is_some()
            {
                return Err("duplicate numeric placement".into());
            }
        }
        Ok(Self {
            implementation: FLOAT_INTEGER_IMPLEMENTATION.into(),
            selected: super::retained_table::retain(selected)?,
        })
    }
    fn selected(&self, gear: &PlannedGear) -> Result<&(bool, CapabilityOffer), String> {
        let selected = self
            .selected
            .get(&gear.placement_id)
            .ok_or("unselected numeric placement")?;
        verify_fixed_placement(gear, &selected.1).map_err(|e| format!("{e:?}"))?;
        Ok(selected)
    }
}
impl KernelOperationFactory for FloatIntegerOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.selected(gear)?;
        let maximum = crate::transport_envelope::maximum_prepared_transport_value_bytes(
            &fixed_numeric_type("NumericI16Vector160")?,
        )
        .map_err(|e| format!("{e:?}"))?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: maximum,
            maximum_value_bytes: maximum,
            host_requests: 0,
            sign_items: 16,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _store: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.prepare_with_inventory(gear)
            .map(|prepared| prepared.into_back())
    }
}

impl FloatIntegerOperationFactory {
    pub fn prepare_with_inventory(
        &self,
        gear: &PlannedGear,
    ) -> Result<super::prepared_numeric_back::PreparedNumericBack, String> {
        let (flow, _) = self.selected(gear)?;
        let back = FixedFloatIntegerBack::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
            gear, 2, *flow,
        )?;
        let local = back.local_accounted_heap_bytes();
        Ok(super::prepared_numeric_back::PreparedNumericBack::new(
            back, local,
        ))
    }
}

impl FloatIntegerOperationFactory {
    /// Local retained array/key/offer/profile payload capacities. Shared Arc
    /// profile payloads, inline factory root and allocator bookkeeping separate.
    pub fn local_owned_payload_bytes(&self) -> Option<usize> {
        let mut total = self
            .implementation
            .owned_heap_bytes()
            .checked_add(self.selected.array_capacity_bytes().ok()?)?;
        for (key, value) in self.selected.iter() {
            total = total
                .checked_add(key.owned_heap_bytes())?
                .checked_add(capability_offer_owned_heap_bytes(&value.1).ok()?)?;
        }
        Some(total)
    }
    pub fn retained_selection_array_bytes(&self) -> Result<usize, OwnerTableRefusal> {
        self.selected.array_capacity_bytes()
    }
}
