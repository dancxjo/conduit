//! Persistent ownership and cooperative pumping of exact scheduler runs.
//!
//! This module does not introduce another executor. It owns one already
//! admitted `DeterministicExecutor` and exposes bounded turns to the host.

use std::cell::RefCell;
use std::rc::Rc;

use conduit_core::{SemanticHash, StopPolicy, TerminalClass};

use crate::{
    DeterministicExecutor, SchedulerError, SchedulerHighWater, SchedulerNode, SchedulerStatus,
    ValueStorageUsage,
};

/// Immutable identities pinned when an authorized exact run starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactRunIdentity {
    pub plan_identity: SemanticHash,
    pub source_semantic_hash: SemanticHash,
    pub plan_epoch: u64,
    pub run_id: String,
}

/// Externally visible state of a persistent exact run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactRunState {
    /// There is ready work; a later bounded pump can continue it.
    Active,
    /// The run is alive but needs an authorized timer, host operation, input,
    /// output, or cancellation wake. Waiting is not terminal.
    Waiting,
    /// Drain cancellation was requested and retained work is being settled.
    Quiescing,
    /// The exact run reached one terminal class.
    Terminal(TerminalClass),
}

/// Bounded facts returned by one cooperative scheduling turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactRunPump {
    pub state: ExactRunState,
    pub decisions: u64,
    pub tick: u64,
    /// The next bounded scheduler-event cursor. Event draining and retention
    /// policy are added by the dedicated evidence follow-up.
    pub event_cursor: u64,
    pub high_water: SchedulerHighWater,
}

/// Finite admission controller for concurrently retained exact-run sessions.
///
/// A host creates one registry for its runtime boundary and passes it to each
/// start request. The registry reserves the caller's declared runtime budget
/// before any implementation is prepared or started; releasing a terminal
/// session (or abandoning a failed start) returns that reservation.
#[derive(Clone, Debug)]
pub struct ExactRunSessionRegistry {
    capacity: Rc<RefCell<SessionCapacity>>,
}

#[derive(Debug)]
struct SessionCapacity {
    maximum_sessions: usize,
    maximum_reserved_bytes: u64,
    active_sessions: usize,
    reserved_bytes: u64,
    abandoned_live_session: bool,
}

/// A non-forgeable reservation retained by exactly one exact-run session.
#[derive(Debug)]
pub struct ExactRunSessionAdmission {
    capacity: Rc<RefCell<SessionCapacity>>,
    reserved_bytes: u64,
}

impl ExactRunSessionRegistry {
    /// Creates a finite hosted-session admission boundary.
    pub fn new(
        maximum_sessions: usize,
        maximum_reserved_bytes: u64,
    ) -> Result<Self, SchedulerError> {
        if maximum_sessions == 0 || maximum_reserved_bytes == 0 {
            return Err(SchedulerError::InvalidPolicy);
        }
        Ok(Self {
            capacity: Rc::new(RefCell::new(SessionCapacity {
                maximum_sessions,
                maximum_reserved_bytes,
                active_sessions: 0,
                reserved_bytes: 0,
                abandoned_live_session: false,
            })),
        })
    }

    /// Reserves one concurrent session and its declared runtime budget before
    /// node preparation or execution begins.
    pub fn admit(&self, reserved_bytes: u64) -> Result<ExactRunSessionAdmission, SchedulerError> {
        if reserved_bytes == 0 {
            return Err(SchedulerError::InvalidPolicy);
        }
        let mut capacity = self.capacity.borrow_mut();
        if capacity.abandoned_live_session {
            return Err(SchedulerError::AllocationUnavailable);
        }
        let next_sessions = capacity
            .active_sessions
            .checked_add(1)
            .ok_or(SchedulerError::AllocationUnavailable)?;
        let next_bytes = capacity
            .reserved_bytes
            .checked_add(reserved_bytes)
            .ok_or(SchedulerError::AllocationUnavailable)?;
        if next_sessions > capacity.maximum_sessions || next_bytes > capacity.maximum_reserved_bytes
        {
            return Err(SchedulerError::AllocationUnavailable);
        }
        capacity.active_sessions = next_sessions;
        capacity.reserved_bytes = next_bytes;
        Ok(ExactRunSessionAdmission {
            capacity: Rc::clone(&self.capacity),
            reserved_bytes,
        })
    }

    #[must_use]
    pub fn active_sessions(&self) -> usize {
        self.capacity.borrow().active_sessions
    }

    #[must_use]
    pub fn reserved_bytes(&self) -> u64 {
        self.capacity.borrow().reserved_bytes
    }

    /// Whether a nonterminal session was abandoned. This is distinct from a
    /// requested cancellation, and the registry rejects another Start until
    /// its owning host is replaced or recovered deliberately.
    #[must_use]
    pub fn has_abandoned_live_session(&self) -> bool {
        self.capacity.borrow().abandoned_live_session
    }
}

impl Drop for ExactRunSessionAdmission {
    fn drop(&mut self) {
        let mut capacity = self.capacity.borrow_mut();
        capacity.active_sessions = capacity.active_sessions.saturating_sub(1);
        capacity.reserved_bytes = capacity.reserved_bytes.saturating_sub(self.reserved_bytes);
    }
}

impl ExactRunSessionAdmission {
    fn mark_live_session_abandoned(&self) {
        self.capacity.borrow_mut().abandoned_live_session = true;
    }
}

/// One persistent exact execution session. All mutable scheduler state is
/// owned by this value and is released when it is finalized or dropped.
pub struct ExactRunSession<N: SchedulerNode> {
    identity: ExactRunIdentity,
    executor: Option<DeterministicExecutor<N>>,
    admission: Option<ExactRunSessionAdmission>,
    stop: Option<StopPolicy>,
}

impl<N: SchedulerNode> Drop for ExactRunSession<N> {
    fn drop(&mut self) {
        if self
            .executor
            .as_ref()
            .is_some_and(|executor| !is_terminal(executor.status()))
        {
            self.admission
                .as_ref()
                .expect("live exact-run session retains its admission")
                .mark_live_session_abandoned();
        }
    }
}

impl<N: SchedulerNode> ExactRunSession<N> {
    #[must_use]
    pub fn new(
        admission: ExactRunSessionAdmission,
        identity: ExactRunIdentity,
        executor: DeterministicExecutor<N>,
    ) -> Self {
        Self {
            identity,
            executor: Some(executor),
            admission: Some(admission),
            stop: None,
        }
    }

    #[must_use]
    pub fn identity(&self) -> &ExactRunIdentity {
        &self.identity
    }

    #[must_use]
    pub fn state(&self) -> ExactRunState {
        state_for(self.executor().status(), self.stop)
    }

    #[must_use]
    pub fn scheduler_status(&self) -> SchedulerStatus {
        self.executor().status()
    }

    /// Pump at most `quantum` fair node decisions. Reaching the quantum gives
    /// control back to the host without resetting any run identity, counter,
    /// queue, or timer state.
    pub fn pump(&mut self, quantum: u64) -> Result<ExactRunPump, SchedulerError> {
        if quantum == 0 {
            return Err(SchedulerError::InvalidPolicy);
        }
        let start = self.executor().decisions();
        while self.executor().decisions().saturating_sub(start) < quantum {
            let before = self.executor().decisions();
            let status = self.executor_mut().run_one()?;
            if !matches!(status, SchedulerStatus::Running) || self.executor().decisions() == before
            {
                break;
            }
        }
        Ok(self.snapshot())
    }

    /// Advance only the active run's exact scheduler clock. The caller must
    /// supply an admitted monotonic tick; this never creates a new epoch.
    pub fn advance_to(&mut self, tick: u64) -> Result<ExactRunPump, SchedulerError> {
        self.executor_mut().advance_to(tick)?;
        Ok(self.snapshot())
    }

    /// Wake one exact named host operation on this session.
    pub fn notify_host_operation(
        &mut self,
        subject: conduit_core::Id<'static>,
    ) -> Result<ExactRunPump, SchedulerError> {
        self.executor_mut().notify_host_operation(subject)?;
        Ok(self.snapshot())
    }

    /// Request the active session's exact Drain or Abort path.
    pub fn cancel(&mut self, stop: StopPolicy) -> Result<ExactRunPump, SchedulerError> {
        self.executor_mut().cancel(stop)?;
        self.stop = Some(stop);
        Ok(self.snapshot())
    }

    #[must_use]
    pub fn next_timer_deadline(&self) -> Option<u64> {
        self.executor().next_timer_deadline()
    }

    #[must_use]
    pub fn scheduler_event_count(&self) -> usize {
        self.executor().event_count()
    }

    pub fn scheduler_events(&self) -> impl Iterator<Item = &crate::SchedulerEvent> {
        self.executor().events()
    }

    #[must_use]
    pub fn exact_evidence(&self) -> Vec<crate::ExactEvidenceRecord> {
        self.executor().project_exact_evidence(
            &self.identity.plan_identity.to_string(),
            self.identity.plan_epoch,
            &self.identity.run_id,
        )
    }

    #[must_use]
    pub fn allocation(&self) -> crate::SchedulerAllocation {
        self.executor().allocation()
    }

    /// The finite runtime reservation held for this session's complete life.
    #[must_use]
    pub fn reserved_session_bytes(&self) -> u64 {
        self.admission
            .as_ref()
            .map_or(0, |admission| admission.reserved_bytes)
    }

    #[must_use]
    pub fn plan_budget(&self) -> conduit_core::PlanResourceBudget {
        self.executor().plan_budget()
    }

    #[must_use]
    pub fn high_water(&self) -> SchedulerHighWater {
        self.executor().high_water()
    }

    /// Current and high-water payload storage for hosts that expose a fixed
    /// value arena. Portable drivers return no host-specific measurement.
    #[must_use]
    pub fn value_storage_usage(&self) -> Option<ValueStorageUsage> {
        self.executor().value_storage_usage()
    }

    /// Releases the owned scheduler only after it is terminal. A nonterminal
    /// error leaves this same session retained and usable by the caller.
    pub fn finalize(&mut self) -> Result<DeterministicExecutor<N>, ExactRunState> {
        if is_terminal(self.executor().status()) {
            let executor = self.executor.take().expect("terminal executor is retained");
            let admission = self
                .admission
                .take()
                .expect("terminal executor retains its admission");
            drop(admission);
            Ok(executor)
        } else {
            Err(self.state())
        }
    }

    fn executor(&self) -> &DeterministicExecutor<N> {
        self.executor
            .as_ref()
            .expect("exact-run session executor is retained until finalization")
    }

    fn executor_mut(&mut self) -> &mut DeterministicExecutor<N> {
        self.executor
            .as_mut()
            .expect("exact-run session executor is retained until finalization")
    }

    fn snapshot(&self) -> ExactRunPump {
        ExactRunPump {
            state: self.state(),
            decisions: self.executor().decisions(),
            tick: self.executor().tick(),
            event_cursor: u64::try_from(self.executor().event_count()).unwrap_or(u64::MAX),
            high_water: self.executor().high_water(),
        }
    }
}

const fn is_terminal(status: SchedulerStatus) -> bool {
    matches!(
        status,
        SchedulerStatus::Succeeded
            | SchedulerStatus::Cancelled
            | SchedulerStatus::Disconnected
            | SchedulerStatus::Failed(_)
    )
}

fn state_for(status: SchedulerStatus, stop: Option<StopPolicy>) -> ExactRunState {
    match status {
        SchedulerStatus::Running => match stop {
            Some(StopPolicy::Drain) => ExactRunState::Quiescing,
            Some(StopPolicy::Abort) => ExactRunState::Terminal(TerminalClass::Cancelled),
            None => ExactRunState::Active,
        },
        SchedulerStatus::Stalled => match stop {
            Some(StopPolicy::Drain) => ExactRunState::Quiescing,
            Some(StopPolicy::Abort) => ExactRunState::Terminal(TerminalClass::Cancelled),
            None => ExactRunState::Waiting,
        },
        SchedulerStatus::Succeeded => ExactRunState::Terminal(TerminalClass::Succeeded),
        SchedulerStatus::Cancelled => ExactRunState::Terminal(TerminalClass::Cancelled),
        SchedulerStatus::Disconnected => ExactRunState::Terminal(TerminalClass::Disconnected),
        SchedulerStatus::Failed(_) => ExactRunState::Terminal(TerminalClass::Failed),
    }
}
