use conduit_kernel::ExecutionDisposition;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionDispositionExplanation {
    pub disposition: &'static str,
    pub summary: &'static str,
    pub semantic_completion: bool,
}

pub const fn explain_execution_disposition(
    disposition: ExecutionDisposition,
) -> ExecutionDispositionExplanation {
    let summary = match disposition {
        ExecutionDisposition::Continued => "The Play continues within its admitted realization.",
        ExecutionDisposition::QuiescentAwaitingInput => {
            "The Play is awaiting input; it has not completed."
        }
        ExecutionDisposition::SemanticCompleted => {
            "The Form reached its defined semantic completion."
        }
        ExecutionDisposition::BodyLulled => {
            "The Body suspended the Play lifecycle; the Form did not complete."
        }
        ExecutionDisposition::Cancelled => "The Play was explicitly cancelled before completion.",
        ExecutionDisposition::SemanticRefused => {
            "The requested value or transition was outside the semantic contract."
        }
        ExecutionDisposition::ValueDomainOverflow => {
            "The value is outside its semantic domain; no result was wrapped or truncated."
        }
        ExecutionDisposition::StateCapacityExhausted => {
            "The next State value exceeded its admitted State capacity."
        }
        ExecutionDisposition::ResourceCapacityExhausted => {
            "The realization exhausted an admitted non-State resource capacity."
        }
        ExecutionDisposition::WorkBudgetExhausted => {
            "The realization exhausted its admitted work allowance before semantic completion."
        }
        ExecutionDisposition::Failed => "Execution failed for a retained machine-readable cause.",
        ExecutionDisposition::HostLost => "The exact Host needed by the Play was lost.",
        ExecutionDisposition::BootLost => "The exact Boot needed by the Play was lost.",
        ExecutionDisposition::ResourceLost => "An admitted Resource needed by the Play was lost.",
        ExecutionDisposition::LineLost => "An exact Line needed by the Play was lost.",
        ExecutionDisposition::PlanRetired => {
            "The Plan was retired; this does not assert semantic completion."
        }
        ExecutionDisposition::PlanReplaced => {
            "A replacement Plan was selected; this does not restart or complete the Form."
        }
    };
    ExecutionDispositionExplanation {
        disposition: disposition.as_str(),
        summary,
        semantic_completion: disposition.is_semantic_completion(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patchbay_does_not_describe_waiting_or_exhaustion_as_completion() {
        let completed = explain_execution_disposition(ExecutionDisposition::SemanticCompleted);
        let waiting = explain_execution_disposition(ExecutionDisposition::QuiescentAwaitingInput);
        let exhausted = explain_execution_disposition(ExecutionDisposition::WorkBudgetExhausted);

        assert!(completed.semantic_completion);
        assert!(!waiting.semantic_completion);
        assert!(!exhausted.semantic_completion);
        assert!(waiting.summary.contains("awaiting input"));
        assert!(exhausted.summary.contains("before semantic completion"));
        assert_ne!(completed.disposition, waiting.disposition);
        assert_ne!(completed.disposition, exhausted.disposition);
    }

    #[test]
    fn patchbay_explains_semantic_and_embodiment_overflow_separately() {
        let domain = explain_execution_disposition(ExecutionDisposition::ValueDomainOverflow);
        let state = explain_execution_disposition(ExecutionDisposition::StateCapacityExhausted);
        let resource =
            explain_execution_disposition(ExecutionDisposition::ResourceCapacityExhausted);

        assert!(domain.summary.contains("semantic domain"));
        assert!(state.summary.contains("State capacity"));
        assert!(resource.summary.contains("non-State resource"));
        assert_ne!(domain.disposition, state.disposition);
        assert_ne!(state.disposition, resource.disposition);
    }
}
