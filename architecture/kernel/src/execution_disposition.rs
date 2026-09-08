//! Machine-readable semantic and embodiment disposition vocabulary.
//!
//! This classification does not make quiescence terminal and does not turn an
//! embodiment ceiling into semantic completion. Callers retain the exact
//! [`Failure`] beside the classification when one exists.

use crate::{Failure, FailureCode};

/// Checked semantic policy for classifying a structurally drained scheduler.
///
/// Draining is not itself a terminal fact. Forms remain live by default; the
/// exceptional policy is admitted only when checked semantics provide an exact
/// completion witness.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DrainedPlayDisposition {
    #[default]
    Quiescent,
    SemanticCompletion,
}

impl DrainedPlayDisposition {
    pub const fn execution_disposition(self) -> ExecutionDisposition {
        match self {
            Self::Quiescent => ExecutionDisposition::QuiescentAwaitingInput,
            Self::SemanticCompletion => ExecutionDisposition::SemanticCompleted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionDisposition {
    Continued,
    QuiescentAwaitingInput,
    SemanticCompleted,
    BodyLulled,
    Cancelled,
    SemanticRefused,
    ValueDomainOverflow,
    StateCapacityExhausted,
    ResourceCapacityExhausted,
    WorkBudgetExhausted,
    Failed,
    HostLost,
    BootLost,
    ResourceLost,
    LineLost,
    PlanRetired,
    PlanReplaced,
}

impl ExecutionDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Continued => "continued",
            Self::QuiescentAwaitingInput => "quiescent_awaiting_input",
            Self::SemanticCompleted => "semantic_completed",
            Self::BodyLulled => "body_lulled",
            Self::Cancelled => "cancelled",
            Self::SemanticRefused => "semantic_refused",
            Self::ValueDomainOverflow => "value_domain_overflow",
            Self::StateCapacityExhausted => "state_capacity_exhausted",
            Self::ResourceCapacityExhausted => "resource_capacity_exhausted",
            Self::WorkBudgetExhausted => "work_budget_exhausted",
            Self::Failed => "failed",
            Self::HostLost => "host_lost",
            Self::BootLost => "boot_lost",
            Self::ResourceLost => "resource_lost",
            Self::LineLost => "line_lost",
            Self::PlanRetired => "plan_retired",
            Self::PlanReplaced => "plan_replaced",
        }
    }

    /// Classify an exact kernel failure without discarding its code or detail.
    pub const fn from_failure(failure: Failure) -> Self {
        match failure.code {
            FailureCode::InvalidInput => Self::SemanticRefused,
            FailureCode::StateCapacityExhausted => Self::StateCapacityExhausted,
            FailureCode::StorageExhausted | FailureCode::IdentityCapacityExhausted => {
                Self::ResourceCapacityExhausted
            }
            FailureCode::WorkBudgetExhausted => Self::WorkBudgetExhausted,
            FailureCode::Cancelled => Self::Cancelled,
            FailureCode::InvalidPort
            | FailureCode::InvalidLifecycle
            | FailureCode::HostOperationDenied
            | FailureCode::HostOperationFailed => Self::Failed,
        }
    }

    pub const fn is_semantic_completion(self) -> bool {
        matches!(self, Self::SemanticCompleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_names_keep_semantic_and_embodiment_outcomes_distinct() {
        let dispositions = [
            ExecutionDisposition::SemanticCompleted,
            ExecutionDisposition::QuiescentAwaitingInput,
            ExecutionDisposition::ValueDomainOverflow,
            ExecutionDisposition::StateCapacityExhausted,
            ExecutionDisposition::ResourceCapacityExhausted,
            ExecutionDisposition::WorkBudgetExhausted,
            ExecutionDisposition::PlanReplaced,
        ];
        for (index, disposition) in dispositions.iter().enumerate() {
            assert!(dispositions[index + 1..]
                .iter()
                .all(|other| disposition.as_str() != other.as_str()));
        }
        assert!(ExecutionDisposition::SemanticCompleted.is_semantic_completion());
        assert!(!ExecutionDisposition::QuiescentAwaitingInput.is_semantic_completion());
        assert!(!ExecutionDisposition::WorkBudgetExhausted.is_semantic_completion());
    }

    #[test]
    fn exact_failure_categories_survive_classification() {
        for (code, expected) in [
            (
                FailureCode::InvalidInput,
                ExecutionDisposition::SemanticRefused,
            ),
            (
                FailureCode::StateCapacityExhausted,
                ExecutionDisposition::StateCapacityExhausted,
            ),
            (
                FailureCode::StorageExhausted,
                ExecutionDisposition::ResourceCapacityExhausted,
            ),
            (
                FailureCode::WorkBudgetExhausted,
                ExecutionDisposition::WorkBudgetExhausted,
            ),
            (FailureCode::Cancelled, ExecutionDisposition::Cancelled),
        ] {
            let failure = Failure { code, detail: 17 };
            assert_eq!(ExecutionDisposition::from_failure(failure), expected);
            assert_eq!(failure.code, code);
            assert_eq!(failure.detail, 17);
        }
    }

    #[test]
    fn drained_work_is_quiescent_unless_semantics_explicitly_complete() {
        assert_eq!(
            DrainedPlayDisposition::default().execution_disposition(),
            ExecutionDisposition::QuiescentAwaitingInput
        );
        assert_eq!(
            DrainedPlayDisposition::SemanticCompletion.execution_disposition(),
            ExecutionDisposition::SemanticCompleted
        );
    }
}
