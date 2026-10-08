//! Hosted owners for explicitly closing-Flow typed pairing.
use crate::{fixed_numeric_pair_flow::*, fixed_numeric_preparation::verify_fixed_placement};
use alloc::collections::BTreeMap;
use alloc::{boxed::Box, format, string::String};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::bounded_owner_table::{BoundedOwnerTable, OwnerTableRefusal};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;

pub struct FixedFlowPairOperationFactory {
    identity: ImplementationId,
    selected: BoundedOwnerTable<PlacementId, CapabilityOffer>,
}
impl FixedFlowPairOperationFactory {
    pub fn for_plan(plan: &Plan) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("Flow pairing requires one sealed fragment".into());
        }
        let mut selected = BTreeMap::new();
        for gear in plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.kind_id.as_str().starts_with("numeric/flow-pair"))
        {
            let offer = fixed_flow_pair_offer(gear.kind_id.as_str())?;
            verify_fixed_placement(gear, &offer).map_err(|error| format!("{error:?}"))?;
            selected.insert(gear.placement_id.clone(), offer);
        }
        Ok(Self {
            identity: ImplementationId::from(FLOW_PAIR_IMPLEMENTATION),
            selected: super::retained_table::retain(selected)?,
        })
    }
    /// Reuses independently prepared exact Source profiles. The complete Plan
    /// verification and transient selection construction remain upstream prep.
    pub fn for_plan_with_prepared_profiles(
        plan: &Plan,
        profiles: &[&PreparedFixedFlowPairProfile],
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("Flow pairing requires one sealed fragment".into());
        }
        let mut selected = BTreeMap::new();
        for gear in plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.kind_id.as_str().starts_with("numeric/flow-pair"))
        {
            let mut matches = profiles
                .iter()
                .filter(|profile| profile.offer().kind_id == gear.kind_id);
            let profile = matches.next().ok_or("missing prepared Flow pair profile")?;
            if matches.next().is_some() {
                return Err("ambiguous prepared Flow pair profile".into());
            }
            let offer = profile.offer();
            verify_fixed_placement(gear, offer).map_err(|error| format!("{error:?}"))?;
            selected.insert(gear.placement_id.clone(), offer.clone());
        }
        Ok(Self {
            identity: ImplementationId::from(FLOW_PAIR_IMPLEMENTATION),
            selected: super::retained_table::retain(selected)?,
        })
    }
    fn verify(&self, gear: &PlannedGear) -> Result<(), String> {
        let offer = self
            .selected
            .get(&gear.placement_id)
            .ok_or("unselected Flow pair placement")?;
        verify_fixed_placement(gear, offer).map_err(|error| format!("{error:?}"))
    }
}
impl KernelOperationFactory for FixedFlowPairOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.identity
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.verify(gear)?;
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
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        self.prepare_with_inventory(gear)
            .map(|prepared| prepared.into_back())
    }
}

impl FixedFlowPairOperationFactory {
    pub fn prepare_with_inventory(
        &self,
        gear: &PlannedGear,
    ) -> Result<super::prepared_numeric_back::PreparedNumericBack, String> {
        self.verify(gear)?;
        let back = FixedFlowPairBack::prepare_planned::<PORTS>(gear, 3)
            .map_err(|error| format!("{error:?}"))?;
        let local = back.local_accounted_heap_bytes();
        Ok(super::prepared_numeric_back::PreparedNumericBack::new(
            back, local,
        ))
    }
}

impl FixedFlowPairOperationFactory {
    // This is per-Back preparation only: the caller separately admits the
    // immutable profile and this factory's Plan/selection preparation.
    fn profile_for_storage<'a>(
        &self,
        gear: &PlannedGear,
        profile: &'a PreparedFixedFlowPairProfile,
    ) -> Result<&'a PreparedFixedFlowPairProfile, FlowPairStorageRefusal> {
        let offer = self
            .selected
            .get(&gear.placement_id)
            .ok_or(FlowPairStorageRefusal::ProfileNotPrepared)?;
        if offer != profile.offer() {
            return Err(FlowPairStorageRefusal::ProfileNotPrepared);
        }
        verify_fixed_placement(gear, offer)
            .map_err(|_| FlowPairStorageRefusal::ProfileNotPrepared)?;
        Ok(profile)
    }
    pub fn preparation_storage_reservation(
        &self,
        gear: &PlannedGear,
        profile: &PreparedFixedFlowPairProfile,
    ) -> Result<FlowPairStorageReceipt, FlowPairStorageRefusal> {
        let mut r =
            FixedFlowPairBack::storage_reservation(self.profile_for_storage(gear, profile)?)?;
        let root = core::mem::size_of::<FixedFlowPairBack>();
        r.preparation_requested_bytes_bound = r
            .preparation_requested_bytes_bound
            .checked_add(root)
            .ok_or(FlowPairStorageRefusal::Capacity)?;
        r.preparation_peak_bytes_bound = r
            .preparation_peak_bytes_bound
            .checked_add(root)
            .ok_or(FlowPairStorageRefusal::Capacity)?;
        r.retained_heap_bytes_bound = r
            .retained_heap_bytes_bound
            .checked_add(root)
            .ok_or(FlowPairStorageRefusal::Capacity)?;
        Ok(r)
    }
    pub fn prepare_with_storage_limits(
        &self,
        gear: &PlannedGear,
        profile: &PreparedFixedFlowPairProfile,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<
        (
            super::prepared_numeric_back::PreparedNumericBack,
            FlowPairStorageReceipt,
        ),
        FlowPairStorageRefusal,
    > {
        let profile = self.profile_for_storage(gear, profile)?;
        let root = core::mem::size_of::<FixedFlowPairBack>();
        let requested = maximum_preparation_requested_bytes
            .checked_sub(root)
            .ok_or(FlowPairStorageRefusal::Capacity)?;
        let retained = maximum_retained_heap_bytes
            .checked_sub(root)
            .ok_or(FlowPairStorageRefusal::Capacity)?;
        let (back, mut r) =
            FixedFlowPairBack::prepare_selected_with_storage_limits(profile, requested, retained)?;
        r.preparation_requested_bytes_bound += root;
        r.preparation_peak_bytes_bound += root;
        r.retained_heap_bytes_bound += root;
        let local = back.local_accounted_heap_bytes();
        r.retained_accounted_heap_bytes = local + root;
        Ok((
            super::prepared_numeric_back::PreparedNumericBack::new(back, local),
            r,
        ))
    }
}

impl FixedFlowPairOperationFactory {
    /// Local retained array/key/offer/profile payload capacities. Shared Arc
    /// profile payloads, inline factory root and allocator bookkeeping separate.
    pub fn local_owned_payload_bytes(&self) -> Option<usize> {
        let mut total = self
            .identity
            .owned_heap_bytes()
            .checked_add(self.selected.array_capacity_bytes().ok()?)?;
        for (key, value) in self.selected.iter() {
            total = total
                .checked_add(key.owned_heap_bytes())?
                .checked_add(capability_offer_owned_heap_bytes(value).ok()?)?;
        }
        Some(total)
    }
    pub fn retained_selection_array_bytes(&self) -> Result<usize, OwnerTableRefusal> {
        self.selected.array_capacity_bytes()
    }
}
