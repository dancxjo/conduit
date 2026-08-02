//! Hosted ownership and compilation of one exact physical execution arrangement.
//!
//! The logical [`conduit_core::ExecutionPlan`] remains unchanged. This module
//! binds a finite region/lane arrangement to that plan, the exact resolver
//! decision, and one plan epoch.

use std::collections::BTreeMap;
use std::fmt;

use conduit_core::{
    BoundednessProfile, CommitOrdering, ExecutionArrangement, ExecutionBoundary,
    ExecutionCommitDomain, ExecutionContractError, ExecutionGuarantee, ExecutionLogicalCord,
    ExecutionPlan, ExecutionRegion, Id, InstancePath, PinnedDescriptor, SemanticHash,
    validate_execution_arrangement,
};
use sha2::{Digest, Sha256};

use crate::host_resolution::{
    PlanSealingReason, ResolvedExecutionDescriptor, ResolvedExecutionLane,
    ResolvedExecutionPlacement, ResolvedPlacement, seal_resolved_execution_plan,
};

/// Explicit policy-owned bounds for physical arrangement compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionArrangementPolicy<'a> {
    pub plan_epoch: u64,
    pub boundary_realization: PinnedDescriptor<'a>,
    pub maximum_proposal_bytes: u64,
    pub maximum_head_of_line_ticks: u64,
    pub cancellation_slots: u16,
    pub evidence_slots: u32,
}

/// One owned physical region. Members retain their logical instance paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExecutionRegion {
    pub id: String,
    pub members: Vec<String>,
    pub placement: String,
    pub placement_generation: u64,
    pub lane: String,
    pub lane_generation: u64,
    pub commit_domain: String,
    pub independent: bool,
    pub maximum_in_flight_proposals: u16,
    pub scratch_bytes: u32,
    pub retained_state_bytes: u64,
    pub pending_operation_slots: u16,
    pub timer_slots: u16,
    pub evidence_slots: u32,
}

/// One owned physical realization of a logical cross-region cord.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExecutionBoundary {
    pub cord: String,
    pub from_region: String,
    pub to_region: String,
    pub realization: ResolvedExecutionDescriptor,
    pub generation: u64,
    pub from_placement_generation: u64,
    pub to_placement_generation: u64,
    pub capacity_items: u16,
    pub capacity_bytes: u64,
    pub wake_slots: u16,
    pub evidence_slots: u32,
}

/// One owned deterministic publication domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExecutionCommitDomain {
    pub id: String,
    pub ordering: CommitOrdering,
    pub proposal_slots: u16,
    pub commit_slots: u16,
    pub maximum_proposal_bytes: u64,
    pub maximum_head_of_line_ticks: u64,
    pub cancellation_slots: u16,
    pub evidence_slots: u32,
}

/// Physical execution identity, deliberately distinct from the logical plan
/// and from the resolver decision that supplied host observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExecutionArrangement {
    pub identity: SemanticHash,
    pub plan_identity: SemanticHash,
    pub resolution_identity: SemanticHash,
    pub plan_epoch: u64,
    pub placements: Vec<ResolvedExecutionPlacement>,
    pub lanes: Vec<ResolvedExecutionLane>,
    pub regions: Vec<ResolvedExecutionRegion>,
    pub boundaries: Vec<ResolvedExecutionBoundary>,
    pub commit_domains: Vec<ResolvedExecutionCommitDomain>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionArrangementReason {
    InvalidPolicy,
    PlanNotSealed,
    ObservationMissing,
    PlacementUnavailable,
    LaneUnavailable,
    CapacityExceeded,
    IdentityCollision,
    Contract(ExecutionContractError),
    IdentityMismatch,
}

impl ExecutionArrangementReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "CND-EXA-001",
            Self::PlanNotSealed => "CND-EXA-002",
            Self::ObservationMissing => "CND-EXA-003",
            Self::PlacementUnavailable => "CND-EXA-004",
            Self::LaneUnavailable => "CND-EXA-005",
            Self::CapacityExceeded => "CND-EXA-006",
            Self::IdentityCollision => "CND-EXA-007",
            Self::Contract(error) => error.code(),
            Self::IdentityMismatch => "CND-EXA-008",
        }
    }
}

impl fmt::Display for ExecutionArrangementReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPolicy => "physical execution policy is invalid",
            Self::PlanNotSealed => "logical plan does not match the resolver decision",
            Self::ObservationMissing => "selected host execution observation is missing",
            Self::PlacementUnavailable => "selected host has no usable execution placement",
            Self::LaneUnavailable => "selected placement has no usable execution lane",
            Self::CapacityExceeded => "physical execution capacity is insufficient",
            Self::IdentityCollision => "physical execution identities collide",
            Self::Contract(_) => "physical execution arrangement violates the core contract",
            Self::IdentityMismatch => "physical execution identity does not match its contents",
        })
    }
}

#[derive(Clone, Copy, Default)]
struct LaneUsage {
    ready: u16,
    proposals: u16,
    timers: u16,
    scratch: u32,
    evidence: u32,
}

impl LaneUsage {
    fn admits(self, lane: &ResolvedExecutionLane, scratch: u32, timers: u16) -> bool {
        self.ready
            .checked_add(1)
            .is_some_and(|value| value <= lane.ready_slots)
            && self
                .proposals
                .checked_add(1)
                .is_some_and(|value| value <= lane.proposal_slots)
            && self
                .timers
                .checked_add(timers)
                .is_some_and(|value| value <= lane.timer_slots)
            && self
                .scratch
                .checked_add(scratch)
                .is_some_and(|value| value <= lane.scratch_bytes)
            && self
                .evidence
                .checked_add(1)
                .is_some_and(|value| value <= lane.evidence_slots)
    }

    fn reserve(&mut self, scratch: u32, timers: u16) -> Result<(), ExecutionArrangementReason> {
        self.ready = self
            .ready
            .checked_add(1)
            .ok_or(ExecutionArrangementReason::CapacityExceeded)?;
        self.proposals = self
            .proposals
            .checked_add(1)
            .ok_or(ExecutionArrangementReason::CapacityExceeded)?;
        self.timers = self
            .timers
            .checked_add(timers)
            .ok_or(ExecutionArrangementReason::CapacityExceeded)?;
        self.scratch = self
            .scratch
            .checked_add(scratch)
            .ok_or(ExecutionArrangementReason::CapacityExceeded)?;
        self.evidence = self
            .evidence
            .checked_add(1)
            .ok_or(ExecutionArrangementReason::CapacityExceeded)?;
        Ok(())
    }
}

/// Compile one conservative, deterministic physical arrangement from an exact
/// logical plan and the host observations retained by its resolver decision.
pub fn resolve_execution_arrangement(
    plan: &ExecutionPlan<'_>,
    resolution: &ResolvedPlacement,
    validation: conduit_core::PlanValidationContext<'_>,
    policy: ExecutionArrangementPolicy<'_>,
) -> Result<ResolvedExecutionArrangement, ExecutionArrangementReason> {
    if policy.plan_epoch == 0
        || policy.maximum_proposal_bytes == 0
        || policy.maximum_head_of_line_ticks == 0
        || policy.cancellation_slots == 0
        || policy.evidence_slots == 0
        || Id::new(policy.boundary_realization.id.as_str()).is_err()
        || policy.boundary_realization.schema_version != 0
        || policy.boundary_realization.semantic_hash == SemanticHash::from_bytes([0; 32])
    {
        return Err(ExecutionArrangementReason::InvalidPolicy);
    }
    seal_resolved_execution_plan(resolution, plan, validation)
        .map_err(|_: PlanSealingReason| ExecutionArrangementReason::PlanNotSealed)?;

    let structurally_simple = plan.feedback_boundaries.is_empty()
        && plan.distributed_cords.is_empty()
        && plan.fanouts.is_empty()
        && plan.merges.is_empty()
        && plan.event_streams.is_empty()
        && plan.jobs.is_empty()
        && plan.instance_pools.is_empty()
        && plan.supervisions.is_empty();
    let mut placement_set = BTreeMap::<String, ResolvedExecutionPlacement>::new();
    let mut lane_set = BTreeMap::<String, ResolvedExecutionLane>::new();
    let mut usage = BTreeMap::<String, LaneUsage>::new();
    let mut regions = Vec::with_capacity(plan.nodes.len());

    for node in plan.nodes {
        let binding = resolution
            .bindings
            .iter()
            .find(|binding| binding.instance == node.instance.as_str())
            .ok_or(ExecutionArrangementReason::PlanNotSealed)?;
        let observation = resolution
            .host_execution
            .iter()
            .find(|observation| {
                observation.report_id == binding.report_id
                    && observation.report_identity == binding.report_identity
            })
            .ok_or(ExecutionArrangementReason::ObservationMissing)?;
        let mut placements = observation.placements.iter().collect::<Vec<_>>();
        placements.sort_by_key(|placement| placement.id.as_str());
        let (placement, mut lanes) = placements
            .into_iter()
            .find_map(|placement| {
                let mut lanes = observation
                    .lanes
                    .iter()
                    .filter(|lane| {
                        lane.placement == placement.id
                            && lane.placement_generation == placement.generation
                    })
                    .collect::<Vec<_>>();
                lanes.sort_by_key(|lane| lane.id.as_str());
                (!lanes.is_empty()).then_some((placement, lanes))
            })
            .ok_or(ExecutionArrangementReason::PlacementUnavailable)?;
        let profile = node
            .execution_profile
            .ok_or(ExecutionArrangementReason::CapacityExceeded)?;
        let scratch = profile.limits.max_scratch_bytes.max(1);
        let timers = profile.limits.max_timers;
        let independently_computable = structurally_simple
            && node.required_effects.is_empty()
            && node.required_resources.is_empty()
            && !plan
                .authorities
                .iter()
                .any(|authority| authority.node == node.instance)
            && profile.boundedness == BoundednessProfile::Hard
            && profile.step_bound_enforced;
        if independently_computable {
            lanes.sort_by_key(|lane| {
                let eligible = lane.independent_progress == ExecutionGuarantee::Guaranteed
                    && lane.simultaneous_execution == ExecutionGuarantee::Guaranteed;
                (
                    !eligible,
                    usage.get(&lane.id).copied().unwrap_or_default().ready,
                    lane.id.as_str(),
                )
            });
        }
        let lane = lanes
            .into_iter()
            .find(|lane| {
                usage
                    .get(&lane.id)
                    .copied()
                    .unwrap_or_default()
                    .admits(lane, scratch, timers)
            })
            .ok_or(ExecutionArrangementReason::LaneUnavailable)?;
        let independent = independently_computable
            && lane.independent_progress == ExecutionGuarantee::Guaranteed
            && lane.simultaneous_execution == ExecutionGuarantee::Guaranteed;
        usage
            .entry(lane.id.clone())
            .or_default()
            .reserve(scratch, timers)?;
        insert_exact(&mut placement_set, placement.id.clone(), placement.clone())?;
        insert_exact(&mut lane_set, lane.id.clone(), lane.clone())?;
        regions.push(ResolvedExecutionRegion {
            id: format!("region/{}", node.instance.as_str()),
            members: vec![node.instance.as_str().to_owned()],
            placement: placement.id.clone(),
            placement_generation: placement.generation,
            lane: lane.id.clone(),
            lane_generation: lane.generation,
            commit_domain: "commit/global".to_owned(),
            independent,
            maximum_in_flight_proposals: 1,
            scratch_bytes: scratch,
            retained_state_bytes: profile.limits.max_retained_bytes,
            pending_operation_slots: profile.limits.max_pending_operations,
            timer_slots: timers,
            evidence_slots: 1,
        });
    }

    let region_by_node = regions
        .iter()
        .map(|region| (region.members[0].as_str(), region))
        .collect::<BTreeMap<_, _>>();
    let realization = ResolvedExecutionDescriptor {
        id: policy.boundary_realization.id.to_string(),
        schema_version: policy.boundary_realization.schema_version,
        semantic_hash: policy.boundary_realization.semantic_hash,
    };
    let mut boundaries = Vec::with_capacity(plan.cords.len());
    for cord in plan.cords {
        let from = region_by_node
            .get(cord.from.node.as_str())
            .ok_or(ExecutionArrangementReason::PlanNotSealed)?;
        let to = region_by_node
            .get(cord.to.node.as_str())
            .ok_or(ExecutionArrangementReason::PlanNotSealed)?;
        if from.id == to.id {
            continue;
        }
        let from_placement = placement_set
            .get(&from.placement)
            .ok_or(ExecutionArrangementReason::PlacementUnavailable)?;
        let to_placement = placement_set
            .get(&to.placement)
            .ok_or(ExecutionArrangementReason::PlacementUnavailable)?;
        boundaries.push(ResolvedExecutionBoundary {
            cord: cord.id.to_string(),
            from_region: from.id.clone(),
            to_region: to.id.clone(),
            realization: realization.clone(),
            generation: policy.plan_epoch,
            from_placement_generation: from_placement.generation,
            to_placement_generation: to_placement.generation,
            capacity_items: cord.flow.capacity.items(),
            capacity_bytes: cord.flow.capacity.max_queued_bytes(),
            wake_slots: 1,
            evidence_slots: 1,
        });
    }
    let slots =
        u16::try_from(regions.len()).map_err(|_| ExecutionArrangementReason::CapacityExceeded)?;
    let commit_domains = vec![ResolvedExecutionCommitDomain {
        id: "commit/global".to_owned(),
        ordering: CommitOrdering::DeterministicFrontier,
        proposal_slots: slots,
        commit_slots: slots,
        maximum_proposal_bytes: policy.maximum_proposal_bytes,
        maximum_head_of_line_ticks: policy.maximum_head_of_line_ticks,
        cancellation_slots: policy.cancellation_slots,
        evidence_slots: policy.evidence_slots,
    }];
    let mut arrangement = ResolvedExecutionArrangement {
        identity: SemanticHash::from_bytes([0; 32]),
        plan_identity: plan.identity,
        resolution_identity: resolution.computed_identity(),
        plan_epoch: policy.plan_epoch,
        placements: placement_set.into_values().collect(),
        lanes: lane_set.into_values().collect(),
        regions,
        boundaries,
        commit_domains,
    };
    arrangement.identity = arrangement.computed_identity();
    arrangement.validate(plan, resolution)?;
    Ok(arrangement)
}

fn insert_exact<T: Eq>(
    values: &mut BTreeMap<String, T>,
    id: String,
    value: T,
) -> Result<(), ExecutionArrangementReason> {
    if values.get(&id).is_some_and(|existing| existing != &value) {
        return Err(ExecutionArrangementReason::IdentityCollision);
    }
    values.entry(id).or_insert(value);
    Ok(())
}

impl ResolvedExecutionArrangement {
    #[must_use]
    pub fn computed_identity(&self) -> SemanticHash {
        let mut digest = Sha256::new();
        hash(&mut digest, b"kind", b"conduit/execution-arrangement");
        hash(&mut digest, b"plan", self.plan_identity.as_bytes());
        hash(
            &mut digest,
            b"resolution",
            self.resolution_identity.as_bytes(),
        );
        hash(&mut digest, b"epoch", &self.plan_epoch.to_be_bytes());
        hash_len(&mut digest, b"placements", self.placements.len());
        for placement in &self.placements {
            hash(&mut digest, b"placement-id", placement.id.as_bytes());
            hash(
                &mut digest,
                b"placement-host",
                placement.host_observation.as_bytes(),
            );
            hash(
                &mut digest,
                b"placement-generation",
                &placement.generation.to_be_bytes(),
            );
            hash_descriptor(&mut digest, b"placement-provider", &placement.provider);
            hash_descriptor(
                &mut digest,
                b"placement-authority",
                &placement.authority_boundary,
            );
            hash_descriptor(
                &mut digest,
                b"placement-resource",
                &placement.resource_boundary,
            );
            hash_descriptor(
                &mut digest,
                b"placement-lifecycle",
                &placement.lifecycle_boundary,
            );
            hash_descriptor(
                &mut digest,
                b"placement-failure",
                &placement.failure_boundary,
            );
            hash(
                &mut digest,
                b"placement-isolation",
                placement.isolation.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"placement-memory-containment",
                placement.memory_containment.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"placement-regain-control",
                placement.regain_control.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"placement-effect-fencing",
                placement.effect_fencing.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"placement-stop",
                placement.stop_execution.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"placement-reclaim",
                placement.reclaim_resources.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"placement-regain-ticks",
                &placement.maximum_regain_control_ticks.to_be_bytes(),
            );
        }
        hash_len(&mut digest, b"lanes", self.lanes.len());
        for lane in &self.lanes {
            hash(&mut digest, b"lane-id", lane.id.as_bytes());
            hash(&mut digest, b"lane-placement", lane.placement.as_bytes());
            hash(
                &mut digest,
                b"lane-placement-generation",
                &lane.placement_generation.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"lane-generation",
                &lane.generation.to_be_bytes(),
            );
            hash(&mut digest, b"lane-ready", &lane.ready_slots.to_be_bytes());
            hash(&mut digest, b"lane-wake", &lane.wake_slots.to_be_bytes());
            hash(
                &mut digest,
                b"lane-proposal",
                &lane.proposal_slots.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"lane-commit",
                &lane.commit_slots.to_be_bytes(),
            );
            hash(&mut digest, b"lane-timers", &lane.timer_slots.to_be_bytes());
            hash(
                &mut digest,
                b"lane-scratch",
                &lane.scratch_bytes.to_be_bytes(),
            );
            hash(&mut digest, b"lane-stack", &lane.stack_bytes.to_be_bytes());
            hash(
                &mut digest,
                b"lane-evidence",
                &lane.evidence_slots.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"lane-independent",
                lane.independent_progress.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"lane-simultaneous",
                lane.simultaneous_execution.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"lane-preemption",
                lane.preemption.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"lane-termination",
                lane.termination.as_str().as_bytes(),
            );
        }
        hash_len(&mut digest, b"regions", self.regions.len());
        for region in &self.regions {
            hash(&mut digest, b"region-id", region.id.as_bytes());
            hash_len(&mut digest, b"region-members", region.members.len());
            for member in &region.members {
                hash(&mut digest, b"region-member", member.as_bytes());
            }
            hash(
                &mut digest,
                b"region-placement",
                region.placement.as_bytes(),
            );
            hash(
                &mut digest,
                b"region-placement-generation",
                &region.placement_generation.to_be_bytes(),
            );
            hash(&mut digest, b"region-lane", region.lane.as_bytes());
            hash(
                &mut digest,
                b"region-lane-generation",
                &region.lane_generation.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"region-domain",
                region.commit_domain.as_bytes(),
            );
            hash(
                &mut digest,
                b"region-independent",
                &[u8::from(region.independent)],
            );
            hash(
                &mut digest,
                b"region-proposals",
                &region.maximum_in_flight_proposals.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"region-scratch",
                &region.scratch_bytes.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"region-retained",
                &region.retained_state_bytes.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"region-operations",
                &region.pending_operation_slots.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"region-timers",
                &region.timer_slots.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"region-evidence",
                &region.evidence_slots.to_be_bytes(),
            );
        }
        hash_len(&mut digest, b"boundaries", self.boundaries.len());
        for boundary in &self.boundaries {
            hash(&mut digest, b"boundary-cord", boundary.cord.as_bytes());
            hash(
                &mut digest,
                b"boundary-from",
                boundary.from_region.as_bytes(),
            );
            hash(&mut digest, b"boundary-to", boundary.to_region.as_bytes());
            hash_descriptor(&mut digest, b"boundary-realization", &boundary.realization);
            hash(
                &mut digest,
                b"boundary-generation",
                &boundary.generation.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"boundary-from-generation",
                &boundary.from_placement_generation.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"boundary-to-generation",
                &boundary.to_placement_generation.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"boundary-items",
                &boundary.capacity_items.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"boundary-bytes",
                &boundary.capacity_bytes.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"boundary-wakes",
                &boundary.wake_slots.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"boundary-evidence",
                &boundary.evidence_slots.to_be_bytes(),
            );
        }
        hash_len(&mut digest, b"commit-domains", self.commit_domains.len());
        for domain in &self.commit_domains {
            hash(&mut digest, b"domain-id", domain.id.as_bytes());
            hash(
                &mut digest,
                b"domain-ordering",
                domain.ordering.as_str().as_bytes(),
            );
            hash(
                &mut digest,
                b"domain-proposals",
                &domain.proposal_slots.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"domain-commits",
                &domain.commit_slots.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"domain-proposal-bytes",
                &domain.maximum_proposal_bytes.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"domain-head-ticks",
                &domain.maximum_head_of_line_ticks.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"domain-cancellations",
                &domain.cancellation_slots.to_be_bytes(),
            );
            hash(
                &mut digest,
                b"domain-evidence",
                &domain.evidence_slots.to_be_bytes(),
            );
        }
        SemanticHash::from_bytes(digest.finalize().into())
    }

    /// Borrow this owned arrangement as the allocator-free core contract for
    /// one provider admission call. No provider observation is rediscovered.
    pub fn with_contract<T>(
        &self,
        use_contract: impl FnOnce(ExecutionArrangement<'_>) -> T,
    ) -> Result<T, ExecutionArrangementReason> {
        let placements = self
            .placements
            .iter()
            .map(ResolvedExecutionPlacement::as_observation)
            .collect::<Vec<_>>();
        let lanes = self
            .lanes
            .iter()
            .map(ResolvedExecutionLane::as_observation)
            .collect::<Vec<_>>();
        let member_storage = self
            .regions
            .iter()
            .map(|region| {
                region
                    .members
                    .iter()
                    .map(|member| {
                        InstancePath::new(member)
                            .map_err(|_| ExecutionArrangementReason::IdentityCollision)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let regions = self
            .regions
            .iter()
            .zip(&member_storage)
            .map(|(region, members)| ExecutionRegion {
                id: Id(&region.id),
                members,
                placement: Id(&region.placement),
                placement_generation: region.placement_generation,
                lane: Id(&region.lane),
                lane_generation: region.lane_generation,
                commit_domain: Id(&region.commit_domain),
                independent: region.independent,
                maximum_in_flight_proposals: region.maximum_in_flight_proposals,
                scratch_bytes: region.scratch_bytes,
                retained_state_bytes: region.retained_state_bytes,
                pending_operation_slots: region.pending_operation_slots,
                timer_slots: region.timer_slots,
                evidence_slots: region.evidence_slots,
            })
            .collect::<Vec<_>>();
        let boundaries = self
            .boundaries
            .iter()
            .map(|boundary| ExecutionBoundary {
                cord: Id(&boundary.cord),
                from_region: Id(&boundary.from_region),
                to_region: Id(&boundary.to_region),
                realization: boundary.realization.as_pin(),
                generation: boundary.generation,
                from_placement_generation: boundary.from_placement_generation,
                to_placement_generation: boundary.to_placement_generation,
                capacity_items: boundary.capacity_items,
                capacity_bytes: boundary.capacity_bytes,
                wake_slots: boundary.wake_slots,
                evidence_slots: boundary.evidence_slots,
            })
            .collect::<Vec<_>>();
        let commit_domains = self
            .commit_domains
            .iter()
            .map(|domain| ExecutionCommitDomain {
                id: Id(&domain.id),
                ordering: domain.ordering,
                proposal_slots: domain.proposal_slots,
                commit_slots: domain.commit_slots,
                maximum_proposal_bytes: domain.maximum_proposal_bytes,
                maximum_head_of_line_ticks: domain.maximum_head_of_line_ticks,
                cancellation_slots: domain.cancellation_slots,
                evidence_slots: domain.evidence_slots,
            })
            .collect::<Vec<_>>();
        Ok(use_contract(ExecutionArrangement {
            placements: &placements,
            lanes: &lanes,
            regions: &regions,
            boundaries: &boundaries,
            commit_domains: &commit_domains,
        }))
    }

    pub fn validate(
        &self,
        plan: &ExecutionPlan<'_>,
        resolution: &ResolvedPlacement,
    ) -> Result<(), ExecutionArrangementReason> {
        if self.resolution_identity != resolution.computed_identity() {
            return Err(ExecutionArrangementReason::PlanNotSealed);
        }
        self.validate_for_plan(plan)
    }

    /// Validate the self-contained physical arrangement against its logical
    /// plan after the resolver decision has been sealed into the arrangement
    /// identity and persisted separately.
    pub fn validate_for_plan(
        &self,
        plan: &ExecutionPlan<'_>,
    ) -> Result<(), ExecutionArrangementReason> {
        if self.plan_epoch == 0 || self.plan_identity != plan.identity {
            return Err(ExecutionArrangementReason::PlanNotSealed);
        }
        if self.identity != self.computed_identity() {
            return Err(ExecutionArrangementReason::IdentityMismatch);
        }
        let placements = self
            .placements
            .iter()
            .map(ResolvedExecutionPlacement::as_observation)
            .collect::<Vec<_>>();
        let lanes = self
            .lanes
            .iter()
            .map(ResolvedExecutionLane::as_observation)
            .collect::<Vec<_>>();
        let member_storage = self
            .regions
            .iter()
            .map(|region| {
                region
                    .members
                    .iter()
                    .map(|member| {
                        InstancePath::new(member)
                            .map_err(|_| ExecutionArrangementReason::IdentityCollision)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let regions = self
            .regions
            .iter()
            .zip(&member_storage)
            .map(|(region, members)| ExecutionRegion {
                id: Id(&region.id),
                members,
                placement: Id(&region.placement),
                placement_generation: region.placement_generation,
                lane: Id(&region.lane),
                lane_generation: region.lane_generation,
                commit_domain: Id(&region.commit_domain),
                independent: region.independent,
                maximum_in_flight_proposals: region.maximum_in_flight_proposals,
                scratch_bytes: region.scratch_bytes,
                retained_state_bytes: region.retained_state_bytes,
                pending_operation_slots: region.pending_operation_slots,
                timer_slots: region.timer_slots,
                evidence_slots: region.evidence_slots,
            })
            .collect::<Vec<_>>();
        let boundaries = self
            .boundaries
            .iter()
            .map(|boundary| ExecutionBoundary {
                cord: Id(&boundary.cord),
                from_region: Id(&boundary.from_region),
                to_region: Id(&boundary.to_region),
                realization: boundary.realization.as_pin(),
                generation: boundary.generation,
                from_placement_generation: boundary.from_placement_generation,
                to_placement_generation: boundary.to_placement_generation,
                capacity_items: boundary.capacity_items,
                capacity_bytes: boundary.capacity_bytes,
                wake_slots: boundary.wake_slots,
                evidence_slots: boundary.evidence_slots,
            })
            .collect::<Vec<_>>();
        let commit_domains = self
            .commit_domains
            .iter()
            .map(|domain| ExecutionCommitDomain {
                id: Id(&domain.id),
                ordering: domain.ordering,
                proposal_slots: domain.proposal_slots,
                commit_slots: domain.commit_slots,
                maximum_proposal_bytes: domain.maximum_proposal_bytes,
                maximum_head_of_line_ticks: domain.maximum_head_of_line_ticks,
                cancellation_slots: domain.cancellation_slots,
                evidence_slots: domain.evidence_slots,
            })
            .collect::<Vec<_>>();
        let nodes = plan
            .nodes
            .iter()
            .map(|node| node.instance)
            .collect::<Vec<_>>();
        let cords = plan
            .cords
            .iter()
            .map(|cord| ExecutionLogicalCord {
                id: cord.id,
                from: cord.from.node,
                to: cord.to.node,
                capacity_items: cord.flow.capacity.items(),
                capacity_bytes: cord.flow.capacity.max_queued_bytes(),
            })
            .collect::<Vec<_>>();
        let hosts = plan
            .host_observations
            .iter()
            .map(|host| host.id)
            .collect::<Vec<_>>();
        validate_execution_arrangement(
            ExecutionArrangement {
                placements: &placements,
                lanes: &lanes,
                regions: &regions,
                boundaries: &boundaries,
                commit_domains: &commit_domains,
            },
            &nodes,
            &cords,
            &hosts,
        )
        .map_err(ExecutionArrangementReason::Contract)
    }
}

fn hash(digest: &mut Sha256, name: &[u8], value: &[u8]) {
    digest.update((name.len() as u64).to_be_bytes());
    digest.update(name);
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn hash_len(digest: &mut Sha256, name: &[u8], value: usize) {
    hash(
        digest,
        name,
        &u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes(),
    );
}

fn hash_descriptor(digest: &mut Sha256, name: &[u8], descriptor: &ResolvedExecutionDescriptor) {
    hash(digest, name, descriptor.id.as_bytes());
    hash(
        digest,
        b"descriptor-version",
        &descriptor.schema_version.to_be_bytes(),
    );
    hash(
        digest,
        b"descriptor-hash",
        descriptor.semantic_hash.as_bytes(),
    );
}
