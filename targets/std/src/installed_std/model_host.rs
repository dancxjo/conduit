//! Host dispatch shared by the fenced legacy fixture and L0 local-model realization.

use crate::hosted_local_model::{
    HostedLocalModelAdapter, LocalModelAdapterTerminal, LocalModelStreamStep,
};
use conduit_core::PlannedGear;
use conduit_kernel::{BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome};

pub(super) enum ModelHostCompletion {
    Output,
    StreamComplete,
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

    pub(super) fn outcome(self, output: Option<BoundedValueRef>) -> HostCallOutcome {
        let (disposition, failure) = match self {
            Self::Output => (HostCallDisposition::Completed, None),
            Self::StreamComplete => (HostCallDisposition::Completed, None),
            Self::Refused => (HostCallDisposition::Denied, None),
            Self::Failed => (
                HostCallDisposition::Failed,
                Some(Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 53,
                }),
            ),
            Self::Cancelled => (HostCallDisposition::Cancelled, None),
            Self::ProviderLost => (
                HostCallDisposition::Failed,
                Some(Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 54,
                }),
            ),
            Self::InvalidStructuredResult => (
                HostCallDisposition::Failed,
                Some(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 55,
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
    contract: &str,
    placement: &PlannedGear,
    input: &[u8],
    local_model: Option<&mut (dyn HostedLocalModelAdapter + 'static)>,
    output: &mut Vec<u8>,
) -> Result<ModelHostCompletion, String> {
    output.clear();
    if contract == conduit_ai::GENERATE_TEXT_HOST_CALL {
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
    if placement.kind_id.as_str() == conduit_ai::LLM_STREAM_GENERATE_KIND {
        return Ok(match adapter.execute_stream_step(placement, input) {
            LocalModelStreamStep::Chunk(chunk) => {
                match conduit_ai::encode_generated_text_chunk(&chunk) {
                    Ok(encoded) => {
                        output.extend_from_slice(&encoded);
                        ModelHostCompletion::Output
                    }
                    Err(_) => ModelHostCompletion::InvalidStructuredResult,
                }
            }
            LocalModelStreamStep::Terminal(evidence) => match evidence.terminal {
                conduit_ai::GeneratedTextFlowTerminal::Completed => {
                    ModelHostCompletion::StreamComplete
                }
                conduit_ai::GeneratedTextFlowTerminal::Cancelled => ModelHostCompletion::Cancelled,
                conduit_ai::GeneratedTextFlowTerminal::ProviderLost => {
                    ModelHostCompletion::ProviderLost
                }
                _ => ModelHostCompletion::Failed,
            },
        });
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
        assert_eq!(malformed.disposition, HostCallDisposition::Failed);
        assert_eq!(lost.disposition, HostCallDisposition::Failed);
        assert_eq!(malformed.failure.unwrap().detail, 55);
        assert_eq!(lost.failure.unwrap().detail, 54);
    }
}
