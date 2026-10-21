//! Immutable preparation of exact execution regions and typed local transfers.
//!
//! This is a preparation boundary, not a worker backend or a reservation. The
//! caller must still admit current offers, reserve storage and lanes atomically,
//! and install execution before starting Play. Cross-region
//! execution requires an installed backend; this API never invents a Cord or moves an effect.

mod transfers;
pub use transfers::PreparedRegionTransfer;
mod lane_offers;
pub use lane_offers::{CheckedRegionLaneOffers, LaneOfferError};

use alloc::vec::Vec;
use conduit_core::{ExecutionRegion, FragmentId, Plan, PlanFragment};
use conduit_kernel::{CordId, NodeId};

use crate::{
    activation_fragment::LoweredFragmentActivations,
    lowering::{
        lower_plan_fragment_for_profile_from_plan, LoweredPlanFragment, LoweringError,
        FIXED_KERNEL_STORAGE_PROFILE,
    },
};

/// Finite preparation ceilings supplied by the intended backend. These are
/// capacity limits, not advertised availability or proof of a reservation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExecutionRegionLimits {
    pub maximum_regions: usize,
    pub maximum_lanes: u32,
    pub maximum_nodes: usize,
    pub maximum_cords: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionPreparationError {
    UnknownFragment,
    NoRegions,
    CapacityExceeded,
    UnsupportedCrossRegionDependency,
    UnsupportedSharedPoolPartition,
    UnsupportedActivationPartition,
    Lowering(LoweringError),
}

/// Exact borrowed admission facts plus numeric membership in the original
/// lowered fragment. Numeric IDs are never renumbered per region.
#[derive(Debug)]
pub struct PreparedExecutionRegion<'plan> {
    admitted: &'plan ExecutionRegion,
    nodes: Vec<NodeId>,
    cords: Vec<CordId>,
}

impl<'plan> PreparedExecutionRegion<'plan> {
    pub fn admitted(&self) -> &'plan ExecutionRegion {
        self.admitted
    }

    pub fn nodes(&self) -> &[NodeId] {
        &self.nodes
    }

    pub fn cords(&self) -> &[CordId] {
        &self.cords
    }
}

/// Holding the sealed Plan by immutable borrow prevents detached, mutable
/// partition metadata from being mistaken for current Plan truth.
#[derive(Debug)]
pub struct PreparedExecutionRegions<'plan> {
    plan: &'plan Plan,
    fragment: &'plan PlanFragment,
    lowered: LoweredPlanFragment,
    activations: LoweredFragmentActivations,
    regions: Vec<PreparedExecutionRegion<'plan>>,
    transfers: Vec<PreparedRegionTransfer<'plan>>,
    requested_lanes: u32,
}

impl<'plan> PreparedExecutionRegions<'plan> {
    pub fn prepare(
        plan: &'plan Plan,
        fragment_id: &FragmentId,
        limits: ExecutionRegionLimits,
    ) -> Result<Self, RegionPreparationError> {
        let fragment = plan
            .fragments
            .iter()
            .find(|f| &f.fragment_id == fragment_id)
            .ok_or(RegionPreparationError::UnknownFragment)?;
        if fragment.execution_regions.is_empty() {
            return Err(RegionPreparationError::NoRegions);
        }
        let requested_lanes = fragment
            .execution_regions
            .iter()
            .try_fold(0u32, |total, region| total.checked_add(region.lane_count))
            .ok_or(RegionPreparationError::CapacityExceeded)?;
        if fragment.execution_regions.len() > limits.maximum_regions
            || requested_lanes > limits.maximum_lanes
            || fragment.placements.len() > limits.maximum_nodes
            || fragment.connections.len() > limits.maximum_cords
        {
            return Err(RegionPreparationError::CapacityExceeded);
        }
        // This entrance validates the complete seal and retains activation
        // truth. A caller cannot supply a different lowered graph or profile.
        let lowered = lower_plan_fragment_for_profile_from_plan(
            plan,
            fragment_id,
            FIXED_KERNEL_STORAGE_PROFILE.with_partitioned_regions(),
        )
        .map_err(RegionPreparationError::Lowering)?;
        let activations = crate::activation_fragment::lower_fragment_activations(plan, fragment_id)
            .map_err(|_| RegionPreparationError::Lowering(LoweringError::InvalidFragment))?;
        let region_for = |placement: &conduit_core::PlacementId| {
            fragment
                .execution_regions
                .iter()
                .position(|region| region.admitted_placements.contains(placement))
        };
        // Byte Cords may cross only under the explicit partitioned profile.
        // Resource moves, startup ordering and fusion still require closure.
        let crosses = fragment.startup_dependencies.iter().any(|dependency| {
            region_for(&dependency.prerequisite_placement_id)
                != region_for(&dependency.dependent_placement_id)
        }) || lowered.resource_cords.iter().any(|cord| {
            let source = lowered.identity.placement_for_node(cord.source_node);
            let sink = lowered.identity.placement_for_node(cord.sink_node);
            source.and_then(&region_for) != sink.and_then(&region_for)
        }) || fragment.execution_fusions.iter().any(|fusion| {
            let first = fusion.preserved_placements.first().and_then(&region_for);
            fusion
                .preserved_placements
                .iter()
                .any(|p| region_for(p) != first)
        });
        if crosses {
            return Err(RegionPreparationError::UnsupportedCrossRegionDependency);
        }
        if fragment.execution_regions.len() > 1 {
            // Pool and activation coordinators need an explicit partition
            // ownership contract before separate workers can share them.
            if !lowered.shared_pools.is_empty() {
                return Err(RegionPreparationError::UnsupportedSharedPoolPartition);
            }
            if !activations.entries.is_empty() {
                return Err(RegionPreparationError::UnsupportedActivationPartition);
            }
        }
        let mut regions = Vec::with_capacity(fragment.execution_regions.len());
        for admitted in &fragment.execution_regions {
            let nodes: Vec<_> = lowered
                .nodes
                .iter()
                .filter(|node| admitted.admitted_placements.contains(&node.placement_id))
                .map(|node| node.node)
                .collect();
            let cords = lowered
                .cords
                .iter()
                .filter(|cord| {
                    fragment.connections.iter().any(|connection| {
                        connection.connection_id == cord.connection_id
                            && admitted
                                .admitted_placements
                                .contains(&connection.sink_placement_id)
                    })
                })
                .map(|cord| cord.spec.cord)
                .collect();
            regions.push(PreparedExecutionRegion {
                admitted,
                nodes,
                cords,
            });
        }
        let transfers = transfers::prepare(fragment, &lowered)?;
        Ok(Self {
            plan,
            fragment,
            lowered,
            activations,
            regions,
            transfers,
            requested_lanes,
        })
    }

    pub fn plan(&self) -> &'plan Plan {
        self.plan
    }
    pub fn fragment(&self) -> &'plan PlanFragment {
        self.fragment
    }
    pub fn lowered(&self) -> &LoweredPlanFragment {
        &self.lowered
    }
    pub fn activations(&self) -> &LoweredFragmentActivations {
        &self.activations
    }
    pub fn regions(&self) -> &[PreparedExecutionRegion<'plan>] {
        &self.regions
    }
    pub fn transfers(&self) -> &[PreparedRegionTransfer<'plan>] {
        &self.transfers
    }
    pub fn requested_lanes(&self) -> u32 {
        self.requested_lanes
    }
}
