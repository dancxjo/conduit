//! Exact pre-Play child-pool handoff for the one supported pure Todo scan.
//! This does not advertise a coordinator offer or authorize Body activation
//! Play; the Body entrance retains its explicit refusal until integration proof.

use super::super::{back::BackBudget, body_scan_back::BodyScanBack};
use conduit_body::BodyPlotPlan;
use conduit_core::{
    prepare_plan_on_hosts, ActivePlayId, PlacementId, PlannedActivationEntry,
    PreparationHostIdentity,
};

pub(super) struct PreparedBodyScan {
    pub partition: usize,
    pub owner: PlacementId,
    pub budget: BackBudget,
    pub back: Option<BodyScanBack>,
}

pub(super) fn prepare(
    partitions: &[BodyPlotPlan],
    parent_play: &ActivePlayId,
) -> Result<Vec<PreparedBodyScan>, String> {
    let mut scans = Vec::new();
    for (partition_index, partition) in partitions.iter().enumerate() {
        if partition.plan.activations.is_empty() {
            continue;
        }
        if partition.plan.activations.len() != 1 {
            return Err("installed Body supports at most one exact scan per Plot".into());
        }
        let PlannedActivationEntry::Scan(planned) = &partition.plan.activations[0] else {
            return Err("installed Body activation Kind is unsupported".into());
        };
        let fragment = partition
            .plan
            .fragments
            .first()
            .filter(|_| partition.plan.fragments.len() == 1)
            .ok_or("installed scan requires one local parent fragment")?;
        let owner = fragment
            .placements
            .iter()
            .find(|placement| placement.placement_id == planned.owner_placement_id)
            .ok_or("installed scan has no exact owner placement")?;
        if owner.kind_id.as_str() != conduit_semantic_catalog::FLOW_SCAN_KIND
            || !owner.host_calls.is_empty()
            || !owner.resources.is_empty()
            || !owner.authority.is_empty()
            || planned.limits.maximum_items > 32
        {
            return Err("installed pure Todo scan parent or bound is unsupported".into());
        }
        let identity = PreparationHostIdentity {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
        };
        let mut host = crate::flow_activation::StdActivationHost::new(
            identity,
            crate::flow_activation::standard_child_registry()?,
        );
        let mut prepared = prepare_plan_on_hosts(&partition.plan, &mut [&mut host])
            .map_err(|error| format!("prepare exact scan Plan: {error:?}"))?;
        let scan = crate::flow_activation::install_pure_todo_scan(
            &partition.plan,
            &mut prepared,
            &planned.activation_id,
            &mut host,
        )?;
        let mut back = BodyScanBack::prepare(scan, planned)?;
        back.bind_parent_play(parent_play)?;
        let items = planned
            .limits
            .maximum_items
            .checked_add(2)
            .ok_or("installed scan value item overflow")?;
        let maximum_value_bytes = planned
            .retained_accumulator_bytes
            .max(planned.retained_item_bytes);
        let value_bytes = maximum_value_bytes
            .checked_mul(u32::from(items))
            .filter(|bytes| *bytes <= 16 * 1024 * 1024)
            .ok_or("installed scan value byte capacity exceeded")?;
        let sign_items = planned
            .limits
            .maximum_items
            .checked_mul(64)
            .and_then(|n| n.checked_add(8))
            .ok_or("installed scan Sign capacity overflow")?;
        scans.push(PreparedBodyScan {
            partition: partition_index,
            owner: planned.owner_placement_id.clone(),
            budget: BackBudget {
                value_items: items,
                value_bytes,
                host_requests: 0,
                sign_items,
                maximum_value_bytes,
            },
            back: Some(back),
        });
    }
    Ok(scans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::ResidentPlot;

    #[test]
    fn exact_todo_child_pool_is_prepared_before_body_play() {
        let plan = crate::flow_activation::tests::todo_scan_plan();
        let partition = BodyPlotPlan {
            plot: ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        };
        let scans = prepare(&[partition], &ActivePlayId::from("body/play/test"))
            .expect("exact pure Todo child Plan prepares before Body Play");
        assert_eq!(scans.len(), 1);
        assert!(scans[0].back.is_some());
        assert_eq!(scans[0].owner.as_str(), "placement");
        assert_eq!(scans[0].budget.host_requests, 0);
    }
}
