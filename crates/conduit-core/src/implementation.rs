//! Host-neutral, allocator-free node implementation and step contracts.

use core::convert::Infallible;
use core::fmt;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    AuthorityTime, CanonicalDescriptor, CanonicalError, CanonicalValue, Direction,
    FieldDisposition, Id, InstancePath, MapField, PinnedDescriptor, PlanResourceBudget,
    SemanticHash, TerminalClass, TypeContractRef,
};

/// Version of the execution-profile descriptor current by specification 022.
pub const EXECUTION_PROFILE_SCHEMA_VERSION: u32 = 0;

/// Strength of the complete dependency-stack bound claimed by an implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundednessProfile {
    /// Every implementation-controlled byte and operation has a hard ceiling.
    Hard,
    /// At least one dependency is only observed and cannot claim hard bounds.
    Observed,
}

impl BoundednessProfile {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Hard => "hard",
            Self::Observed => "observed",
        }
    }
}

/// Cancellation guarantee exposed by the complete implementation stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationGuarantee {
    Bounded,
    Cooperative,
    Unbounded,
}

impl CancellationGuarantee {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Bounded => "bounded",
            Self::Cooperative => "cooperative",
            Self::Unbounded => "unbounded",
        }
    }
}

/// Where one memory ceiling is enforced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryAccounting {
    ExecutorAllocated,
    BackendBounded,
    ExternallyBounded,
    ObservedOnly,
}

impl MemoryAccounting {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ExecutorAllocated => "executor-allocated",
            Self::BackendBounded => "backend-bounded",
            Self::ExternallyBounded => "externally-bounded",
            Self::ObservedOnly => "observed-only",
        }
    }
}

/// Non-overlapping implementation memory categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryCategory {
    Retained,
    StepScratch,
    PortTransactions,
    PendingOperations,
    HostServices,
    ForeignRuntime,
}

impl MemoryCategory {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Retained => "retained",
            Self::StepScratch => "step-scratch",
            Self::PortTransactions => "port-transactions",
            Self::PendingOperations => "pending-operations",
            Self::HostServices => "host-services",
            Self::ForeignRuntime => "foreign-runtime",
        }
    }
}

/// One exact, plan-visible memory charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryClaim {
    pub category: MemoryCategory,
    pub accounting: MemoryAccounting,
    pub bytes: u64,
}

/// Concrete value ownership at an implementation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipModel {
    Owned,
    Borrowed,
    SharedHandle,
    ExclusiveHandle,
}

impl OwnershipModel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::Borrowed => "borrowed",
            Self::SharedHandle => "shared-handle",
            Self::ExclusiveHandle => "exclusive-handle",
        }
    }

    const fn is_handle(self) -> bool {
        matches!(self, Self::SharedHandle | Self::ExclusiveHandle)
    }
}

/// Required end-of-ownership behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleDisposition {
    None,
    ExplicitDispose,
}

impl HandleDisposition {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ExplicitDispose => "explicit-dispose",
        }
    }
}

/// Exact semantic-to-concrete representation binding for one port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueRepresentation<'a> {
    pub direction: Direction,
    pub port: Id<'a>,
    pub semantic_type: TypeContractRef<'a>,
    pub representation: PinnedDescriptor<'a>,
    pub ownership: OwnershipModel,
    pub disposition: HandleDisposition,
    pub max_bytes: u32,
}

/// Every implementation-controlled ceiling for one planned node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    pub max_step_work: u32,
    pub max_retained_values: u16,
    pub max_retained_bytes: u64,
    pub max_scratch_bytes: u32,
    pub max_input_leases: u16,
    pub max_input_bytes: u64,
    pub max_output_reservations: u16,
    pub max_output_bytes: u64,
    pub max_transactions: u16,
    pub max_fragments_per_step: u16,
    pub max_pending_operations: u16,
    pub max_timers: u16,
    pub max_child_tasks: u16,
    pub max_host_buffer_bytes: u64,
    pub max_foreign_queue_items: u16,
    pub max_foreign_queue_bytes: u64,
    pub max_checkpoint_bytes: u64,
    pub implementation_memory_bytes: u64,
    pub cancellation_ticks: u64,
}

/// Versioned implementation execution profile embedded in an exact plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionProfile<'a> {
    pub id: Id<'a>,
    pub schema_version: u32,
    pub semantic_hash: SemanticHash,
    pub boundedness: BoundednessProfile,
    pub cancellation: CancellationGuarantee,
    pub step_bound_enforced: bool,
    pub limits: ExecutionLimits,
    pub representations: &'a [ValueRepresentation<'a>],
    pub memory_claims: &'a [MemoryClaim],
    pub checkpoint: Option<PinnedDescriptor<'a>>,
}

impl ExecutionProfile<'_> {
    /// Number of caller-owned hash slots required to identify this profile.
    pub const fn identity_fact_count(&self) -> usize {
        self.representations.len() + self.memory_claims.len()
    }

    /// Compute the exact profile identity independently of collection order.
    pub fn computed_semantic_hash(
        &self,
        fact_hashes: &mut [SemanticHash],
    ) -> Result<SemanticHash, ProfileIdentityError> {
        let needed = self.identity_fact_count();
        if fact_hashes.len() < needed {
            return Err(ProfileIdentityError::ScratchTooSmall);
        }
        let mut cursor = 0;
        for representation in self.representations {
            fact_hashes[cursor] = hash_representation(*representation)?;
            cursor += 1;
        }
        for claim in self.memory_claims {
            fact_hashes[cursor] = hash_memory_claim(*claim)?;
            cursor += 1;
        }
        let limits = self.limits;
        let checkpoint = self.checkpoint.as_ref();
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic(
                "boundedness",
                CanonicalValue::Identifier(Id(self.boundedness.as_str())),
            ),
            semantic(
                "cancellation",
                CanonicalValue::Identifier(Id(self.cancellation.as_str())),
            ),
            semantic(
                "step_bound_enforced",
                CanonicalValue::Boolean(self.step_bound_enforced),
            ),
            semantic(
                "max_step_work",
                CanonicalValue::Integer(i128::from(limits.max_step_work)),
            ),
            semantic(
                "max_retained_values",
                CanonicalValue::Integer(i128::from(limits.max_retained_values)),
            ),
            semantic(
                "max_retained_bytes",
                CanonicalValue::Integer(i128::from(limits.max_retained_bytes)),
            ),
            semantic(
                "max_scratch_bytes",
                CanonicalValue::Integer(i128::from(limits.max_scratch_bytes)),
            ),
            semantic(
                "max_input_leases",
                CanonicalValue::Integer(i128::from(limits.max_input_leases)),
            ),
            semantic(
                "max_input_bytes",
                CanonicalValue::Integer(i128::from(limits.max_input_bytes)),
            ),
            semantic(
                "max_output_reservations",
                CanonicalValue::Integer(i128::from(limits.max_output_reservations)),
            ),
            semantic(
                "max_output_bytes",
                CanonicalValue::Integer(i128::from(limits.max_output_bytes)),
            ),
            semantic(
                "max_transactions",
                CanonicalValue::Integer(i128::from(limits.max_transactions)),
            ),
            semantic(
                "max_fragments_per_step",
                CanonicalValue::Integer(i128::from(limits.max_fragments_per_step)),
            ),
            semantic(
                "max_pending_operations",
                CanonicalValue::Integer(i128::from(limits.max_pending_operations)),
            ),
            semantic(
                "max_timers",
                CanonicalValue::Integer(i128::from(limits.max_timers)),
            ),
            semantic(
                "max_child_tasks",
                CanonicalValue::Integer(i128::from(limits.max_child_tasks)),
            ),
            semantic(
                "max_host_buffer_bytes",
                CanonicalValue::Integer(i128::from(limits.max_host_buffer_bytes)),
            ),
            semantic(
                "max_foreign_queue_items",
                CanonicalValue::Integer(i128::from(limits.max_foreign_queue_items)),
            ),
            semantic(
                "max_foreign_queue_bytes",
                CanonicalValue::Integer(i128::from(limits.max_foreign_queue_bytes)),
            ),
            semantic(
                "max_checkpoint_bytes",
                CanonicalValue::Integer(i128::from(limits.max_checkpoint_bytes)),
            ),
            semantic(
                "implementation_memory_bytes",
                CanonicalValue::Integer(i128::from(limits.implementation_memory_bytes)),
            ),
            semantic(
                "cancellation_ticks",
                CanonicalValue::Integer(i128::from(limits.cancellation_ticks)),
            ),
            semantic(
                "checkpoint_present",
                CanonicalValue::Boolean(checkpoint.is_some()),
            ),
            semantic(
                "checkpoint_id",
                checkpoint.map_or(CanonicalValue::Null, |pin| {
                    CanonicalValue::Identifier(pin.id)
                }),
            ),
            semantic(
                "checkpoint_version",
                checkpoint.map_or(CanonicalValue::Null, |pin| {
                    CanonicalValue::Integer(i128::from(pin.schema_version))
                }),
            ),
            semantic(
                "checkpoint_hash",
                checkpoint.map_or(CanonicalValue::Null, |pin| {
                    CanonicalValue::Bytes(pin.semantic_hash.as_bytes())
                }),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/implementation-execution-profile"),
            self.schema_version,
            &fields,
            Id("facts"),
            &fact_hashes[..needed],
        )
        .map_err(ProfileIdentityError::Canonical)
    }

    /// Validate descriptor shape, bounds, representations, and exact identity.
    pub fn validate(&self, fact_hashes: &mut [SemanticHash]) -> Result<(), ImplementationError> {
        if self.schema_version != EXECUTION_PROFILE_SCHEMA_VERSION
            || Id::new(self.id.as_str()).is_err()
            || self.limits.max_step_work == 0
            || self.limits.max_transactions == 0
            || self.limits.cancellation_ticks == 0
            || (self.boundedness == BoundednessProfile::Hard
                && (self.cancellation != CancellationGuarantee::Bounded
                    || !self.step_bound_enforced))
        {
            return Err(ImplementationError::InvalidProfile);
        }
        if self.checkpoint.is_some_and(|pin| !valid_pin(pin))
            || (self.checkpoint.is_some() && self.limits.max_checkpoint_bytes == 0)
            || (self.checkpoint.is_none() && self.limits.max_checkpoint_bytes != 0)
        {
            return Err(ImplementationError::InvalidProfile);
        }
        for (index, representation) in self.representations.iter().enumerate() {
            if Id::new(representation.port.as_str()).is_err()
                || representation.semantic_type.validate().is_err()
                || !valid_pin(representation.representation)
                || representation.max_bytes == 0
                || (representation.ownership.is_handle()
                    != (representation.disposition == HandleDisposition::ExplicitDispose))
                || self.representations[..index].iter().any(|prior| {
                    prior.direction == representation.direction && prior.port == representation.port
                })
            {
                return Err(ImplementationError::InvalidProfile);
            }
        }
        let mut claimed = 0_u64;
        for (index, claim) in self.memory_claims.iter().enumerate() {
            if claim.bytes == 0
                || (self.boundedness == BoundednessProfile::Hard
                    && claim.accounting == MemoryAccounting::ObservedOnly)
                || self.memory_claims[..index]
                    .iter()
                    .any(|prior| prior.category == claim.category)
            {
                return Err(ImplementationError::InvalidProfile);
            }
            claimed = claimed
                .checked_add(claim.bytes)
                .ok_or(ImplementationError::InvalidProfile)?;
        }
        if claimed != self.limits.implementation_memory_bytes {
            return Err(ImplementationError::InvalidProfile);
        }
        let has_claim = |category| {
            self.memory_claims
                .iter()
                .any(|claim| claim.category == category)
        };
        if ((self.limits.max_retained_values > 0 || self.limits.max_retained_bytes > 0)
            && !has_claim(MemoryCategory::Retained))
            || (self.limits.max_scratch_bytes > 0 && !has_claim(MemoryCategory::StepScratch))
            || ((self.limits.max_input_leases > 0
                || self.limits.max_input_bytes > 0
                || self.limits.max_output_reservations > 0
                || self.limits.max_output_bytes > 0
                || self.limits.max_transactions > 1
                || self.limits.max_fragments_per_step > 0)
                && !has_claim(MemoryCategory::PortTransactions))
            || ((self.limits.max_pending_operations > 0
                || self.limits.max_timers > 0
                || self.limits.max_child_tasks > 0)
                && !has_claim(MemoryCategory::PendingOperations))
            || (self.limits.max_host_buffer_bytes > 0 && !has_claim(MemoryCategory::HostServices))
            || ((self.limits.max_foreign_queue_items > 0
                || self.limits.max_foreign_queue_bytes > 0)
                && !has_claim(MemoryCategory::ForeignRuntime))
        {
            return Err(ImplementationError::InvalidProfile);
        }
        let bounded_components = [
            self.limits.max_retained_bytes,
            u64::from(self.limits.max_scratch_bytes),
            self.limits.max_input_bytes,
            self.limits.max_output_bytes,
            self.limits.max_host_buffer_bytes,
            self.limits.max_foreign_queue_bytes,
            self.limits.max_checkpoint_bytes,
        ]
        .into_iter()
        .try_fold(0_u64, u64::checked_add)
        .ok_or(ImplementationError::InvalidProfile)?;
        if bounded_components > self.limits.implementation_memory_bytes {
            return Err(ImplementationError::InvalidProfile);
        }
        let identity = self
            .computed_semantic_hash(fact_hashes)
            .map_err(|_| ImplementationError::InvalidProfile)?;
        if identity != self.semantic_hash {
            return Err(ImplementationError::ProfileIdentityMismatch);
        }
        Ok(())
    }
}

/// Validate one profile against its exact node allocation.
pub fn validate_plan_execution_profile(
    profile: &ExecutionProfile<'_>,
    allocation: PlanResourceBudget,
    fact_hashes: &mut [SemanticHash],
) -> Result<(), ImplementationError> {
    profile.validate(fact_hashes)?;
    if profile.limits.implementation_memory_bytes > allocation.memory_bytes
        || profile.limits.max_timers > allocation.timers
        || u16::from(profile.checkpoint.is_some()) > allocation.checkpoints
    {
        return Err(ImplementationError::PlanBudgetExceeded);
    }
    Ok(())
}

/// Exact inputs supplied by the executor when instantiating one selected node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstantiationContext<'a> {
    pub instance: InstancePath<'a>,
    pub implementation: PinnedDescriptor<'a>,
    pub artifact: Id<'a>,
    pub execution_profile_hash: SemanticHash,
    pub configuration_validated: bool,
    pub caller_memory_bytes: u64,
    pub required_resource_bindings: &'a [Id<'a>],
    pub provided_resource_bindings: &'a [Id<'a>],
    pub required_grants: &'a [Id<'a>],
    pub provided_grants: &'a [Id<'a>],
    pub cancellation_scope: Id<'a>,
}

/// Validate that instantiation uses only the exact prepared plan binding.
pub fn validate_instantiation(
    profile: &ExecutionProfile<'_>,
    context: InstantiationContext<'_>,
) -> Result<(), ImplementationError> {
    let caller_memory = profile
        .memory_claims
        .iter()
        .filter(|claim| claim.accounting == MemoryAccounting::ExecutorAllocated)
        .try_fold(0_u64, |total, claim| total.checked_add(claim.bytes))
        .ok_or(ImplementationError::InstantiationViolation)?;
    if InstancePath::new(context.instance.as_str()).is_err()
        || !valid_pin(context.implementation)
        || Id::new(context.artifact.as_str()).is_err()
        || Id::new(context.cancellation_scope.as_str()).is_err()
        || context.execution_profile_hash != profile.semantic_hash
        || !context.configuration_validated
        || context.caller_memory_bytes != caller_memory
        || !same_exact_ids(
            context.required_resource_bindings,
            context.provided_resource_bindings,
        )
        || !same_exact_ids(context.required_grants, context.provided_grants)
    {
        return Err(ImplementationError::InstantiationViolation);
    }
    Ok(())
}

fn same_exact_ids(required: &[Id<'_>], provided: &[Id<'_>]) -> bool {
    required.len() == provided.len()
        && required.iter().enumerate().all(|(index, id)| {
            Id::new(id.as_str()).is_ok() && !required[..index].contains(id) && provided.contains(id)
        })
        && provided.iter().enumerate().all(|(index, id)| {
            Id::new(id.as_str()).is_ok() && !provided[..index].contains(id) && required.contains(id)
        })
}

/// Runtime phase of one already selected implementation instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstancePhase {
    Instantiated,
    Prepared,
    Started,
    Draining,
    Cancelling,
    Terminal(TerminalClass),
}

/// Result reported while preparing an instance without starting effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrepareOutcome<'a> {
    Ready,
    Failed { code: Id<'a> },
}

/// Executor-measured prepare/start work. These phases cannot leave pending work.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LifecycleUsage {
    pub work_units: u32,
    pub scratch_bytes: u32,
    pub pending_operations: u16,
}

/// Atomically move all instances to prepared, or leave every phase unchanged.
pub fn prepare_all(
    machines: &mut [ImplementationMachine<'_>],
    outcomes: &[PrepareOutcome<'_>],
    usages: &[LifecycleUsage],
) -> Result<(), ImplementationError> {
    if machines.len() != outcomes.len()
        || machines.len() != usages.len()
        || machines
            .iter()
            .any(|machine| machine.phase != InstancePhase::Instantiated)
        || outcomes
            .iter()
            .any(|outcome| matches!(outcome, PrepareOutcome::Failed { .. }))
        || machines
            .iter()
            .zip(usages)
            .any(|(machine, usage)| validate_lifecycle_usage(machine.profile, *usage).is_err())
    {
        return Err(ImplementationError::PrepareFailed);
    }
    for machine in machines {
        machine.phase = InstancePhase::Prepared;
    }
    Ok(())
}

/// Start a completely prepared set; partial start is impossible.
pub fn start_all(
    machines: &mut [ImplementationMachine<'_>],
    usages: &[LifecycleUsage],
) -> Result<(), ImplementationError> {
    if machines.len() != usages.len()
        || machines
            .iter()
            .any(|machine| machine.phase != InstancePhase::Prepared)
        || machines
            .iter()
            .zip(usages)
            .any(|(machine, usage)| validate_lifecycle_usage(machine.profile, *usage).is_err())
    {
        return Err(ImplementationError::IllegalLifecycle);
    }
    for machine in machines {
        machine.phase = InstancePhase::Started;
    }
    Ok(())
}

fn validate_lifecycle_usage(
    profile: &ExecutionProfile<'_>,
    usage: LifecycleUsage,
) -> Result<(), ImplementationError> {
    if usage.work_units > profile.limits.max_step_work
        || usage.scratch_bytes > profile.limits.max_scratch_bytes
        || usage.pending_operations != 0
    {
        return Err(ImplementationError::StepBoundExceeded);
    }
    Ok(())
}

/// Exact change that may wake a pending implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeInterestKind {
    Input,
    Output,
    Timer,
    HostOperation,
    Cancellation,
}

/// Named, finite wake interest. There is no anonymous "poll later".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WakeInterest<'a> {
    pub kind: WakeInterestKind,
    pub subject: Id<'a>,
}

/// Executor-accounted work and storage observed during one step.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StepUsage {
    pub work_units: u32,
    pub observable_operations: u16,
    pub committed_transactions: u16,
    pub retained_values: u16,
    pub retained_bytes: u64,
    pub scratch_bytes: u32,
    pub input_leases: u16,
    pub input_bytes: u64,
    pub output_reservations: u16,
    pub output_bytes: u64,
    pub pending_operations: u16,
    pub timers: u16,
    pub child_tasks: u16,
    pub host_buffer_bytes: u64,
    pub foreign_queue_items: u16,
    pub foreign_queue_bytes: u64,
    pub fragments: u16,
    /// Domain evidence requests are not executor-observed progress.
    pub domain_evidence: u16,
}

/// Result of one nonblocking bounded step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepOutcome<'a> {
    Progress,
    Pending(&'a [WakeInterest<'a>]),
    Yielded,
    Completed,
    Failed { code: Id<'a> },
}

/// Stable outcome retained by executor-owned evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepOutcomeKind {
    Progress,
    Pending,
    Yielded,
    Completed,
    Failed,
}

/// Executor-created observation. Implementation domain evidence is separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepObservation {
    sequence: u64,
    outcome: StepOutcomeKind,
    observable_operations: u16,
    domain_evidence: u16,
}

impl StepObservation {
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub const fn outcome(self) -> StepOutcomeKind {
        self.outcome
    }

    #[must_use]
    pub const fn observable_operations(self) -> u16 {
        self.observable_operations
    }

    #[must_use]
    pub const fn domain_evidence(self) -> u16 {
        self.domain_evidence
    }
}

/// Lifecycle and step validator shared by every concrete binding.
#[derive(Clone, Copy, Debug)]
pub struct ImplementationMachine<'a> {
    profile: &'a ExecutionProfile<'a>,
    phase: InstancePhase,
    next_sequence: u64,
}

impl<'a> ImplementationMachine<'a> {
    pub fn instantiate(
        profile: &'a ExecutionProfile<'a>,
        context: InstantiationContext<'_>,
    ) -> Result<Self, ImplementationError> {
        validate_instantiation(profile, context)?;
        Ok(Self {
            profile,
            phase: InstancePhase::Instantiated,
            next_sequence: 0,
        })
    }

    #[must_use]
    pub const fn phase(self) -> InstancePhase {
        self.phase
    }

    /// Exact plan-pinned profile enforced by this machine.
    #[must_use]
    pub const fn profile(self) -> &'a ExecutionProfile<'a> {
        self.profile
    }

    pub fn drain(&mut self) -> Result<(), ImplementationError> {
        self.transition(InstancePhase::Started, InstancePhase::Draining)
    }

    pub fn cancel(&mut self) -> Result<(), ImplementationError> {
        if !matches!(
            self.phase,
            InstancePhase::Instantiated
                | InstancePhase::Prepared
                | InstancePhase::Started
                | InstancePhase::Draining
        ) {
            return Err(ImplementationError::IllegalLifecycle);
        }
        self.phase = InstancePhase::Cancelling;
        Ok(())
    }

    pub fn abort(&mut self) -> Result<(), ImplementationError> {
        if matches!(self.phase, InstancePhase::Terminal(_)) {
            return Err(ImplementationError::IllegalLifecycle);
        }
        self.phase = InstancePhase::Terminal(TerminalClass::Cancelled);
        Ok(())
    }

    /// Validate one step and create executor-owned evidence facts.
    pub fn observe_step(
        &mut self,
        outcome: StepOutcome<'_>,
        usage: StepUsage,
    ) -> Result<StepObservation, ImplementationError> {
        if !matches!(
            self.phase,
            InstancePhase::Started | InstancePhase::Draining | InstancePhase::Cancelling
        ) {
            return Err(ImplementationError::IllegalLifecycle);
        }
        validate_usage(self.profile.limits, usage)?;
        let kind = match outcome {
            StepOutcome::Progress => {
                if usage.observable_operations == 0 {
                    return Err(ImplementationError::FalseProgress);
                }
                StepOutcomeKind::Progress
            }
            StepOutcome::Pending(interests) => {
                if usage.observable_operations != 0 || interests.is_empty() {
                    return Err(ImplementationError::UnqualifiedPending);
                }
                validate_interests(interests, self.profile.limits)?;
                StepOutcomeKind::Pending
            }
            StepOutcome::Yielded => {
                if usage.work_units != self.profile.limits.max_step_work {
                    return Err(ImplementationError::FalseProgress);
                }
                StepOutcomeKind::Yielded
            }
            StepOutcome::Completed => {
                self.phase = InstancePhase::Terminal(if self.phase == InstancePhase::Cancelling {
                    TerminalClass::Cancelled
                } else {
                    TerminalClass::Succeeded
                });
                StepOutcomeKind::Completed
            }
            StepOutcome::Failed { code } => {
                if Id::new(code.as_str()).is_err() {
                    return Err(ImplementationError::InvalidProfile);
                }
                self.phase = InstancePhase::Terminal(TerminalClass::Failed);
                StepOutcomeKind::Failed
            }
        };
        let sequence = self.next_sequence;
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or(ImplementationError::StepBoundExceeded)?;
        Ok(StepObservation {
            sequence,
            outcome: kind,
            observable_operations: usage.observable_operations,
            domain_evidence: usage.domain_evidence,
        })
    }

    fn transition(
        &mut self,
        from: InstancePhase,
        to: InstancePhase,
    ) -> Result<(), ImplementationError> {
        if self.phase != from {
            return Err(ImplementationError::IllegalLifecycle);
        }
        self.phase = to;
        Ok(())
    }
}

fn validate_usage(limits: ExecutionLimits, usage: StepUsage) -> Result<(), ImplementationError> {
    if usage.work_units > limits.max_step_work
        || usage.committed_transactions > limits.max_transactions
        || usage.retained_values > limits.max_retained_values
        || usage.retained_bytes > limits.max_retained_bytes
        || usage.scratch_bytes > limits.max_scratch_bytes
        || usage.input_leases > limits.max_input_leases
        || usage.input_bytes > limits.max_input_bytes
        || usage.output_reservations > limits.max_output_reservations
        || usage.output_bytes > limits.max_output_bytes
        || usage.pending_operations > limits.max_pending_operations
        || usage.timers > limits.max_timers
        || usage.child_tasks > limits.max_child_tasks
        || usage.host_buffer_bytes > limits.max_host_buffer_bytes
        || usage.foreign_queue_items > limits.max_foreign_queue_items
        || usage.foreign_queue_bytes > limits.max_foreign_queue_bytes
        || usage.fragments > limits.max_fragments_per_step
    {
        return Err(ImplementationError::StepBoundExceeded);
    }
    Ok(())
}

fn validate_interests(
    interests: &[WakeInterest<'_>],
    limits: ExecutionLimits,
) -> Result<(), ImplementationError> {
    let maximum = usize::from(limits.max_input_leases)
        .checked_add(usize::from(limits.max_output_reservations))
        .and_then(|value| value.checked_add(usize::from(limits.max_pending_operations)))
        .and_then(|value| value.checked_add(usize::from(limits.max_timers)))
        .and_then(|value| value.checked_add(1))
        .ok_or(ImplementationError::StepBoundExceeded)?;
    if interests.len() > maximum {
        return Err(ImplementationError::StepBoundExceeded);
    }
    for (index, interest) in interests.iter().enumerate() {
        if Id::new(interest.subject.as_str()).is_err() || interests[..index].contains(interest) {
            return Err(ImplementationError::UnqualifiedPending);
        }
    }
    Ok(())
}

/// Whether a transaction publishes every reserved output together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationMode {
    Atomic,
    Independent,
}

/// State of one executor-mediated local port transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionState {
    Open,
    Committed,
    RolledBack,
}

/// Exact ownership result from commit or rollback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransactionResolution {
    pub consumed_inputs: u16,
    pub published_outputs: u16,
    pub published_bytes: u64,
}

/// Counter-based reference transaction over executor-owned leases/reservations.
#[derive(Clone, Copy, Debug)]
pub struct PortTransaction<'a> {
    profile: &'a ExecutionProfile<'a>,
    state: TransactionState,
    input_leases: u16,
    input_bytes: u64,
    output_reservations: u16,
    output_bytes: u64,
    produced_bytes: u64,
    fragments: u16,
}

impl<'a> PortTransaction<'a> {
    #[must_use]
    pub const fn new(profile: &'a ExecutionProfile<'a>) -> Self {
        Self {
            profile,
            state: TransactionState::Open,
            input_leases: 0,
            input_bytes: 0,
            output_reservations: 0,
            output_bytes: 0,
            produced_bytes: 0,
            fragments: 0,
        }
    }

    #[must_use]
    pub const fn state(self) -> TransactionState {
        self.state
    }

    pub fn lease_input(
        &mut self,
        representation: ValueRepresentation<'a>,
        bytes: u32,
    ) -> Result<(), ImplementationError> {
        self.require_open()?;
        if representation.direction != Direction::Input
            || !self.profile.representations.contains(&representation)
            || bytes == 0
            || bytes > representation.max_bytes
        {
            return Err(ImplementationError::TransactionViolation);
        }
        let leases = self
            .input_leases
            .checked_add(1)
            .ok_or(ImplementationError::TransactionViolation)?;
        let total = self
            .input_bytes
            .checked_add(u64::from(bytes))
            .ok_or(ImplementationError::TransactionViolation)?;
        if leases > self.profile.limits.max_input_leases
            || total > self.profile.limits.max_input_bytes
        {
            return Err(ImplementationError::TransactionViolation);
        }
        self.input_leases = leases;
        self.input_bytes = total;
        Ok(())
    }

    pub fn reserve_output(
        &mut self,
        representation: ValueRepresentation<'a>,
        bytes: u32,
    ) -> Result<(), ImplementationError> {
        self.require_open()?;
        if representation.direction != Direction::Output
            || !self.profile.representations.contains(&representation)
            || bytes == 0
            || bytes > representation.max_bytes
        {
            return Err(ImplementationError::TransactionViolation);
        }
        let reservations = self
            .output_reservations
            .checked_add(1)
            .ok_or(ImplementationError::TransactionViolation)?;
        let total = self
            .output_bytes
            .checked_add(u64::from(bytes))
            .ok_or(ImplementationError::TransactionViolation)?;
        if reservations > self.profile.limits.max_output_reservations
            || total > self.profile.limits.max_output_bytes
        {
            return Err(ImplementationError::TransactionViolation);
        }
        self.output_reservations = reservations;
        self.output_bytes = total;
        Ok(())
    }

    pub fn write_fragment(&mut self, bytes: u32) -> Result<(), ImplementationError> {
        self.require_open()?;
        let fragments = self
            .fragments
            .checked_add(1)
            .ok_or(ImplementationError::TransactionViolation)?;
        let produced = self
            .produced_bytes
            .checked_add(u64::from(bytes))
            .ok_or(ImplementationError::TransactionViolation)?;
        if bytes == 0
            || fragments > self.profile.limits.max_fragments_per_step
            || produced > self.output_bytes
        {
            return Err(ImplementationError::TransactionViolation);
        }
        self.fragments = fragments;
        self.produced_bytes = produced;
        Ok(())
    }

    pub fn commit(
        &mut self,
        mode: PublicationMode,
    ) -> Result<TransactionResolution, ImplementationError> {
        self.require_open()?;
        if self.input_leases == 0 && self.output_reservations == 0 {
            return Err(ImplementationError::TransactionViolation);
        }
        if mode == PublicationMode::Atomic
            && self.output_reservations > 0
            && self.produced_bytes == 0
        {
            return Err(ImplementationError::TransactionViolation);
        }
        if mode == PublicationMode::Independent && self.output_reservations > 1 {
            return Err(ImplementationError::TransactionViolation);
        }
        self.state = TransactionState::Committed;
        Ok(TransactionResolution {
            consumed_inputs: self.input_leases,
            published_outputs: self.output_reservations,
            published_bytes: self.produced_bytes,
        })
    }

    pub fn rollback(&mut self) -> Result<TransactionResolution, ImplementationError> {
        self.require_open()?;
        self.state = TransactionState::RolledBack;
        Ok(TransactionResolution {
            consumed_inputs: 0,
            published_outputs: 0,
            published_bytes: 0,
        })
    }

    fn require_open(self) -> Result<(), ImplementationError> {
        if self.state == TransactionState::Open {
            Ok(())
        } else {
            Err(ImplementationError::TransactionViolation)
        }
    }
}

/// Exact host-service operation request. It carries no ambient lookup path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostOperationRequest<'a> {
    pub operation: Id<'a>,
    pub resource_binding: Id<'a>,
    pub grant: Id<'a>,
    pub deadline: AuthorityTime<'a>,
    pub cancellation_scope: Id<'a>,
    pub buffer_bytes: u64,
    pub correlation: Id<'a>,
}

/// Exact plan/run context used to validate a host operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostOperationContext<'a> {
    pub required_resource_bindings: &'a [Id<'a>],
    pub grant_ids: &'a [Id<'a>],
    pub now: AuthorityTime<'a>,
}

/// Validate bindings, authority, deadline, cancellation, and budget.
pub fn validate_host_operation(
    request: HostOperationRequest<'_>,
    context: HostOperationContext<'_>,
    profile: &ExecutionProfile<'_>,
) -> Result<(), ImplementationError> {
    let latest = context
        .now
        .tick
        .checked_add(profile.limits.cancellation_ticks)
        .ok_or(ImplementationError::HostOperationViolation)?;
    if Id::new(request.operation.as_str()).is_err()
        || Id::new(request.resource_binding.as_str()).is_err()
        || Id::new(request.grant.as_str()).is_err()
        || Id::new(request.cancellation_scope.as_str()).is_err()
        || Id::new(request.correlation.as_str()).is_err()
        || !context
            .required_resource_bindings
            .contains(&request.resource_binding)
        || !context.grant_ids.contains(&request.grant)
        || request.deadline.basis != context.now.basis
        || request.deadline.tick <= context.now.tick
        || request.deadline.tick > latest
        || request.buffer_bytes > profile.limits.max_host_buffer_bytes
        || profile.limits.max_pending_operations == 0
    {
        return Err(ImplementationError::HostOperationViolation);
    }
    Ok(())
}

/// Optional state-export request; portability is never presumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointRequest<'a> {
    pub contract: PinnedDescriptor<'a>,
    pub maximum_bytes: u64,
}

impl ExecutionProfile<'_> {
    pub fn validate_checkpoint(
        &self,
        request: CheckpointRequest<'_>,
    ) -> Result<(), ImplementationError> {
        if self.checkpoint != Some(request.contract)
            || request.maximum_bytes == 0
            || request.maximum_bytes > self.limits.max_checkpoint_bytes
        {
            return Err(ImplementationError::UnsupportedCheckpoint);
        }
        Ok(())
    }
}

/// Stable implementation-contract violation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImplementationError {
    InvalidProfile,
    ProfileIdentityMismatch,
    PlanBudgetExceeded,
    IllegalLifecycle,
    StepBoundExceeded,
    FalseProgress,
    UnqualifiedPending,
    TransactionViolation,
    HostOperationViolation,
    UnsupportedCheckpoint,
    PrepareFailed,
    InstantiationViolation,
}

impl ImplementationError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidProfile => "CND-IMP-001",
            Self::ProfileIdentityMismatch => "CND-IMP-002",
            Self::PlanBudgetExceeded => "CND-IMP-003",
            Self::IllegalLifecycle => "CND-IMP-004",
            Self::StepBoundExceeded => "CND-IMP-005",
            Self::FalseProgress => "CND-IMP-006",
            Self::UnqualifiedPending => "CND-IMP-007",
            Self::TransactionViolation => "CND-IMP-008",
            Self::HostOperationViolation => "CND-IMP-009",
            Self::UnsupportedCheckpoint => "CND-IMP-010",
            Self::PrepareFailed => "CND-IMP-011",
            Self::InstantiationViolation => "CND-IMP-012",
        }
    }
}

impl fmt::Display for ImplementationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidProfile => "implementation execution profile is invalid",
            Self::ProfileIdentityMismatch => "execution profile identity does not match",
            Self::PlanBudgetExceeded => "execution profile exceeds the exact plan allocation",
            Self::IllegalLifecycle => "implementation lifecycle transition is illegal",
            Self::StepBoundExceeded => "implementation step exceeded an exact bound",
            Self::FalseProgress => "implementation reported progress without an observable commit",
            Self::UnqualifiedPending => "pending step has no exact finite wake interests",
            Self::TransactionViolation => "port transaction violated lease or reservation rules",
            Self::HostOperationViolation => "host operation lacks an exact bounded binding",
            Self::UnsupportedCheckpoint => "checkpoint export is absent or exceeds its contract",
            Self::PrepareFailed => "prepare-all failed before any instance started",
            Self::InstantiationViolation => {
                "instantiation does not match the exact prepared plan binding"
            }
        })
    }
}

/// Profile canonical-identity construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

impl From<CanonicalError<Infallible>> for ProfileIdentityError {
    fn from(value: CanonicalError<Infallible>) -> Self {
        Self::Canonical(value)
    }
}

fn hash_representation(
    value: ValueRepresentation<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic(
            "direction",
            CanonicalValue::Identifier(Id(match value.direction {
                Direction::Input => "input",
                Direction::Output => "output",
            })),
        ),
        semantic("port", CanonicalValue::Identifier(value.port)),
        semantic(
            "semantic_type_id",
            CanonicalValue::Identifier(value.semantic_type.contract_id),
        ),
        semantic(
            "semantic_type_version",
            CanonicalValue::Integer(i128::from(value.semantic_type.schema_version)),
        ),
        semantic(
            "semantic_type_hash",
            CanonicalValue::Bytes(value.semantic_type.semantic_hash.as_bytes()),
        ),
        semantic(
            "representation_id",
            CanonicalValue::Identifier(value.representation.id),
        ),
        semantic(
            "representation_version",
            CanonicalValue::Integer(i128::from(value.representation.schema_version)),
        ),
        semantic(
            "representation_hash",
            CanonicalValue::Bytes(value.representation.semantic_hash.as_bytes()),
        ),
        semantic(
            "ownership",
            CanonicalValue::Identifier(Id(value.ownership.as_str())),
        ),
        semantic(
            "disposition",
            CanonicalValue::Identifier(Id(value.disposition.as_str())),
        ),
        semantic(
            "max_bytes",
            CanonicalValue::Integer(i128::from(value.max_bytes)),
        ),
    ];
    CanonicalDescriptor {
        kind: Id("conduit/value-representation"),
        schema_version: 0,
        body: CanonicalValue::Map(&fields),
    }
    .semantic_hash()
}

fn hash_memory_claim(value: MemoryClaim) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic(
            "category",
            CanonicalValue::Identifier(Id(value.category.as_str())),
        ),
        semantic(
            "accounting",
            CanonicalValue::Identifier(Id(value.accounting.as_str())),
        ),
        semantic("bytes", CanonicalValue::Integer(i128::from(value.bytes))),
    ];
    CanonicalDescriptor {
        kind: Id("conduit/implementation-memory-claim"),
        schema_version: 0,
        body: CanonicalValue::Map(&fields),
    }
    .semantic_hash()
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn valid_pin(pin: PinnedDescriptor<'_>) -> bool {
    Id::new(pin.id.as_str()).is_ok() && pin.schema_version == 0
}
