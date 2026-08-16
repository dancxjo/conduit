//! Host dispatch shared by the fenced legacy fixture and L0 local-model realization.

use crate::hosted_local_model::{HostedLocalModelAdapter, LocalModelAdapterTerminal};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationOutcome,
};

pub(super) enum ModelHostCompletion {
    Output,
    Refused,
    Failed,
    Cancelled,
    ProviderLost,
    InvalidStructuredResult,
}

impl ModelHostCompletion {
    pub(super) const fn has_output(&self) -> bool {
        matches!(self, Self::Output)
    }

    pub(super) fn outcome(self, output: Option<BoundedValueRef>) -> HostOperationOutcome {
        let (disposition, failure) = match self {
            Self::Output => (HostOperationDisposition::Completed, None),
            Self::Refused => (HostOperationDisposition::Denied, None),
            Self::Failed => (
                HostOperationDisposition::Failed,
                Some(Failure {
                    code: FailureCode::HostOperationFailed,
                    detail: 53,
                }),
            ),
            Self::Cancelled => (HostOperationDisposition::Cancelled, None),
            Self::ProviderLost => (
                HostOperationDisposition::Failed,
                Some(Failure {
                    code: FailureCode::HostOperationFailed,
                    detail: 54,
                }),
            ),
            Self::InvalidStructuredResult => (
                HostOperationDisposition::Failed,
                Some(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 55,
                }),
            ),
        };
        HostOperationOutcome {
            disposition,
            output,
            failure,
        }
    }
}

pub(super) fn execute(
    contract: &str,
    placement: &PlannedGear,
    input: &[u8],
    local_model: Option<&mut dyn HostedLocalModelAdapter>,
    output: &mut Vec<u8>,
) -> Result<ModelHostCompletion, String> {
    output.clear();
    if contract == conduit_ai::GENERATE_TEXT_HOST_OPERATION {
        super::generate_text::execute_fixture(placement, input, output)?;
        return Ok(ModelHostCompletion::Output);
    }
    if contract != conduit_ai::LOCAL_MODEL_OPERATION {
        return Err("model host received an unsupported operation".to_string());
    }
    super::local_model_operation::validate(placement)?;
    let Some(adapter) = local_model else {
        return Ok(ModelHostCompletion::Refused);
    };
    if !adapter
        .offer()
        .capability_offers()
        .map_err(|error| format!("active local-model offer: {error:?}"))?
        .iter()
        .any(|offer| {
            offer.kind_id == placement.kind_id
                && offer.implementation.artifact_id == placement.artifact_id
        })
    {
        return Ok(ModelHostCompletion::Refused);
    }
    Ok(match adapter.execute(placement, input, output) {
        LocalModelAdapterTerminal::Produced | LocalModelAdapterTerminal::Truncated => {
            ModelHostCompletion::Output
        }
        LocalModelAdapterTerminal::Refused => ModelHostCompletion::Refused,
        LocalModelAdapterTerminal::Failed => ModelHostCompletion::Failed,
        LocalModelAdapterTerminal::Cancelled => ModelHostCompletion::Cancelled,
        LocalModelAdapterTerminal::ProviderLost => ModelHostCompletion::ProviderLost,
        LocalModelAdapterTerminal::InvalidStructuredResult => {
            ModelHostCompletion::InvalidStructuredResult
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_structure_and_provider_loss_keep_distinct_machine_details() {
        let malformed = ModelHostCompletion::InvalidStructuredResult.outcome(None);
        let lost = ModelHostCompletion::ProviderLost.outcome(None);
        assert_eq!(malformed.disposition, HostOperationDisposition::Failed);
        assert_eq!(lost.disposition, HostOperationDisposition::Failed);
        assert_eq!(malformed.failure.unwrap().detail, 55);
        assert_eq!(lost.failure.unwrap().detail, 54);
    }
}
