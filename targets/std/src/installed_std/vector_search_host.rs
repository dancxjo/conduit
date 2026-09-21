//! Host dispatch for the portable vector-search operation.

use crate::hosted_vector_search::{HostedVectorSearchAdapter, HostedVectorSearchTerminal};
use conduit_core::PlannedGear;
use conduit_kernel::{BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome};

pub(super) enum Completion {
    Output,
    Refused,
    Failed,
    Cancelled,
    ProviderLost,
    QueueFull,
    MalformedInput,
}

impl Completion {
    pub(super) const fn has_output(&self) -> bool {
        matches!(self, Self::Output)
    }

    pub(super) fn outcome(self, output: Option<BoundedValueRef>) -> HostCallOutcome {
        let (disposition, failure) = match self {
            Self::Output => (HostCallDisposition::Completed, None),
            Self::Refused | Self::QueueFull => (HostCallDisposition::Denied, None),
            Self::Cancelled => (HostCallDisposition::Cancelled, None),
            Self::Failed => (HostCallDisposition::Failed, failure(70)),
            Self::ProviderLost => (HostCallDisposition::Failed, failure(71)),
            Self::MalformedInput => (
                HostCallDisposition::Failed,
                Some(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 72,
                }),
            ),
        };
        HostCallOutcome {
            disposition,
            output,
            failure,
        }
    }
}

pub(super) fn execute(
    placement: &PlannedGear,
    input: &[u8],
    adapter: Option<&mut dyn HostedVectorSearchAdapter>,
    output: &mut Vec<u8>,
) -> Result<Completion, String> {
    super::vector_search_operation::validate(placement)?;
    let Some(adapter) = adapter else {
        return Ok(Completion::Refused);
    };
    let offer = adapter.capability_offer();
    if offer.capability_id != placement.capability_id
        || offer.kind_id != placement.kind_id
        || offer.implementation.implementation_id != placement.implementation_id
        || offer.implementation.artifact_id != placement.artifact_id
        || offer.implementation.execution_profile_id != placement.execution_profile_id
    {
        return Ok(Completion::Refused);
    }
    Ok(match adapter.execute(placement, input, output) {
        HostedVectorSearchTerminal::Produced => Completion::Output,
        HostedVectorSearchTerminal::Refused => Completion::Refused,
        HostedVectorSearchTerminal::Failed => Completion::Failed,
        HostedVectorSearchTerminal::Cancelled => Completion::Cancelled,
        HostedVectorSearchTerminal::ProviderLost => Completion::ProviderLost,
        HostedVectorSearchTerminal::QueueFull => Completion::QueueFull,
        HostedVectorSearchTerminal::MalformedInput => Completion::MalformedInput,
    })
}

fn failure(detail: u16) -> Option<Failure> {
    Some(Failure {
        code: FailureCode::HostCallFailed,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_loss_and_malformed_input_remain_distinct() {
        let lost = Completion::ProviderLost.outcome(None);
        let malformed = Completion::MalformedInput.outcome(None);
        assert_eq!(lost.failure.unwrap().detail, 71);
        assert_eq!(malformed.failure.unwrap().code, FailureCode::InvalidInput);
        assert_eq!(malformed.failure.unwrap().detail, 72);
    }
}
