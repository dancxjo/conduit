//! Host-neutral physical execution arrangement contracts.
//!
//! Logical nodes and cords remain semantic identities. Regions, placements,
//! lanes, boundary realizations, proposals, and commit domains describe the
//! finite physical arrangement selected for one exact plan epoch.

use core::convert::Infallible;
use core::fmt;

use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition, Id, InstancePath,
    MapField, PinnedDescriptor, SemanticHash,
};

/// Strength of one provider claim. Only `Guaranteed` may satisfy admission
/// that depends on the behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ExecutionGuarantee {
    Unsupported,
    Observed,
    Guaranteed,
}

impl ExecutionGuarantee {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Observed => "observed",
            Self::Guaranteed => "guaranteed",
        }
    }
}

/// Strongest complete containment profile provided by one placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum IsolationProfile {
    StepNative,
    IsolatedCooperative,
    IsolatedPreemptible,
    IsolatedTerminable,
}

impl IsolationProfile {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StepNative => "step-native",
            Self::IsolatedCooperative => "isolated-cooperative",
            Self::IsolatedPreemptible => "isolated-preemptible",
            Self::IsolatedTerminable => "isolated-terminable",
        }
    }
}

/// Deterministic publication policy for one commit domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitOrdering {
    /// One plan-derived ticket frontier. Physical completion cannot pass it.
    DeterministicFrontier,
    /// A structurally proven independent domain with its own deterministic
    /// frontier.
    IndependentFrontier,
}

impl CommitOrdering {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeterministicFrontier => "deterministic-frontier",
            Self::IndependentFrontier => "independent-frontier",
        }
    }
}

/// Independently owned authority/resource/lifecycle/failure boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionPlacement<'a> {
    pub id: Id<'a>,
    /// Fresh host observation authorizing no behavior by itself.
    pub host_observation: Id<'a>,
    pub provider: PinnedDescriptor<'a>,
    pub authority_boundary: PinnedDescriptor<'a>,
    pub resource_boundary: PinnedDescriptor<'a>,
    pub lifecycle_boundary: PinnedDescriptor<'a>,
    pub failure_boundary: PinnedDescriptor<'a>,
    pub generation: u64,
    pub isolation: IsolationProfile,
    pub memory_containment: ExecutionGuarantee,
    pub regain_control: ExecutionGuarantee,
    pub effect_fencing: ExecutionGuarantee,
    pub stop_execution: ExecutionGuarantee,
    pub reclaim_resources: ExecutionGuarantee,
    /// Finite bound required by preemptible/terminable profiles.
    pub maximum_regain_control_ticks: u64,
}

impl ExecutionPlacement<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let fields = [
            field("id", CanonicalValue::Identifier(self.id)),
            field(
                "host_observation",
                CanonicalValue::Identifier(self.host_observation),
            ),
            field("provider_id", CanonicalValue::Identifier(self.provider.id)),
            field(
                "provider_schema_version",
                CanonicalValue::Integer(i128::from(self.provider.schema_version)),
            ),
            field(
                "provider_semantic_hash",
                CanonicalValue::Bytes(self.provider.semantic_hash.as_bytes()),
            ),
            field(
                "authority_boundary_id",
                CanonicalValue::Identifier(self.authority_boundary.id),
            ),
            field(
                "authority_boundary_schema_version",
                CanonicalValue::Integer(i128::from(self.authority_boundary.schema_version)),
            ),
            field(
                "authority_boundary_semantic_hash",
                CanonicalValue::Bytes(self.authority_boundary.semantic_hash.as_bytes()),
            ),
            field(
                "resource_boundary_id",
                CanonicalValue::Identifier(self.resource_boundary.id),
            ),
            field(
                "resource_boundary_schema_version",
                CanonicalValue::Integer(i128::from(self.resource_boundary.schema_version)),
            ),
            field(
                "resource_boundary_semantic_hash",
                CanonicalValue::Bytes(self.resource_boundary.semantic_hash.as_bytes()),
            ),
            field(
                "lifecycle_boundary_id",
                CanonicalValue::Identifier(self.lifecycle_boundary.id),
            ),
            field(
                "lifecycle_boundary_schema_version",
                CanonicalValue::Integer(i128::from(self.lifecycle_boundary.schema_version)),
            ),
            field(
                "lifecycle_boundary_semantic_hash",
                CanonicalValue::Bytes(self.lifecycle_boundary.semantic_hash.as_bytes()),
            ),
            field(
                "failure_boundary_id",
                CanonicalValue::Identifier(self.failure_boundary.id),
            ),
            field(
                "failure_boundary_schema_version",
                CanonicalValue::Integer(i128::from(self.failure_boundary.schema_version)),
            ),
            field(
                "failure_boundary_semantic_hash",
                CanonicalValue::Bytes(self.failure_boundary.semantic_hash.as_bytes()),
            ),
            field(
                "generation",
                CanonicalValue::Integer(i128::from(self.generation)),
            ),
            field(
                "isolation",
                CanonicalValue::Identifier(Id(self.isolation.as_str())),
            ),
            field(
                "memory_containment",
                CanonicalValue::Identifier(Id(self.memory_containment.as_str())),
            ),
            field(
                "regain_control",
                CanonicalValue::Identifier(Id(self.regain_control.as_str())),
            ),
            field(
                "effect_fencing",
                CanonicalValue::Identifier(Id(self.effect_fencing.as_str())),
            ),
            field(
                "stop_execution",
                CanonicalValue::Identifier(Id(self.stop_execution.as_str())),
            ),
            field(
                "reclaim_resources",
                CanonicalValue::Identifier(Id(self.reclaim_resources.as_str())),
            ),
            field(
                "maximum_regain_control_ticks",
                CanonicalValue::Integer(i128::from(self.maximum_regain_control_ticks)),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/execution-placement"),
            schema_version: 0,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

/// One finite independently progressing physical resource inside a placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLane<'a> {
    pub id: Id<'a>,
    pub placement: Id<'a>,
    pub placement_generation: u64,
    /// Lane ownership generation, independent of placement generation.
    pub generation: u64,
    pub independent_progress: ExecutionGuarantee,
    pub simultaneous_execution: ExecutionGuarantee,
    pub preemption: ExecutionGuarantee,
    pub termination: ExecutionGuarantee,
    pub ready_slots: u16,
    pub wake_slots: u16,
    pub proposal_slots: u16,
    pub commit_slots: u16,
    pub timer_slots: u16,
    pub scratch_bytes: u32,
    pub stack_bytes: u32,
    pub evidence_slots: u32,
}

impl ExecutionLane<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let fields = [
            field("id", CanonicalValue::Identifier(self.id)),
            field("placement", CanonicalValue::Identifier(self.placement)),
            field(
                "placement_generation",
                CanonicalValue::Integer(i128::from(self.placement_generation)),
            ),
            field(
                "generation",
                CanonicalValue::Integer(i128::from(self.generation)),
            ),
            field(
                "independent_progress",
                CanonicalValue::Identifier(Id(self.independent_progress.as_str())),
            ),
            field(
                "simultaneous_execution",
                CanonicalValue::Identifier(Id(self.simultaneous_execution.as_str())),
            ),
            field(
                "preemption",
                CanonicalValue::Identifier(Id(self.preemption.as_str())),
            ),
            field(
                "termination",
                CanonicalValue::Identifier(Id(self.termination.as_str())),
            ),
            field(
                "ready_slots",
                CanonicalValue::Integer(i128::from(self.ready_slots)),
            ),
            field(
                "wake_slots",
                CanonicalValue::Integer(i128::from(self.wake_slots)),
            ),
            field(
                "proposal_slots",
                CanonicalValue::Integer(i128::from(self.proposal_slots)),
            ),
            field(
                "commit_slots",
                CanonicalValue::Integer(i128::from(self.commit_slots)),
            ),
            field(
                "timer_slots",
                CanonicalValue::Integer(i128::from(self.timer_slots)),
            ),
            field(
                "scratch_bytes",
                CanonicalValue::Integer(i128::from(self.scratch_bytes)),
            ),
            field(
                "stack_bytes",
                CanonicalValue::Integer(i128::from(self.stack_bytes)),
            ),
            field(
                "evidence_slots",
                CanonicalValue::Integer(i128::from(self.evidence_slots)),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/execution-lane"),
            schema_version: 0,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

/// One physical scheduling/placement unit. Member identity remains logical.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionRegion<'a> {
    pub id: Id<'a>,
    pub members: &'a [InstancePath<'a>],
    pub placement: Id<'a>,
    pub placement_generation: u64,
    pub lane: Id<'a>,
    pub lane_generation: u64,
    pub commit_domain: Id<'a>,
    /// Whether the compiler proved this region independent of other regions
    /// admitted on simultaneous lanes.
    pub independent: bool,
    pub maximum_in_flight_proposals: u16,
    pub scratch_bytes: u32,
    pub retained_state_bytes: u64,
    pub pending_operation_slots: u16,
    pub timer_slots: u16,
    pub evidence_slots: u32,
}

/// Finite physical realization of a logical cord crossing regions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionBoundary<'a> {
    pub cord: Id<'a>,
    pub from_region: Id<'a>,
    pub to_region: Id<'a>,
    pub realization: PinnedDescriptor<'a>,
    /// Boundary transport/mailbox generation.
    pub generation: u64,
    pub from_placement_generation: u64,
    pub to_placement_generation: u64,
    pub capacity_items: u16,
    pub capacity_bytes: u64,
    pub wake_slots: u16,
    pub evidence_slots: u32,
}

/// Finite staging and deterministic publication reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionCommitDomain<'a> {
    pub id: Id<'a>,
    pub ordering: CommitOrdering,
    pub proposal_slots: u16,
    pub commit_slots: u16,
    pub maximum_proposal_bytes: u64,
    pub maximum_head_of_line_ticks: u64,
    pub cancellation_slots: u16,
    pub evidence_slots: u32,
}

/// Complete physical arrangement for one exact plan epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionArrangement<'a> {
    pub placements: &'a [ExecutionPlacement<'a>],
    pub lanes: &'a [ExecutionLane<'a>],
    pub regions: &'a [ExecutionRegion<'a>],
    pub boundaries: &'a [ExecutionBoundary<'a>],
    pub commit_domains: &'a [ExecutionCommitDomain<'a>],
}

/// Logical cord identity used by arrangement validation without importing a
/// hosted runtime representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLogicalCord<'a> {
    pub id: Id<'a>,
    pub from: InstancePath<'a>,
    pub to: InstancePath<'a>,
    pub capacity_items: u16,
    pub capacity_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionContractError {
    EmptyArrangement,
    InvalidIdentity,
    DuplicateIdentity,
    DanglingReference,
    GenerationMismatch,
    InvalidGuarantee,
    UnboundedReservation,
    NodeMembership,
    BoundaryMismatch,
    CapacityExceeded,
    ArithmeticOverflow,
}

/// Plan-derived proposal ticket. Provider identity and completion time are
/// deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionProposalTicket<'a> {
    pub plan_epoch: u64,
    pub commit_domain: Id<'a>,
    pub sequence: u64,
}

/// Allocation-free deterministic commit-window accounting. Proposal bytes
/// remain in caller-owned, plan-reserved slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeterministicCommitFrontier<'a> {
    plan_epoch: u64,
    commit_domain: Id<'a>,
    next_dispatch: u64,
    next_commit: u64,
    in_flight: u16,
    window: u16,
}

impl<'a> DeterministicCommitFrontier<'a> {
    pub fn new(
        plan_epoch: u64,
        commit_domain: Id<'a>,
        first_sequence: u64,
        window: u16,
    ) -> Result<Self, ExecutionContractError> {
        if plan_epoch == 0 || !valid_id(commit_domain) || window == 0 {
            return Err(ExecutionContractError::UnboundedReservation);
        }
        Ok(Self {
            plan_epoch,
            commit_domain,
            next_dispatch: first_sequence,
            next_commit: first_sequence,
            in_flight: 0,
            window,
        })
    }

    #[must_use]
    pub const fn next_commit(&self) -> u64 {
        self.next_commit
    }

    #[must_use]
    pub const fn in_flight(&self) -> u16 {
        self.in_flight
    }

    /// Reserve exactly the next deterministic ticket before physical compute.
    pub fn dispatch(
        &mut self,
        ticket: ExecutionProposalTicket<'_>,
    ) -> Result<(), ExecutionContractError> {
        if ticket.plan_epoch != self.plan_epoch
            || ticket.commit_domain != self.commit_domain
            || ticket.sequence != self.next_dispatch
        {
            return Err(ExecutionContractError::GenerationMismatch);
        }
        if self.in_flight == self.window {
            return Err(ExecutionContractError::CapacityExceeded);
        }
        self.next_dispatch = self
            .next_dispatch
            .checked_add(1)
            .ok_or(ExecutionContractError::ArithmeticOverflow)?;
        self.in_flight += 1;
        Ok(())
    }

    /// Authorize publication of the head proposal only. A physically later
    /// proposal waits in its already reserved slot.
    pub fn commit_head(
        &mut self,
        ticket: ExecutionProposalTicket<'_>,
    ) -> Result<(), ExecutionContractError> {
        if ticket.plan_epoch != self.plan_epoch
            || ticket.commit_domain != self.commit_domain
            || ticket.sequence != self.next_commit
            || self.in_flight == 0
        {
            return Err(ExecutionContractError::GenerationMismatch);
        }
        self.next_commit = self
            .next_commit
            .checked_add(1)
            .ok_or(ExecutionContractError::ArithmeticOverflow)?;
        self.in_flight -= 1;
        Ok(())
    }

    /// Fence every uncommitted proposal on cancellation or plan transition.
    /// The returned count must be dispositioned by the caller exactly once.
    pub fn fence(&mut self, new_plan_epoch: u64) -> Result<u16, ExecutionContractError> {
        if new_plan_epoch <= self.plan_epoch {
            return Err(ExecutionContractError::GenerationMismatch);
        }
        let disposed = self.in_flight;
        self.plan_epoch = new_plan_epoch;
        self.next_dispatch = 0;
        self.next_commit = 0;
        self.in_flight = 0;
        Ok(disposed)
    }
}

impl ExecutionContractError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyArrangement => "CND-EXE-001",
            Self::InvalidIdentity => "CND-EXE-002",
            Self::DuplicateIdentity => "CND-EXE-003",
            Self::DanglingReference => "CND-EXE-004",
            Self::GenerationMismatch => "CND-EXE-005",
            Self::InvalidGuarantee => "CND-EXE-006",
            Self::UnboundedReservation => "CND-EXE-007",
            Self::NodeMembership => "CND-EXE-008",
            Self::BoundaryMismatch => "CND-EXE-009",
            Self::CapacityExceeded => "CND-EXE-010",
            Self::ArithmeticOverflow => "CND-EXE-011",
        }
    }
}

impl fmt::Display for ExecutionContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyArrangement => "execution arrangement is empty",
            Self::InvalidIdentity => "execution identity is invalid",
            Self::DuplicateIdentity => "execution identity is duplicated",
            Self::DanglingReference => "execution arrangement contains a dangling reference",
            Self::GenerationMismatch => "placement or lane generation does not match",
            Self::InvalidGuarantee => "provider guarantee does not support the selected profile",
            Self::UnboundedReservation => "execution reservation is zero or otherwise unbounded",
            Self::NodeMembership => "logical node membership is missing or duplicated",
            Self::BoundaryMismatch => "physical boundary does not match a crossing logical cord",
            Self::CapacityExceeded => "execution reservation exceeds its owning provider capacity",
            Self::ArithmeticOverflow => "execution reservation accounting overflowed",
        })
    }
}

fn valid_id(value: Id<'_>) -> bool {
    Id::new(value.as_str()).is_ok()
}

fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    valid_id(value.id)
        && value.schema_version == 0
        && value.semantic_hash != SemanticHash::from_bytes([0; 32])
}

/// Validate the independently observed placement and lane population in one
/// host capability report. An empty population is honest unsupported state.
pub fn validate_execution_provider_observation(
    placements: &[ExecutionPlacement<'_>],
    lanes: &[ExecutionLane<'_>],
    host_observation: Id<'_>,
) -> Result<(), ExecutionContractError> {
    for (index, placement) in placements.iter().enumerate() {
        if !valid_id(placement.id)
            || placement.host_observation != host_observation
            || !valid_pin(placement.provider)
            || !valid_pin(placement.authority_boundary)
            || !valid_pin(placement.resource_boundary)
            || !valid_pin(placement.lifecycle_boundary)
            || !valid_pin(placement.failure_boundary)
            || placement.generation == 0
            || placements[..index]
                .iter()
                .any(|prior| prior.id == placement.id)
        {
            return Err(ExecutionContractError::InvalidIdentity);
        }
        let valid_isolation = match placement.isolation {
            IsolationProfile::StepNative => true,
            IsolationProfile::IsolatedCooperative => {
                placement.memory_containment == ExecutionGuarantee::Guaranteed
            }
            IsolationProfile::IsolatedPreemptible => {
                placement.memory_containment == ExecutionGuarantee::Guaranteed
                    && placement.regain_control == ExecutionGuarantee::Guaranteed
                    && placement.maximum_regain_control_ticks > 0
            }
            IsolationProfile::IsolatedTerminable => {
                placement.memory_containment == ExecutionGuarantee::Guaranteed
                    && placement.regain_control == ExecutionGuarantee::Guaranteed
                    && placement.effect_fencing == ExecutionGuarantee::Guaranteed
                    && placement.stop_execution == ExecutionGuarantee::Guaranteed
                    && placement.reclaim_resources == ExecutionGuarantee::Guaranteed
                    && placement.maximum_regain_control_ticks > 0
            }
        };
        if !valid_isolation {
            return Err(ExecutionContractError::InvalidGuarantee);
        }
    }
    for (index, lane) in lanes.iter().enumerate() {
        let placement = placements
            .iter()
            .find(|placement| placement.id == lane.placement)
            .ok_or(ExecutionContractError::DanglingReference)?;
        if !valid_id(lane.id)
            || lane.generation == 0
            || lane.placement_generation != placement.generation
            || lanes[..index].iter().any(|prior| prior.id == lane.id)
        {
            return Err(ExecutionContractError::GenerationMismatch);
        }
        if lane.ready_slots == 0
            || lane.wake_slots == 0
            || lane.proposal_slots == 0
            || lane.commit_slots == 0
            || lane.scratch_bytes == 0
            || lane.stack_bytes == 0
            || lane.evidence_slots == 0
        {
            return Err(ExecutionContractError::UnboundedReservation);
        }
        if lane.simultaneous_execution == ExecutionGuarantee::Guaranteed
            && lane.independent_progress != ExecutionGuarantee::Guaranteed
        {
            return Err(ExecutionContractError::InvalidGuarantee);
        }
        if lane.preemption == ExecutionGuarantee::Guaranteed
            && placement.regain_control != ExecutionGuarantee::Guaranteed
        {
            return Err(ExecutionContractError::InvalidGuarantee);
        }
        if lane.termination == ExecutionGuarantee::Guaranteed
            && (placement.effect_fencing != ExecutionGuarantee::Guaranteed
                || placement.stop_execution != ExecutionGuarantee::Guaranteed)
        {
            return Err(ExecutionContractError::InvalidGuarantee);
        }
    }
    Ok(())
}

/// Validate one exact physical arrangement against the logical node/cord set
/// and the independently observed hosts it references.
pub fn validate_execution_arrangement(
    arrangement: ExecutionArrangement<'_>,
    nodes: &[InstancePath<'_>],
    cords: &[ExecutionLogicalCord<'_>],
    host_observations: &[Id<'_>],
) -> Result<(), ExecutionContractError> {
    if nodes.is_empty()
        || arrangement.placements.is_empty()
        || arrangement.lanes.is_empty()
        || arrangement.regions.is_empty()
        || arrangement.commit_domains.is_empty()
    {
        return Err(ExecutionContractError::EmptyArrangement);
    }

    for (index, placement) in arrangement.placements.iter().enumerate() {
        if !valid_id(placement.id)
            || !valid_id(placement.host_observation)
            || !valid_pin(placement.provider)
            || !valid_pin(placement.authority_boundary)
            || !valid_pin(placement.resource_boundary)
            || !valid_pin(placement.lifecycle_boundary)
            || !valid_pin(placement.failure_boundary)
            || placement.generation == 0
            || !host_observations.contains(&placement.host_observation)
        {
            return Err(ExecutionContractError::InvalidIdentity);
        }
        if arrangement.placements[..index]
            .iter()
            .any(|prior| prior.id == placement.id)
        {
            return Err(ExecutionContractError::DuplicateIdentity);
        }
        match placement.isolation {
            IsolationProfile::StepNative => {}
            IsolationProfile::IsolatedCooperative => {
                if placement.memory_containment != ExecutionGuarantee::Guaranteed {
                    return Err(ExecutionContractError::InvalidGuarantee);
                }
            }
            IsolationProfile::IsolatedPreemptible => {
                if placement.memory_containment != ExecutionGuarantee::Guaranteed
                    || placement.regain_control != ExecutionGuarantee::Guaranteed
                    || placement.maximum_regain_control_ticks == 0
                {
                    return Err(ExecutionContractError::InvalidGuarantee);
                }
            }
            IsolationProfile::IsolatedTerminable => {
                if placement.memory_containment != ExecutionGuarantee::Guaranteed
                    || placement.regain_control != ExecutionGuarantee::Guaranteed
                    || placement.effect_fencing != ExecutionGuarantee::Guaranteed
                    || placement.stop_execution != ExecutionGuarantee::Guaranteed
                    || placement.reclaim_resources != ExecutionGuarantee::Guaranteed
                    || placement.maximum_regain_control_ticks == 0
                {
                    return Err(ExecutionContractError::InvalidGuarantee);
                }
            }
        }
    }

    for (index, lane) in arrangement.lanes.iter().enumerate() {
        if !valid_id(lane.id)
            || !valid_id(lane.placement)
            || lane.placement_generation == 0
            || lane.generation == 0
        {
            return Err(ExecutionContractError::InvalidIdentity);
        }
        if arrangement.lanes[..index]
            .iter()
            .any(|prior| prior.id == lane.id)
        {
            return Err(ExecutionContractError::DuplicateIdentity);
        }
        let placement = arrangement
            .placements
            .iter()
            .find(|placement| placement.id == lane.placement)
            .ok_or(ExecutionContractError::DanglingReference)?;
        if placement.generation != lane.placement_generation {
            return Err(ExecutionContractError::GenerationMismatch);
        }
        if lane.ready_slots == 0
            || lane.wake_slots == 0
            || lane.proposal_slots == 0
            || lane.commit_slots == 0
            || lane.scratch_bytes == 0
            || lane.stack_bytes == 0
            || lane.evidence_slots == 0
        {
            return Err(ExecutionContractError::UnboundedReservation);
        }
        if lane.simultaneous_execution == ExecutionGuarantee::Guaranteed
            && lane.independent_progress != ExecutionGuarantee::Guaranteed
        {
            return Err(ExecutionContractError::InvalidGuarantee);
        }
        if lane.preemption == ExecutionGuarantee::Guaranteed
            && placement.regain_control != ExecutionGuarantee::Guaranteed
        {
            return Err(ExecutionContractError::InvalidGuarantee);
        }
        if lane.termination == ExecutionGuarantee::Guaranteed
            && (placement.effect_fencing != ExecutionGuarantee::Guaranteed
                || placement.stop_execution != ExecutionGuarantee::Guaranteed)
        {
            return Err(ExecutionContractError::InvalidGuarantee);
        }
    }

    for (index, domain) in arrangement.commit_domains.iter().enumerate() {
        if !valid_id(domain.id) {
            return Err(ExecutionContractError::InvalidIdentity);
        }
        if arrangement.commit_domains[..index]
            .iter()
            .any(|prior| prior.id == domain.id)
        {
            return Err(ExecutionContractError::DuplicateIdentity);
        }
        if domain.proposal_slots == 0
            || domain.commit_slots == 0
            || domain.maximum_proposal_bytes == 0
            || domain.maximum_head_of_line_ticks == 0
            || domain.cancellation_slots == 0
            || domain.evidence_slots == 0
        {
            return Err(ExecutionContractError::UnboundedReservation);
        }
    }

    for (index, region) in arrangement.regions.iter().enumerate() {
        if !valid_id(region.id)
            || !valid_id(region.placement)
            || !valid_id(region.lane)
            || !valid_id(region.commit_domain)
            || region.members.is_empty()
        {
            return Err(ExecutionContractError::InvalidIdentity);
        }
        if arrangement.regions[..index]
            .iter()
            .any(|prior| prior.id == region.id)
        {
            return Err(ExecutionContractError::DuplicateIdentity);
        }
        let lane = arrangement
            .lanes
            .iter()
            .find(|lane| lane.id == region.lane)
            .ok_or(ExecutionContractError::DanglingReference)?;
        let placement = arrangement
            .placements
            .iter()
            .find(|placement| placement.id == region.placement)
            .ok_or(ExecutionContractError::DanglingReference)?;
        if lane.placement != region.placement
            || placement.generation != region.placement_generation
            || lane.generation != region.lane_generation
            || !arrangement
                .commit_domains
                .iter()
                .any(|domain| domain.id == region.commit_domain)
        {
            return Err(ExecutionContractError::DanglingReference);
        }
        if region.independent
            && (lane.independent_progress != ExecutionGuarantee::Guaranteed
                || lane.simultaneous_execution != ExecutionGuarantee::Guaranteed)
        {
            return Err(ExecutionContractError::InvalidGuarantee);
        }
        if region.maximum_in_flight_proposals == 0
            || region.scratch_bytes == 0
            || region.evidence_slots == 0
            || region.maximum_in_flight_proposals > lane.proposal_slots
            || region.scratch_bytes > lane.scratch_bytes
            || region.timer_slots > lane.timer_slots
            || region.evidence_slots > lane.evidence_slots
        {
            return Err(ExecutionContractError::CapacityExceeded);
        }
        for (member_index, member) in region.members.iter().enumerate() {
            if !nodes.contains(member)
                || region.members[..member_index].contains(member)
                || arrangement.regions[..index]
                    .iter()
                    .any(|prior| prior.members.contains(member))
            {
                return Err(ExecutionContractError::NodeMembership);
            }
        }
    }
    if nodes.iter().any(|node| {
        arrangement
            .regions
            .iter()
            .filter(|region| region.members.contains(node))
            .count()
            != 1
    }) {
        return Err(ExecutionContractError::NodeMembership);
    }

    for (index, boundary) in arrangement.boundaries.iter().enumerate() {
        if !valid_id(boundary.cord)
            || !valid_id(boundary.from_region)
            || !valid_id(boundary.to_region)
            || !valid_pin(boundary.realization)
            || boundary.generation == 0
            || boundary.from_placement_generation == 0
            || boundary.to_placement_generation == 0
            || boundary.capacity_items == 0
            || boundary.capacity_bytes == 0
            || boundary.wake_slots == 0
            || boundary.evidence_slots == 0
        {
            return Err(ExecutionContractError::UnboundedReservation);
        }
        if arrangement.boundaries[..index]
            .iter()
            .any(|prior| prior.cord == boundary.cord)
        {
            return Err(ExecutionContractError::DuplicateIdentity);
        }
        let cord = cords
            .iter()
            .find(|cord| cord.id == boundary.cord)
            .ok_or(ExecutionContractError::DanglingReference)?;
        let from = region_for_node(arrangement.regions, cord.from)
            .ok_or(ExecutionContractError::NodeMembership)?;
        let to = region_for_node(arrangement.regions, cord.to)
            .ok_or(ExecutionContractError::NodeMembership)?;
        if from.id == to.id
            || from.id != boundary.from_region
            || to.id != boundary.to_region
            || boundary.capacity_items != cord.capacity_items
            || boundary.capacity_bytes != cord.capacity_bytes
        {
            return Err(ExecutionContractError::BoundaryMismatch);
        }
        let from_placement = arrangement
            .placements
            .iter()
            .find(|placement| placement.id == from.placement)
            .ok_or(ExecutionContractError::DanglingReference)?;
        let to_placement = arrangement
            .placements
            .iter()
            .find(|placement| placement.id == to.placement)
            .ok_or(ExecutionContractError::DanglingReference)?;
        if boundary.from_placement_generation != from_placement.generation
            || boundary.to_placement_generation != to_placement.generation
        {
            return Err(ExecutionContractError::GenerationMismatch);
        }
    }
    for cord in cords {
        let from = region_for_node(arrangement.regions, cord.from)
            .ok_or(ExecutionContractError::NodeMembership)?;
        let to = region_for_node(arrangement.regions, cord.to)
            .ok_or(ExecutionContractError::NodeMembership)?;
        let count = arrangement
            .boundaries
            .iter()
            .filter(|boundary| boundary.cord == cord.id)
            .count();
        if (from.id == to.id && count != 0) || (from.id != to.id && count != 1) {
            return Err(ExecutionContractError::BoundaryMismatch);
        }
    }

    for lane in arrangement.lanes {
        let mut ready = 0_u16;
        let mut proposals = 0_u16;
        let mut timers = 0_u16;
        let mut scratch = 0_u32;
        let mut evidence = 0_u32;
        for region in arrangement
            .regions
            .iter()
            .filter(|region| region.lane == lane.id)
        {
            ready = ready
                .checked_add(1)
                .ok_or(ExecutionContractError::ArithmeticOverflow)?;
            proposals = proposals
                .checked_add(region.maximum_in_flight_proposals)
                .ok_or(ExecutionContractError::ArithmeticOverflow)?;
            timers = timers
                .checked_add(region.timer_slots)
                .ok_or(ExecutionContractError::ArithmeticOverflow)?;
            scratch = scratch
                .checked_add(region.scratch_bytes)
                .ok_or(ExecutionContractError::ArithmeticOverflow)?;
            evidence = evidence
                .checked_add(region.evidence_slots)
                .ok_or(ExecutionContractError::ArithmeticOverflow)?;
        }
        if ready > lane.ready_slots
            || proposals > lane.proposal_slots
            || timers > lane.timer_slots
            || scratch > lane.scratch_bytes
            || evidence > lane.evidence_slots
        {
            return Err(ExecutionContractError::CapacityExceeded);
        }
    }

    for domain in arrangement.commit_domains {
        let proposals = arrangement
            .regions
            .iter()
            .filter(|region| region.commit_domain == domain.id)
            .try_fold(0_u16, |total, region| {
                total.checked_add(region.maximum_in_flight_proposals)
            })
            .ok_or(ExecutionContractError::ArithmeticOverflow)?;
        if proposals > domain.proposal_slots || proposals > domain.commit_slots {
            return Err(ExecutionContractError::CapacityExceeded);
        }
    }
    Ok(())
}

fn region_for_node<'a>(
    regions: &'a [ExecutionRegion<'a>],
    node: InstancePath<'_>,
) -> Option<&'a ExecutionRegion<'a>> {
    regions.iter().find(|region| region.members.contains(&node))
}

const fn field<'a>(key: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(key),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
