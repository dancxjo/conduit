//! Exact sealed-Plan owner for generic atomic structured closing-Flow pair.
use crate::{closing_structured_pair::*, fixed_numeric_preparation::verify_fixed_placement};
use alloc::{boxed::Box, collections::BTreeMap, format, string::String, vec::Vec};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
pub struct ClosingStructuredPairOperationFactory {
    identity: ImplementationId,
    selected: BTreeMap<PlacementId, ClosingStructuredPairProfile>,
}
impl ClosingStructuredPairOperationFactory {
    pub fn for_plan(
        plan: &Plan,
        profiles: Vec<ClosingStructuredPairProfile>,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("atomic pair requires one sealed fragment".into());
        }
        let mut by_kind = BTreeMap::new();
        for profile in profiles {
            if by_kind
                .insert(profile.contract()?.kind_id, profile)
                .is_some()
            {
                return Err("duplicate atomic pair profile".into());
            }
        }
        let mut selected = BTreeMap::new();
        let mut counts = BTreeMap::<KindId, u16>::new();
        for gear in plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.implementation_id.as_str() == PAIR_IMPLEMENTATION)
        {
            let profile = by_kind
                .get(&gear.kind_id)
                .ok_or("unadmitted atomic pair profile")?;
            verify_fixed_placement(gear, &profile.offer()?).map_err(|e| format!("{e:?}"))?;
            let count = counts.entry(gear.kind_id.clone()).or_default();
            *count = count.checked_add(1).ok_or("pair instance count overflow")?;
            if *count > profile.contract()?.limits.max_active_instances {
                return Err("pair selected capacity exceeded".into());
            }
            selected.insert(gear.placement_id.clone(), profile.clone());
        }
        Ok(Self {
            identity: ImplementationId::from(PAIR_IMPLEMENTATION),
            selected,
        })
    }
    fn profile(&self, gear: &PlannedGear) -> Result<&ClosingStructuredPairProfile, String> {
        let profile = self
            .selected
            .get(&gear.placement_id)
            .ok_or("unselected atomic pair placement")?;
        verify_fixed_placement(gear, &profile.offer()?).map_err(|e| format!("{e:?}"))?;
        Ok(profile)
    }
}
impl KernelOperationFactory for ClosingStructuredPairOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.identity
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.profile(gear)?;
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

impl ClosingStructuredPairOperationFactory {
    pub fn prepare_with_inventory(
        &self,
        gear: &PlannedGear,
    ) -> Result<super::prepared_numeric_back::PreparedNumericBack, String> {
        let back =
            ClosingStructuredPairBack::prepare_planned::<PORTS>(self.profile(gear)?, gear, 3)?;
        let local = back.local_accounted_heap_bytes();
        Ok(super::prepared_numeric_back::PreparedNumericBack::new(
            back, local,
        ))
    }
}
