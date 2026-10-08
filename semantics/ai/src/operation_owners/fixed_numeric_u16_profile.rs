//! Exact selected Source profile admission owner; resource/model custody is unrelated.
use crate::{
    fixed_numeric_preparation::verify_fixed_placement,
    fixed_numeric_u16_profile::{PreparedU16Profile, U16_PROFILE_IMPLEMENTATION, U16ProfileBack},
};
use alloc::{boxed::Box, format, string::String};
use alloc::{collections::BTreeMap, sync::Arc};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
struct SelectedProfile {
    profile: Arc<PreparedU16Profile>,
    flow: bool,
    offer: CapabilityOffer,
}
pub struct U16ProfileOperationFactory {
    implementation: ImplementationId,
    selected: BTreeMap<PlacementId, SelectedProfile>,
}
impl U16ProfileOperationFactory {
    pub fn for_plan(plan: &Plan, profiles: &[Arc<PreparedU16Profile>]) -> Result<Self, String> {
        let mut selected = BTreeMap::new();
        for gear in plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|gear| gear.implementation_id.as_str() == U16_PROFILE_IMPLEMENTATION)
        {
            let mut matches = profiles
                .iter()
                .flat_map(|profile| [false, true].map(move |flow| (profile, flow)))
                .filter(|(profile, flow)| profile.kind_identity(*flow) == gear.kind_id.as_str());
            let (profile, flow) = matches
                .next()
                .ok_or("selected U16profile has no admitted Source declaration")?;
            if matches.next().is_some() {
                return Err("ambiguous selected U16profile".into());
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
            implementation: U16_PROFILE_IMPLEMENTATION.into(),
            selected,
        })
    }
    fn selected(&self, gear: &PlannedGear) -> Result<&SelectedProfile, String> {
        let selected = self
            .selected
            .get(&gear.placement_id)
            .ok_or("no exact selected U16profile")?;
        verify_fixed_placement(gear, &selected.offer).map_err(|e| format!("{e:?}"))?;
        Ok(selected)
    }
}
impl KernelOperationFactory for U16ProfileOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let selected = self.selected(gear)?;
        let maximum = crate::transport_envelope::maximum_prepared_transport_value_bytes(
            selected.profile.value_type(),
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
        _values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        let selected = self.selected(gear)?;
        Ok(Box::new(U16ProfileBack::prepare_planned::<
            FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
        >(
            gear, 2, &selected.profile, selected.flow
        )?))
    }
}
