//! Exact ordinary numerical Each ownership before the Body kernel starts.
use super::super::back::BackBudget;
use conduit_body::BodyPlotPlan;
use conduit_core::{
    prepare_plan_on_hosts, ActivePlayId, PlacementId, PlannedActivationEntry,
    PreparationHostIdentity,
};

pub(super) struct PreparedBodyEach {
    pub partition: usize,
    pub owner: PlacementId,
    pub budget: BackBudget,
    pub back: Option<conduit_composite::KernelEachBack>,
}
pub(super) fn prepare(
    partitions: &[BodyPlotPlan],
    parent: &ActivePlayId,
    preloaded: usize,
) -> Result<Vec<PreparedBodyEach>, String> {
    let mut prepared = Vec::new();
    for (index, partition) in partitions.iter().enumerate() {
        if partition.plan.activations.is_empty() {
            continue;
        }
        if partition.plan.activations.len() != 1 {
            return Err("Body supports one exact activation per Plot".into());
        }
        let PlannedActivationEntry::Unary(each) = &partition.plan.activations[0] else {
            continue;
        };
        crate::flow_activation::validate_near_each(&partition.plan, each)?;
        if preloaded > usize::from(each.limits.maximum_items) {
            return Err("preloaded numerical Values exceed Each admission".into());
        }
        let fragment = partition
            .plan
            .fragments
            .first()
            .filter(|_| partition.plan.fragments.len() == 1)
            .ok_or("Each requires one local parent fragment")?;
        let identity = PreparationHostIdentity {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
        };
        let mut host = crate::flow_activation::StdActivationHost::new(
            identity,
            crate::flow_activation::standard_child_registry()?,
        );
        let mut receipts = prepare_plan_on_hosts(&partition.plan, &mut [&mut host])
            .map_err(|e| format!("prepare Each Plan: {e:?}"))?;
        let mut back = crate::flow_activation::install_pure_near_each(
            &partition.plan,
            &mut receipts,
            &each.activation_id,
            &mut host,
        )?;
        back.bind_parent_play(parent)?;
        let (maximum, value_bytes) = each.selected_plan.fragments[0].fore_ports.iter().try_fold(
            (1_u32, 0_u32),
            |(maximum, bytes), port| {
                bytes
                    .checked_add(port.byte_capacity)
                    .filter(|bytes| *bytes <= 16 * 1024 * 1024)
                    .map(|bytes| (maximum.max(port.byte_capacity), bytes))
                    .ok_or("Each value bytes overflow")
            },
        )?;
        let sign_items = each
            .limits
            .maximum_items
            .checked_mul(64)
            .and_then(|items| items.checked_add(8))
            .ok_or("Each Sign budget overflow")?;
        prepared.push(PreparedBodyEach {
            partition: index,
            owner: each.owner_placement_id.clone(),
            budget: BackBudget {
                value_items: 2,
                value_bytes,
                maximum_value_bytes: maximum,
                host_requests: 0,
                sign_items,
            },
            back: Some(back),
        });
    }
    Ok(prepared)
}
