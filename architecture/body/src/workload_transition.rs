//! Singular transaction for replacing a Body workload realization.

use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkloadTransitionState {
    Proposed,
    Checked,
    Planned {
        plan_id: String,
    },
    Reserved {
        plan_id: String,
    },
    Prepared {
        plan_id: String,
    },
    Committed {
        revision: u64,
        plan_id: String,
        play_id: Option<String>,
    },
    Refused {
        previous_revision: u64,
        reason: WorkloadTransitionRefusal,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkloadTransitionRefusal {
    InvalidTransition,
    StaleRevision,
    PlanningFailed,
    AdmissionFailed,
    AuthorityChanged,
    EnvironmentChanged,
    QuiescenceFailed,
    PlayStartFailed,
    ConcurrentEdit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkloadTransition {
    pub previous_revision: u64,
    pub proposed_revision: u64,
    pub state: WorkloadTransitionState,
}

impl WorkloadTransition {
    pub fn propose(previous: u64, proposed: u64) -> Result<Self, WorkloadTransitionRefusal> {
        if proposed
            != previous
                .checked_add(1)
                .ok_or(WorkloadTransitionRefusal::StaleRevision)?
        {
            return Err(WorkloadTransitionRefusal::StaleRevision);
        }
        Ok(Self {
            previous_revision: previous,
            proposed_revision: proposed,
            state: WorkloadTransitionState::Proposed,
        })
    }
    pub fn check(&mut self) -> Result<(), WorkloadTransitionRefusal> {
        self.advance(
            WorkloadTransitionState::Proposed,
            WorkloadTransitionState::Checked,
        )
    }
    pub fn plan(&mut self, plan: String) -> Result<(), WorkloadTransitionRefusal> {
        if plan.is_empty() {
            return self.refuse(WorkloadTransitionRefusal::PlanningFailed);
        }
        self.advance(
            WorkloadTransitionState::Checked,
            WorkloadTransitionState::Planned { plan_id: plan },
        )
    }
    pub fn reserve(&mut self) -> Result<(), WorkloadTransitionRefusal> {
        let WorkloadTransitionState::Planned { plan_id } = &self.state else {
            return Err(WorkloadTransitionRefusal::InvalidTransition);
        };
        self.state = WorkloadTransitionState::Reserved {
            plan_id: plan_id.clone(),
        };
        Ok(())
    }
    pub fn prepare(&mut self) -> Result<(), WorkloadTransitionRefusal> {
        let WorkloadTransitionState::Reserved { plan_id } = &self.state else {
            return Err(WorkloadTransitionRefusal::InvalidTransition);
        };
        self.state = WorkloadTransitionState::Prepared {
            plan_id: plan_id.clone(),
        };
        Ok(())
    }
    pub fn commit(
        &mut self,
        expected_current: u64,
        play: Option<String>,
    ) -> Result<(), WorkloadTransitionRefusal> {
        let WorkloadTransitionState::Prepared { plan_id } = &self.state else {
            return Err(WorkloadTransitionRefusal::InvalidTransition);
        };
        if expected_current != self.previous_revision {
            return self.refuse(WorkloadTransitionRefusal::ConcurrentEdit);
        }
        self.state = WorkloadTransitionState::Committed {
            revision: self.proposed_revision,
            plan_id: plan_id.clone(),
            play_id: play,
        };
        Ok(())
    }
    pub fn refuse(
        &mut self,
        reason: WorkloadTransitionRefusal,
    ) -> Result<(), WorkloadTransitionRefusal> {
        self.state = WorkloadTransitionState::Refused {
            previous_revision: self.previous_revision,
            reason,
        };
        Err(reason)
    }
    fn advance(
        &mut self,
        expected: WorkloadTransitionState,
        next: WorkloadTransitionState,
    ) -> Result<(), WorkloadTransitionRefusal> {
        if core::mem::discriminant(&self.state) != core::mem::discriminant(&expected) {
            return Err(WorkloadTransitionRefusal::InvalidTransition);
        }
        self.state = next;
        Ok(())
    }
}
