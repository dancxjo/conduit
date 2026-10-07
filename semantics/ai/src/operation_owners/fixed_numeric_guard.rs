//! Exact sealed-Plan owner for generic closing-Flow Boolean assertion.
use crate::{fixed_numeric_guard::*, fixed_numeric_preparation::verify_fixed_placement};
use alloc::{boxed::Box, collections::BTreeMap, format, string::String, vec::Vec};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
pub struct FixedGuardOperationFactory {
    identity: ImplementationId,
    selected: BTreeMap<PlacementId, FixedGuardProfile>,
}
impl FixedGuardOperationFactory {
    pub fn for_plan(plan: &Plan, profiles: Vec<FixedGuardProfile>) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err("guard requires one sealed fragment".into());
        }
        let mut by_kind = BTreeMap::new();
        for profile in profiles {
            if by_kind
                .insert(profile.contract()?.kind_id, profile)
                .is_some()
            {
                return Err("duplicate guard profile".into());
            }
        }
        let mut selected = BTreeMap::new();
        for gear in plan.fragments[0]
            .placements
            .iter()
            .filter(|gear| gear.implementation_id.as_str() == GUARD_IMPLEMENTATION)
        {
            let profile = by_kind
                .get(&gear.kind_id)
                .ok_or("unadmitted guard profile")?;
            verify_fixed_placement(gear, &profile.offer()?).map_err(|e| format!("{e:?}"))?;
            selected.insert(gear.placement_id.clone(), profile.clone());
        }
        Ok(Self {
            identity: ImplementationId::from(GUARD_IMPLEMENTATION),
            selected,
        })
    }
    fn profile(&self, gear: &PlannedGear) -> Result<&FixedGuardProfile, String> {
        let profile = self
            .selected
            .get(&gear.placement_id)
            .ok_or("unselected guard placement")?;
        verify_fixed_placement(gear, &profile.offer()?).map_err(|e| format!("{e:?}"))?;
        Ok(profile)
    }
}
impl KernelOperationFactory for FixedGuardOperationFactory {
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
        Ok(Box::new(FixedGuardBack::prepare_planned::<PORTS>(
            self.profile(gear)?,
            gear,
            3,
        )?))
    }
}
