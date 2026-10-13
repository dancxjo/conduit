//! Finite exact objective arithmetic, independent of resource/Line admission.
use alloc::{vec, vec::Vec};
use conduit_core::{ExactNumericRefusal, ExactNumericWork, ExactRational};
use conduit_plot::CheckedPlot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalPlacementObjective {
    AggregateFulfillmentTime,
    /// Earliest modeled completion for an acyclic finite dependency graph.
    /// Per-operation fulfillment estimates include their declared stages.
    /// This does not impose serial Host scheduling or a hard deadline.
    CriticalPathFulfillmentTime,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalObjectiveRefusal {
    UnknownConnectionEndpoint,
    CyclicDependency,
}

pub(super) fn topological_order(plot: &CheckedPlot) -> Result<Vec<usize>, GlobalObjectiveRefusal> {
    let mut indegree = vec![0u16; plot.gears.len()];
    for cord in &plot.connections {
        let sink = plot
            .gears
            .iter()
            .position(|gear| gear.gear_id == cord.sink_gear_id)
            .ok_or(GlobalObjectiveRefusal::UnknownConnectionEndpoint)?;
        if !plot
            .gears
            .iter()
            .any(|gear| gear.gear_id == cord.source_gear_id)
        {
            return Err(GlobalObjectiveRefusal::UnknownConnectionEndpoint);
        }
        indegree[sink] += 1;
    }
    let mut order = Vec::with_capacity(plot.gears.len());
    while order.len() < plot.gears.len() {
        let next = (0..plot.gears.len())
            .find(|index| indegree[*index] == 0 && !order.contains(index))
            .ok_or(GlobalObjectiveRefusal::CyclicDependency)?;
        order.push(next);
        for cord in &plot.connections {
            if cord.source_gear_id == plot.gears[next].gear_id {
                let sink = plot
                    .gears
                    .iter()
                    .position(|gear| gear.gear_id == cord.sink_gear_id)
                    .ok_or(GlobalObjectiveRefusal::UnknownConnectionEndpoint)?;
                indegree[sink] -= 1;
            }
        }
    }
    Ok(order)
}
pub(super) fn critical_path_with_transfers(
    plot: &CheckedPlot,
    order: &[usize],
    costs: &[ExactRational],
    transfers: &[ExactRational],
    work: &mut ExactNumericWork,
) -> Result<ExactRational, ExactNumericRefusal> {
    let edges: Vec<_> = plot
        .connections
        .iter()
        .enumerate()
        .map(|(index, cord)| {
            (
                plot.gears
                    .iter()
                    .position(|gear| gear.gear_id == cord.source_gear_id)
                    .expect("validated source"),
                plot.gears
                    .iter()
                    .position(|gear| gear.gear_id == cord.sink_gear_id)
                    .expect("validated sink"),
                transfers[index],
            )
        })
        .collect();
    longest_path(order, costs, &edges, work)
}
fn longest_path(
    order: &[usize],
    costs: &[ExactRational],
    edges: &[(usize, usize, ExactRational)],
    work: &mut ExactNumericWork,
) -> Result<ExactRational, ExactNumericRefusal> {
    let mut finish = vec![ExactRational::ZERO; costs.len()];
    let mut maximum = ExactRational::ZERO;
    for index in order {
        let mut arrived = ExactRational::ZERO;
        for (source, sink, transfer) in edges {
            if sink == index {
                let delivered = finish[*source].checked_add(*transfer, work)?;
                if delivered.checked_compare(arrived, work)?.is_gt() {
                    arrived = delivered;
                }
            }
        }
        finish[*index] = arrived.checked_add(costs[*index], work)?;
        if finish[*index].checked_compare(maximum, work)?.is_gt() {
            maximum = finish[*index];
        }
    }
    Ok(maximum)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_predecessor_adds_its_own_transfer_before_join() {
        let costs = [2, 7, 5].map(ExactRational::from_i128);
        assert_eq!(
            longest_path(
                &[0, 1, 2],
                &costs,
                &[
                    (0, 2, ExactRational::from_i128(20)),
                    (1, 2, ExactRational::from_i128(1))
                ],
                &mut ExactNumericWork::new(100_000_000)
            )
            .unwrap(),
            ExactRational::from_i128(27)
        );
    }
    #[test]
    fn parallel_predecessors_take_maximum_before_serial_successor() {
        let costs = [2, 7, 5].map(ExactRational::from_i128);
        assert_eq!(
            longest_path(
                &[0, 1, 2],
                &costs,
                &[(0, 2, ExactRational::ZERO), (1, 2, ExactRational::ZERO)],
                &mut ExactNumericWork::new(100_000_000)
            )
            .unwrap(),
            ExactRational::from_i128(12)
        );
        assert_eq!(
            longest_path(
                &[0, 1, 2],
                &costs,
                &[],
                &mut ExactNumericWork::new(100_000_000)
            )
            .unwrap(),
            ExactRational::from_i128(7)
        );
        assert_eq!(
            longest_path(
                &[0, 1, 2],
                &costs,
                &[(0, 1, ExactRational::ZERO), (1, 2, ExactRational::ZERO)],
                &mut ExactNumericWork::new(100_000_000)
            )
            .unwrap(),
            ExactRational::from_i128(14)
        );
    }
}
