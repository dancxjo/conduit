//! Portable scheduler invariants shared by hosted and constrained executors.

use core::fmt;

/// Version of the deterministic scheduler contract.
pub const SCHEDULER_CONTRACT_VERSION: u32 = 1;

/// Fixed ordering used whenever more than one node is runnable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadyQueueDiscipline {
    /// One bounded step per node, appended at the tail after progress or yield.
    RoundRobin,
}

/// Plan-independent limits for one executor run.
///
/// Queue capacities and node work limits remain exact plan facts. These limits
/// bound scheduler-owned time and evidence storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerPolicy {
    /// Contract version interpreted by the executor.
    pub schema_version: u32,
    /// Deterministic ready-queue ordering.
    pub ready_queue: ReadyQueueDiscipline,
    /// Maximum node decisions before the run must terminate or be renewed.
    pub max_decisions: u64,
    /// Maximum simulated-clock tick accepted by this run.
    pub max_tick: u64,
    /// Maximum consecutive full-budget yields by one node.
    pub max_consecutive_yields: u32,
    /// Exact capacity of the executor-owned scheduler event log.
    pub max_events: u32,
}

impl SchedulerPolicy {
    /// Validate finite scheduler-owned bounds.
    pub const fn validate(self) -> Result<(), SchedulerContractError> {
        if self.schema_version != SCHEDULER_CONTRACT_VERSION {
            return Err(SchedulerContractError::UnsupportedVersion);
        }
        if self.max_decisions == 0
            || self.max_tick == 0
            || self.max_consecutive_yields == 0
            || self.max_events == 0
        {
            return Err(SchedulerContractError::UnboundedPolicy);
        }
        Ok(())
    }
}

/// Exact scheduler reason retained for every ready-queue decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerDecisionReason {
    Initial,
    Progress,
    FairYield,
    InputReady,
    OutputReady,
    TimerReady,
    HostOperationReady,
    Cancellation,
    TerminalPropagation,
}

/// Mutually exclusive runtime population counters for a bounded pool.
///
/// #60 owns admission and supervision behavior. This type is the scheduler
/// reconciliation boundary it consumes: every reserved slot is in exactly one
/// state, and resource-waiting work is not runnable demand.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PoolPopulation {
    pub queued: u16,
    pub pending: u16,
    pub blocked: u16,
    pub ready: u16,
    pub running: u16,
    pub preempted: u16,
    pub checkpointing: u16,
    pub restarting: u16,
    pub retiring: u16,
    pub terminal_cleanup: u16,
    /// Independently maintained reservation total. Validation rejects drift.
    pub reserved_total: u16,
}

impl PoolPopulation {
    /// Count slots charged to the live-instance reservation.
    pub const fn live_reserved(self) -> Option<u16> {
        let mut total = 0_u16;
        let states = [
            self.pending,
            self.blocked,
            self.ready,
            self.running,
            self.preempted,
            self.checkpointing,
            self.restarting,
            self.retiring,
            self.terminal_cleanup,
        ];
        let mut index = 0;
        while index < states.len() {
            match total.checked_add(states[index]) {
                Some(next) => total = next,
                None => return None,
            }
            index += 1;
        }
        Some(total)
    }

    /// Count work that can actually consume a scheduler turn now.
    ///
    /// Pending host/device operations and blocked resource waits deliberately
    /// do not become a "shortfall" that could manufacture new admissions.
    pub const fn runnable(self) -> Option<u16> {
        self.ready.checked_add(self.running)
    }

    /// Reconcile every population against the exact plan pool maxima.
    pub const fn validate(
        self,
        maximum_live: u16,
        maximum_queued: u16,
    ) -> Result<(), SchedulerContractError> {
        if maximum_live == 0 {
            return Err(SchedulerContractError::PopulationExceeded);
        }
        let live = match self.live_reserved() {
            Some(value) => value,
            None => return Err(SchedulerContractError::PopulationOverflow),
        };
        let total = match live.checked_add(self.queued) {
            Some(value) => value,
            None => return Err(SchedulerContractError::PopulationOverflow),
        };
        let maximum = match maximum_live.checked_add(maximum_queued) {
            Some(value) => value,
            None => return Err(SchedulerContractError::PopulationOverflow),
        };
        if live > maximum_live
            || self.queued > maximum_queued
            || total > maximum
            || total != self.reserved_total
        {
            return Err(SchedulerContractError::PopulationExceeded);
        }
        Ok(())
    }
}

/// Inputs required before a scheduler may authorize a restart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestartAssessment {
    pub attempt: u16,
    pub maximum_attempts: u16,
    pub progress_ticks: u64,
    pub minimum_progress_ticks: u64,
    pub checkpoint_cost_ticks: u64,
    pub remaining_ticks: u64,
    pub cooldown_until_tick: u64,
    pub now_tick: u64,
    pub starvation_deadline_tick: u64,
}

/// Explainable bounded restart decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartDecision {
    Restart,
    WaitForCooldown,
    PreserveCurrentAttempt,
    AttemptsExhausted,
    StarvationDeadlineReached,
}

/// Assess restart without debt, shortfall, or scheduler-order counters.
#[must_use]
pub const fn assess_restart(value: RestartAssessment) -> RestartDecision {
    if value.now_tick >= value.starvation_deadline_tick {
        return RestartDecision::StarvationDeadlineReached;
    }
    if value.maximum_attempts == 0 || value.attempt >= value.maximum_attempts {
        return RestartDecision::AttemptsExhausted;
    }
    if value.now_tick < value.cooldown_until_tick {
        return RestartDecision::WaitForCooldown;
    }
    if value.progress_ticks >= value.minimum_progress_ticks
        || value.checkpoint_cost_ticks >= value.remaining_ticks
    {
        return RestartDecision::PreserveCurrentAttempt;
    }
    RestartDecision::Restart
}

/// Portable scheduler-contract failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerContractError {
    UnsupportedVersion,
    UnboundedPolicy,
    PopulationOverflow,
    PopulationExceeded,
}

impl SchedulerContractError {
    /// Stable diagnostic family.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-SCH-001",
            Self::UnboundedPolicy => "CND-SCH-002",
            Self::PopulationOverflow | Self::PopulationExceeded => "CND-SCH-003",
        }
    }
}

impl fmt::Display for SchedulerContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedVersion => "scheduler contract version is unsupported",
            Self::UnboundedPolicy => "scheduler-owned limits must be positive and finite",
            Self::PopulationOverflow => "pool population accounting overflowed",
            Self::PopulationExceeded => {
                "pool population does not reconcile with exact reservations"
            }
        })
    }
}
