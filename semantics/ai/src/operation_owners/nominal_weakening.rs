//! Exact selected Source profile admission owner; resource/model custody is unrelated.
use crate::{
    fixed_numeric_preparation::verify_fixed_placement,
    nominal_weakening::{NominalWeakeningBack, PreparedNominalWeakening, WEAKENING_IMPLEMENTATION},
};
use alloc::{boxed::Box, format, string::String};
use alloc::{collections::BTreeMap, sync::Arc};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::bounded_owner_table::{BoundedOwnerTable, OwnerTableRefusal};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
struct SelectedProfile {
    profile: Arc<PreparedNominalWeakening>,
    flow: bool,
    offer: CapabilityOffer,
}
pub struct NominalWeakeningOperationFactory {
    implementation: ImplementationId,
    selected: BoundedOwnerTable<PlacementId, SelectedProfile>,
}
impl NominalWeakeningOperationFactory {
    pub fn for_plan(
        plan: &Plan,
        profiles: &[Arc<PreparedNominalWeakening>],
    ) -> Result<Self, String> {
        if !verify_plan(plan) {
            return Err("invalid sealed Plan".into());
        }
        let mut selected = BTreeMap::new();
        for gear in plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|gear| gear.implementation_id.as_str() == WEAKENING_IMPLEMENTATION)
        {
            let mut matches = profiles
                .iter()
                .flat_map(|profile| [false, true].map(move |flow| (profile, flow)))
                .filter(|(profile, flow)| profile.kind_identity(*flow) == gear.kind_id.as_str());
            let (profile, flow) = matches
                .next()
                .ok_or("selected nominal weakening has no exact structural profile")?;
            if matches.next().is_some() {
                return Err("ambiguous selected nominal weakening".into());
            }
            let offer = profile.offer(flow)?;
            verify_fixed_placement(gear, &offer).map_err(|e| format!("{e:?}"))?;
            if selected
                .insert(
                    gear.placement_id.clone(),
                    SelectedProfile {
                        profile: profile.clone(),
                        flow,
                        offer,
                    },
                )
                .is_some()
            {
                return Err("duplicate selected profile placement".into());
            }
        }
        Ok(Self {
            implementation: WEAKENING_IMPLEMENTATION.into(),
            selected: super::retained_table::retain(selected)?,
        })
    }
    fn selected(&self, gear: &PlannedGear) -> Result<&SelectedProfile, String> {
        let selected = self
            .selected
            .get(&gear.placement_id)
            .ok_or("no exact selected nominal weakening")?;
        verify_fixed_placement(gear, &selected.offer).map_err(|e| format!("{e:?}"))?;
        Ok(selected)
    }
}
impl KernelOperationFactory for NominalWeakeningOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let selected = self.selected(gear)?;
        let maximum = crate::transport_envelope::maximum_prepared_transport_value_bytes(
            selected.profile.output_type(),
        )
        .map_err(|e| format!("{e:?}"))?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: maximum,
            maximum_value_bytes: maximum.max(
                crate::transport_envelope::maximum_prepared_transport_value_bytes(
                    selected.profile.input_type(),
                )
                .map_err(|e| format!("{e:?}"))?,
            ),
            host_requests: 0,
            sign_items: 16,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.prepare_with_inventory(gear)
            .map(|prepared| prepared.into_back())
    }
}

impl NominalWeakeningOperationFactory {
    pub fn prepare_with_inventory(
        &self,
        gear: &PlannedGear,
    ) -> Result<super::prepared_numeric_back::PreparedNumericBack, String> {
        let selected = self.selected(gear)?;
        let back = NominalWeakeningBack::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(
            gear,
            2,
            &selected.profile,
            selected.flow,
        )?;
        let local = back.local_accounted_heap_bytes();
        Ok(super::prepared_numeric_back::PreparedNumericBack::new(
            back, local,
        ))
    }
}

impl NominalWeakeningOperationFactory {
    fn selected_for_storage(
        &self,
        gear: &PlannedGear,
    ) -> Result<&SelectedProfile, crate::nominal_weakening::WeakeningBackPreparationRefusal> {
        use crate::nominal_weakening::WeakeningBackPreparationRefusal as Error;
        let selected = self
            .selected
            .get(&gear.placement_id)
            .ok_or(Error::Validation)?;
        verify_fixed_placement(gear, &selected.offer).map_err(|_| Error::Validation)?;
        Ok(selected)
    }
    pub fn preparation_storage_reservation(
        &self,
        gear: &PlannedGear,
    ) -> Result<
        crate::nominal_weakening::WeakeningBackStorageReceipt,
        crate::nominal_weakening::WeakeningBackPreparationRefusal,
    > {
        use crate::nominal_weakening::WeakeningBackPreparationRefusal as Error;
        let mut r =
            NominalWeakeningBack::storage_reservation(&self.selected_for_storage(gear)?.profile)?;
        let root = core::mem::size_of::<NominalWeakeningBack>();
        r.preparation_requested_bytes_bound = r
            .preparation_requested_bytes_bound
            .checked_add(root)
            .ok_or(Error::Capacity)?;
        r.retained_heap_bytes_bound = r
            .retained_heap_bytes_bound
            .checked_add(root)
            .ok_or(Error::Capacity)?;
        Ok(r)
    }
    pub fn prepare_with_storage_limits(
        &self,
        gear: &PlannedGear,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<
        (
            super::prepared_numeric_back::PreparedNumericBack,
            crate::nominal_weakening::WeakeningBackStorageReceipt,
        ),
        crate::nominal_weakening::WeakeningBackPreparationRefusal,
    > {
        use crate::nominal_weakening::WeakeningBackPreparationRefusal as Error;
        let selected = self.selected_for_storage(gear)?;
        let root = core::mem::size_of::<NominalWeakeningBack>();
        let requested = maximum_preparation_requested_bytes
            .checked_sub(root)
            .ok_or(Error::Capacity)?;
        let retained = maximum_retained_heap_bytes
            .checked_sub(root)
            .ok_or(Error::Capacity)?;
        let (back, mut r) = NominalWeakeningBack::prepare_selected_with_storage_limits(
            &selected.profile,
            selected.flow,
            requested,
            retained,
        )?;
        r.preparation_requested_bytes_bound = r
            .preparation_requested_bytes_bound
            .checked_add(root)
            .ok_or(Error::Capacity)?;
        r.retained_heap_bytes_bound = r
            .retained_heap_bytes_bound
            .checked_add(root)
            .ok_or(Error::Capacity)?;
        r.retained_accounted_heap_bytes = r
            .retained_accounted_heap_bytes
            .checked_add(root)
            .ok_or(Error::Capacity)?;
        let local = back.local_accounted_heap_bytes();
        Ok((
            super::prepared_numeric_back::PreparedNumericBack::new(back, local),
            r,
        ))
    }
}

impl NominalWeakeningOperationFactory {
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
                .checked_add(capability_offer_owned_heap_bytes(&value.offer).ok()?)?;
        }
        Some(total)
    }
    pub fn retained_selection_array_bytes(&self) -> Result<usize, OwnerTableRefusal> {
        self.selected.array_capacity_bytes()
    }
}

impl NominalWeakeningOperationFactory {
    /// Visits exact shared owners without allocating; repeated identities remain
    /// visible so the composing inventory can deduplicate by Arc pointer.
    pub fn visit_shared_profiles(&self, visitor: &mut impl FnMut(&Arc<PreparedNominalWeakening>)) {
        for (_, selected) in self.selected.iter() {
            visitor(&selected.profile);
        }
    }
}
