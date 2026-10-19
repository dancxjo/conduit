use super::*;
use alloc::{boxed::Box, collections::BTreeMap};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_kernel::HostedValueStore;
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;
pub struct CategoricalOperationFactory {
    implementation: ImplementationId,
    selected: BTreeMap<PlacementId, (Arc<PreparedCategoricalStep>, bool)>,
    #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
    outlets: BTreeMap<PlacementId, invocation::CategoricalInvocationOutlet>,
}
impl CategoricalOperationFactory {
    pub fn for_plan(
        plan: &Plan,
        profiles: &[Arc<PreparedCategoricalStep>],
    ) -> Result<Self, String> {
        if !verify_plan(plan) {
            return Err("categorical requires sealed Plan".into());
        }
        let mut selected = BTreeMap::new();
        for gear in plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .filter(|g| g.implementation_id.as_str() == CATEGORICAL_STEP_IMPLEMENTATION)
        {
            let mut matches = profiles
                .iter()
                .flat_map(|p| [false, true].map(move |flow| (p, flow)))
                .filter(|(p, flow)| p.verify_placement(gear, *flow).is_ok());
            let (profile, flow) = matches
                .next()
                .ok_or("unadmitted selected categorical model")?;
            if matches.next().is_some() {
                return Err("ambiguous selected categorical model".into());
            }
            if selected
                .insert(gear.placement_id.clone(), (profile.clone(), flow))
                .is_some()
            {
                return Err("duplicate categorical placement".into());
            }
        }
        Ok(Self {
            implementation: CATEGORICAL_STEP_IMPLEMENTATION.into(),
            selected,
            #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
            outlets: BTreeMap::new(),
        })
    }
    /// Each returned outlet may attach to exactly one prepared Back, never another Play.
    #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
    pub fn for_plan_with_invocation_outlets(
        plan: &Plan,
        profiles: &[Arc<PreparedCategoricalStep>],
        slots_per_placement: usize,
    ) -> Result<(Self, Vec<invocation::CategoricalInvocationOutlet>), String> {
        let mut factory = Self::for_plan(plan, profiles)?;
        for gear in plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
        {
            if let Some((profile, flow)) = factory.selected.get(&gear.placement_id) {
                let outlet = invocation::CategoricalInvocationOutlet::prepare(
                    plan,
                    gear,
                    profile.clone(),
                    *flow,
                    slots_per_placement,
                )?;
                factory.outlets.insert(gear.placement_id.clone(), outlet);
            }
        }
        let readers = factory.outlets.values().cloned().collect();
        Ok((factory, readers))
    }
    fn selected(
        &self,
        gear: &PlannedGear,
    ) -> Result<&(Arc<PreparedCategoricalStep>, bool), String> {
        let selected = self
            .selected
            .get(&gear.placement_id)
            .ok_or("unselected categorical placement")?;
        selected.0.verify_placement(gear, selected.1)?;
        Ok(selected)
    }
}
impl KernelOperationFactory for CategoricalOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let (profile, _) = self.selected(gear)?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: profile.output_bytes,
            maximum_value_bytes: profile.input_bytes.max(profile.output_bytes),
            host_requests: 0,
            sign_items: 16,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<PORTS> + Send>, String> {
        let (profile, flow) = self.selected(gear)?;
        let back = CategoricalStepBack::prepare_planned::<PORTS>(gear, 2, profile.clone(), *flow)?;
        #[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
        let back = {
            let mut back = back;
            if let Some(outlet) = self.outlets.get(&gear.placement_id) {
                back.observe(outlet, gear)?;
            }
            back
        };
        Ok(Box::new(back))
    }
}
