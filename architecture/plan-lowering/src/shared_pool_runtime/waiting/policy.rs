//! Request ordering is soft preparation policy, never an occupation grant.
use super::*;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolWaitingClass {
    Interactive,
    Batch,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolWaitingPolicy {
    Fifo,
    InteractiveWithAging { maximum_bypasses: u8 },
    RequesterTurnWithAging { maximum_bypasses: u8 },
}
impl PoolWaitingPolicy {
    pub(super) fn valid(self) -> bool {
        match self {
            Self::Fifo => true,
            Self::InteractiveWithAging { maximum_bypasses }
            | Self::RequesterTurnWithAging { maximum_bypasses } => {
                (1..=32).contains(&maximum_bypasses)
            }
        }
    }
    fn maximum_bypasses(self) -> Option<u8> {
        match self {
            Self::Fifo => None,
            Self::InteractiveWithAging { maximum_bypasses }
            | Self::RequesterTurnWithAging { maximum_bypasses } => Some(maximum_bypasses),
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PoolWaitingChoiceReason {
    Fifo,
    Interactive,
    RequesterTurn,
    Aged,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolWaitingCandidate {
    pub operation: PoolOperationId,
    pub requester: String,
    pub class: PoolWaitingClass,
    pub sequence: u64,
    pub bypasses: u8,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolWaitingDecision {
    pub policy: PoolWaitingPolicy,
    pub operation: PoolOperationId,
    pub reason: PoolWaitingChoiceReason,
    pub candidates: Vec<PoolWaitingCandidate>,
}
impl BoundedPoolWaiting {
    fn choice(&self) -> Option<(usize, PoolWaitingChoiceReason)> {
        let oldest = |predicate: &dyn Fn(&Entry) -> bool| {
            self.entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| e.as_ref().filter(|e| predicate(e)).map(|e| (i, e.sequence)))
                .min_by_key(|(_, sequence)| *sequence)
                .map(|(i, _)| i)
        };
        // A continuously waiting request can be bypassed at most the configured
        // threshold plus 31 older aged requests, regardless of later arrivals.
        if let Some(maximum) = self.policy.maximum_bypasses() {
            if let Some(i) = oldest(&|e| e.bypasses >= maximum) {
                return Some((i, PoolWaitingChoiceReason::Aged));
            }
        }
        match self.policy {
            PoolWaitingPolicy::InteractiveWithAging { .. } => {
                if let Some(i) = oldest(&|e| e.operation.class == PoolWaitingClass::Interactive) {
                    return Some((i, PoolWaitingChoiceReason::Interactive));
                }
            }
            PoolWaitingPolicy::RequesterTurnWithAging { .. } => {
                if let Some(last) = &self.last_requester {
                    if let Some(i) = oldest(&|e| &e.operation.requester != last) {
                        return Some((i, PoolWaitingChoiceReason::RequesterTurn));
                    }
                }
            }
            PoolWaitingPolicy::Fifo => {}
        }
        oldest(&|_| true).map(|i| (i, PoolWaitingChoiceReason::Fifo))
    }
    pub(super) fn first(&self) -> Option<usize> {
        self.choice().map(|(i, _)| i)
    }
    pub(super) fn decision(&self) -> PoolWaitingDecision {
        let (index, reason) = self.choice().expect("checked pending operation");
        PoolWaitingDecision {
            policy: self.policy,
            operation: self.entries[index]
                .as_ref()
                .unwrap()
                .operation
                .operation
                .clone(),
            reason,
            candidates: self
                .entries
                .iter()
                .flatten()
                .map(|e| PoolWaitingCandidate {
                    operation: e.operation.operation.clone(),
                    requester: e.operation.requester.clone(),
                    class: e.operation.class,
                    sequence: e.sequence,
                    bypasses: e.bypasses,
                })
                .collect(),
        }
    }
    pub(super) fn completed_selection(&mut self, operation: &PoolOperationId) {
        for entry in self.entries.iter_mut().flatten() {
            if &entry.operation.operation == operation {
                self.last_requester = Some(entry.operation.requester.clone());
            } else {
                entry.bypasses = entry.bypasses.saturating_add(1);
            }
        }
    }
}
