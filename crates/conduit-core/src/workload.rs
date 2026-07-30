//! Plan-visible workload admission and deadline guarantee boundaries.

use crate::{AuthorityTime, Id, InstancePath, SemanticHash};

pub const WORKLOAD_CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadGuarantee {
    Hard,
    Measured,
    HostObservedBestEffort,
    Unsupported,
}

impl WorkloadGuarantee {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hard => "hard",
            Self::Measured => "measured",
            Self::HostObservedBestEffort => "host-observed-best-effort",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadLimit {
    Finite(u64),
    Unsupported,
}

impl WorkloadLimit {
    const fn valid(self) -> bool {
        match self {
            Self::Finite(value) => value > 0,
            Self::Unsupported => true,
        }
    }

    const fn fits(self, capacity: Self) -> bool {
        match (self, capacity) {
            (Self::Unsupported, _) => true,
            (Self::Finite(required), Self::Finite(available)) => required <= available,
            (Self::Finite(_), Self::Unsupported) => false,
        }
    }

    fn checked_add(self, usage: u64) -> Option<Self> {
        match self {
            Self::Finite(value) => value.checked_add(usage).map(Self::Finite),
            Self::Unsupported => None,
        }
    }

    const fn value(self) -> u64 {
        match self {
            Self::Finite(value) => value,
            Self::Unsupported => 0,
        }
    }
}

/// Complete resource surface for one admitted workload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadBudget {
    pub work_units: WorkloadLimit,
    pub tasks: WorkloadLimit,
    pub processes: WorkloadLimit,
    pub descriptors: WorkloadLimit,
    pub connections: WorkloadLimit,
    pub storage_bytes: WorkloadLimit,
    pub device_operations: WorkloadLimit,
    pub network_bytes: WorkloadLimit,
    pub callbacks: WorkloadLimit,
    pub foreign_queue_items: WorkloadLimit,
    pub transition_overlap_work_units: WorkloadLimit,
}

impl WorkloadBudget {
    pub const UNSUPPORTED: Self = Self {
        work_units: WorkloadLimit::Unsupported,
        tasks: WorkloadLimit::Unsupported,
        processes: WorkloadLimit::Unsupported,
        descriptors: WorkloadLimit::Unsupported,
        connections: WorkloadLimit::Unsupported,
        storage_bytes: WorkloadLimit::Unsupported,
        device_operations: WorkloadLimit::Unsupported,
        network_bytes: WorkloadLimit::Unsupported,
        callbacks: WorkloadLimit::Unsupported,
        foreign_queue_items: WorkloadLimit::Unsupported,
        transition_overlap_work_units: WorkloadLimit::Unsupported,
    };

    pub const fn valid(self) -> bool {
        self.work_units.valid()
            && self.tasks.valid()
            && self.processes.valid()
            && self.descriptors.valid()
            && self.connections.valid()
            && self.storage_bytes.valid()
            && self.device_operations.valid()
            && self.network_bytes.valid()
            && self.callbacks.valid()
            && self.foreign_queue_items.valid()
            && self.transition_overlap_work_units.valid()
    }

    pub const fn fits_within(self, capacity: Self) -> bool {
        self.work_units.fits(capacity.work_units)
            && self.tasks.fits(capacity.tasks)
            && self.processes.fits(capacity.processes)
            && self.descriptors.fits(capacity.descriptors)
            && self.connections.fits(capacity.connections)
            && self.storage_bytes.fits(capacity.storage_bytes)
            && self.device_operations.fits(capacity.device_operations)
            && self.network_bytes.fits(capacity.network_bytes)
            && self.callbacks.fits(capacity.callbacks)
            && self.foreign_queue_items.fits(capacity.foreign_queue_items)
            && self
                .transition_overlap_work_units
                .fits(capacity.transition_overlap_work_units)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeadlineContract<'a> {
    pub time_basis: Id<'a>,
    pub relative_deadline_ticks: u64,
    pub maximum_jitter_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadContract<'a> {
    pub schema_version: u32,
    pub id: Id<'a>,
    pub service: Id<'a>,
    pub node: InstancePath<'a>,
    pub guarantee: WorkloadGuarantee,
    pub budget: WorkloadBudget,
    pub deadline: Option<DeadlineContract<'a>>,
    pub maximum_evidence_events: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadEvidenceKind {
    ExactEnforcement,
    HostObservation,
    Measurement,
    Benchmark,
    None,
}

impl WorkloadEvidenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactEnforcement => "exact-enforcement",
            Self::HostObservation => "host-observation",
            Self::Measurement => "measurement",
            Self::Benchmark => "benchmark",
            Self::None => "none",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadCapability<'a> {
    pub id: Id<'a>,
    pub identity: SemanticHash,
    pub host_observation: Id<'a>,
    pub evidence_kind: WorkloadEvidenceKind,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub capacity: WorkloadBudget,
    pub maximum_deadline_ticks: u64,
    pub maximum_jitter_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadReason {
    UnsupportedVersion,
    InvalidContract,
    UnsupportedWorkload,
    BenchmarkIsNotAuthority,
    ExactEnforcementRequired,
    StaleObservation,
    ClockMismatch,
    CapacityExceeded,
    DeadlineUnsupported,
    DeadlineOverflow,
    EvidenceExhausted,
    IllegalTransition,
    Overload,
    DeadlineMissed,
    JitterExceeded,
}

impl WorkloadReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-WRK-001",
            Self::InvalidContract => "CND-WRK-002",
            Self::UnsupportedWorkload => "CND-WRK-003",
            Self::BenchmarkIsNotAuthority => "CND-WRK-004",
            Self::ExactEnforcementRequired => "CND-WRK-005",
            Self::StaleObservation => "CND-WRK-006",
            Self::ClockMismatch => "CND-WRK-007",
            Self::CapacityExceeded => "CND-WRK-008",
            Self::DeadlineUnsupported => "CND-WRK-009",
            Self::DeadlineOverflow => "CND-WRK-010",
            Self::EvidenceExhausted => "CND-WRK-011",
            Self::IllegalTransition => "CND-WRK-012",
            Self::Overload => "CND-WRK-013",
            Self::DeadlineMissed => "CND-WRK-014",
            Self::JitterExceeded => "CND-WRK-015",
        }
    }
}

pub fn validate_workload_contract(contract: WorkloadContract<'_>) -> Result<(), WorkloadReason> {
    if contract.schema_version != WORKLOAD_CONTRACT_SCHEMA_VERSION {
        return Err(WorkloadReason::UnsupportedVersion);
    }
    if contract.id.0.is_empty()
        || contract.service.0.is_empty()
        || contract.node.as_str().is_empty()
        || !contract.budget.valid()
        || contract.maximum_evidence_events < 2
    {
        return Err(WorkloadReason::InvalidContract);
    }
    match (contract.guarantee, contract.deadline) {
        (WorkloadGuarantee::Hard, Some(deadline))
            if deadline.time_basis.0.is_empty() || deadline.relative_deadline_ticks == 0 =>
        {
            return Err(WorkloadReason::InvalidContract);
        }
        (WorkloadGuarantee::Hard, Some(_)) => {}
        (WorkloadGuarantee::Hard, None) => return Err(WorkloadReason::DeadlineUnsupported),
        (WorkloadGuarantee::Unsupported, Some(_)) => {
            return Err(WorkloadReason::InvalidContract);
        }
        _ => {}
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadAdmission {
    pub absolute_deadline_tick: Option<u64>,
    pub guarantee: WorkloadGuarantee,
}

pub fn admit_workload(
    contract: WorkloadContract<'_>,
    capability: WorkloadCapability<'_>,
    expected_observation: Id<'_>,
    now: AuthorityTime<'_>,
) -> Result<WorkloadAdmission, WorkloadReason> {
    validate_workload_contract(contract)?;
    if contract.guarantee == WorkloadGuarantee::Unsupported {
        return Err(WorkloadReason::UnsupportedWorkload);
    }
    if capability.evidence_kind == WorkloadEvidenceKind::Benchmark {
        return Err(WorkloadReason::BenchmarkIsNotAuthority);
    }
    if capability.id.0.is_empty()
        || capability.host_observation != expected_observation
        || capability.valid_until_tick <= capability.observed_at_tick
    {
        return Err(WorkloadReason::InvalidContract);
    }
    if capability.time_basis != now.basis {
        return Err(WorkloadReason::ClockMismatch);
    }
    if now.tick < capability.observed_at_tick || now.tick >= capability.valid_until_tick {
        return Err(WorkloadReason::StaleObservation);
    }
    if !contract.budget.fits_within(capability.capacity) {
        return Err(WorkloadReason::CapacityExceeded);
    }
    if contract.guarantee == WorkloadGuarantee::Hard
        && capability.evidence_kind != WorkloadEvidenceKind::ExactEnforcement
    {
        return Err(WorkloadReason::ExactEnforcementRequired);
    }
    let absolute_deadline_tick = contract
        .deadline
        .map(|deadline| {
            if deadline.time_basis != now.basis {
                return Err(WorkloadReason::ClockMismatch);
            }
            if capability.maximum_deadline_ticks == 0
                || deadline.relative_deadline_ticks > capability.maximum_deadline_ticks
                || deadline.maximum_jitter_ticks < capability.maximum_jitter_ticks
            {
                return Err(WorkloadReason::DeadlineUnsupported);
            }
            now.tick
                .checked_add(deadline.relative_deadline_ticks)
                .ok_or(WorkloadReason::DeadlineOverflow)
        })
        .transpose()?;
    Ok(WorkloadAdmission {
        absolute_deadline_tick,
        guarantee: contract.guarantee,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadPhase {
    Admitted,
    Completed,
    Terminal(WorkloadReason),
}

/// Allocation-free use-time accounting for one already admitted workload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadState<'a> {
    contract: WorkloadContract<'a>,
    phase: WorkloadPhase,
    absolute_deadline_tick: Option<u64>,
    used: WorkloadBudget,
    evidence_events: u32,
}

impl<'a> WorkloadState<'a> {
    pub const fn new(contract: WorkloadContract<'a>, admission: WorkloadAdmission) -> Self {
        Self {
            contract,
            phase: WorkloadPhase::Admitted,
            absolute_deadline_tick: admission.absolute_deadline_tick,
            used: WorkloadBudget::UNSUPPORTED,
            evidence_events: 0,
        }
    }

    pub const fn phase(&self) -> WorkloadPhase {
        self.phase
    }

    pub const fn used(&self) -> WorkloadBudget {
        self.used
    }

    pub fn record_usage(&mut self, usage: WorkloadUsage) -> Result<(), WorkloadReason> {
        if self.phase != WorkloadPhase::Admitted {
            return Err(WorkloadReason::IllegalTransition);
        }
        let next = WorkloadBudget {
            work_units: add_usage(self.used.work_units, usage.work_units)?,
            tasks: add_usage(self.used.tasks, usage.tasks)?,
            processes: add_usage(self.used.processes, usage.processes)?,
            descriptors: add_usage(self.used.descriptors, usage.descriptors)?,
            connections: add_usage(self.used.connections, usage.connections)?,
            storage_bytes: add_usage(self.used.storage_bytes, usage.storage_bytes)?,
            device_operations: add_usage(self.used.device_operations, usage.device_operations)?,
            network_bytes: add_usage(self.used.network_bytes, usage.network_bytes)?,
            callbacks: add_usage(self.used.callbacks, usage.callbacks)?,
            foreign_queue_items: add_usage(
                self.used.foreign_queue_items,
                usage.foreign_queue_items,
            )?,
            transition_overlap_work_units: add_usage(
                self.used.transition_overlap_work_units,
                usage.transition_overlap_work_units,
            )?,
        };
        if !usage_fits(next, self.contract.budget) {
            self.terminal(WorkloadReason::Overload)?;
            return Err(WorkloadReason::Overload);
        }
        self.used = next;
        Ok(())
    }

    pub fn observe_tick(&mut self, now: AuthorityTime<'_>) -> Result<(), WorkloadReason> {
        if self.phase != WorkloadPhase::Admitted {
            return Err(WorkloadReason::IllegalTransition);
        }
        let Some(deadline) = self.absolute_deadline_tick else {
            return Ok(());
        };
        let expected_basis = self
            .contract
            .deadline
            .expect("admitted deadline")
            .time_basis;
        if now.basis != expected_basis {
            self.terminal(WorkloadReason::ClockMismatch)?;
            return Err(WorkloadReason::ClockMismatch);
        }
        if now.tick >= deadline {
            self.terminal(WorkloadReason::DeadlineMissed)?;
            return Err(WorkloadReason::DeadlineMissed);
        }
        Ok(())
    }

    pub fn complete(
        &mut self,
        now: AuthorityTime<'_>,
        observed_jitter_ticks: u64,
    ) -> Result<(), WorkloadReason> {
        self.observe_tick(now)?;
        if self
            .contract
            .deadline
            .is_some_and(|deadline| observed_jitter_ticks > deadline.maximum_jitter_ticks)
        {
            self.terminal(WorkloadReason::JitterExceeded)?;
            return Err(WorkloadReason::JitterExceeded);
        }
        self.record_evidence()?;
        self.phase = WorkloadPhase::Completed;
        Ok(())
    }

    fn terminal(&mut self, reason: WorkloadReason) -> Result<(), WorkloadReason> {
        self.record_evidence()?;
        self.phase = WorkloadPhase::Terminal(reason);
        Ok(())
    }

    fn record_evidence(&mut self) -> Result<(), WorkloadReason> {
        let next = self
            .evidence_events
            .checked_add(1)
            .ok_or(WorkloadReason::EvidenceExhausted)?;
        if next > self.contract.maximum_evidence_events {
            return Err(WorkloadReason::EvidenceExhausted);
        }
        self.evidence_events = next;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkloadUsage {
    pub work_units: u64,
    pub tasks: u64,
    pub processes: u64,
    pub descriptors: u64,
    pub connections: u64,
    pub storage_bytes: u64,
    pub device_operations: u64,
    pub network_bytes: u64,
    pub callbacks: u64,
    pub foreign_queue_items: u64,
    pub transition_overlap_work_units: u64,
}

fn add_usage(current: WorkloadLimit, addition: u64) -> Result<WorkloadLimit, WorkloadReason> {
    match current {
        WorkloadLimit::Unsupported if addition == 0 => Ok(WorkloadLimit::Unsupported),
        WorkloadLimit::Unsupported => Ok(WorkloadLimit::Finite(addition)),
        _ => current
            .checked_add(addition)
            .ok_or(WorkloadReason::Overload),
    }
}

fn usage_fits(usage: WorkloadBudget, limit: WorkloadBudget) -> bool {
    usage.work_units.value() <= limit.work_units.value()
        && usage.tasks.value() <= limit.tasks.value()
        && usage.processes.value() <= limit.processes.value()
        && usage.descriptors.value() <= limit.descriptors.value()
        && usage.connections.value() <= limit.connections.value()
        && usage.storage_bytes.value() <= limit.storage_bytes.value()
        && usage.device_operations.value() <= limit.device_operations.value()
        && usage.network_bytes.value() <= limit.network_bytes.value()
        && usage.callbacks.value() <= limit.callbacks.value()
        && usage.foreign_queue_items.value() <= limit.foreign_queue_items.value()
        && usage.transition_overlap_work_units.value()
            <= limit.transition_overlap_work_units.value()
}
