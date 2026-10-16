//! Single-use initialized scan lease for an exact parent kernel placement.
use super::{
    install_value_scan, value_scan_child_steps, StdActivationHost, VALUE_SCAN_MAXIMUM_ITEMS,
};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory, KernelScanBack};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use std::sync::Mutex;

/// Retains the already prepared child kernels. It cannot re-plan, prepare a
/// second invocation pool, or substitute another parent placement at Play time.
pub struct PreparedValueScanFactory {
    placement: PlannedGear,
    budget: KernelOperationBudget,
    back: Mutex<Option<KernelScanBack>>,
}
impl PreparedValueScanFactory {
    pub fn prepare(
        plan: &Plan,
        prepared: &mut PreparedPlan,
        activation_id: &str,
        host: &mut StdActivationHost,
    ) -> Result<Self, String> {
        let planned = plan
            .activations
            .iter()
            .find_map(|entry| match entry {
                PlannedActivationEntry::Scan(scan) if scan.activation_id == activation_id => {
                    Some(scan)
                }
                _ => None,
            })
            .ok_or("prepared Value scan activation is absent")?;
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment
                    .placements
                    .iter()
                    .any(|gear| gear.placement_id == planned.owner_placement_id)
            })
            .ok_or("prepared Value scan owner fragment is absent")?;
        let placement = fragment
            .placements
            .iter()
            .find(|gear| gear.placement_id == planned.owner_placement_id)
            .unwrap()
            .clone();
        let scan = install_value_scan(plan, prepared, activation_id, host)?;
        let mut back = KernelScanBack::prepare(
            scan,
            planned,
            VALUE_SCAN_MAXIMUM_ITEMS,
            value_scan_child_steps(planned)?,
        )?;
        let play = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0)
            .active_play_id;
        back.bind_parent_play(&play)?;
        let input = planned.retained_item_bytes;
        let output = planned.retained_accumulator_bytes;
        Ok(Self {
            placement,
            budget: KernelOperationBudget {
                value_items: 2,
                value_bytes: input
                    .checked_add(output)
                    .ok_or("scan transport budget overflow")?,
                maximum_value_bytes: input.max(output),
                host_requests: 0,
                sign_items: 4,
            },
            back: Mutex::new(Some(back)),
        })
    }
    fn validate(&self, placement: &PlannedGear) -> Result<(), String> {
        if placement != &self.placement {
            return Err("initialized Value scan belongs to another exact parent placement".into());
        }
        Ok(())
    }
}
impl KernelOperationFactory for PreparedValueScanFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.placement.implementation_id
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.validate(placement)?;
        Ok(self.budget)
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        self.validate(placement)?;
        let back = self
            .back
            .lock()
            .map_err(|_| "initialized scan lease poisoned")?
            .take()
            .ok_or("initialized scan lease was already transferred")?;
        Ok(Box::new(back))
    }
}
