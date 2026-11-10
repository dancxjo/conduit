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
    attempt: (&str, conduit_kernel::RequestId),
    local_model: Option<&mut (dyn HostedLocalModelAdapter + 'static)>,
    output: &mut Vec<u8>,
) -> Result<ModelHostCompletion, String> {
    output.clear();
    if contract != conduit_ai::LOCAL_MODEL_OPERATION {
        return Err("model host received an unsupported operation".to_string());
    }
    super::local_model_back::validate(placement)?;
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
            LocalModelStreamStep::Terminal(evidence) => match *evidence.terminal() {
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
    let terminal = adapter.execute(placement, input, output);
    if terminal == LocalModelAdapterTerminal::ProviderLost
        && placement.kind_id.as_str() == conduit_ai::LLM_GENERATE_FLOW_KIND
        && super::local_model_back::configuration_count(
            placement,
            conduit_ai::PROVIDER_LOSS_AS_RESULT_PARAMETER,
        ) == Ok(1)
    {
        return provider_loss_result(placement, attempt, input.len(), output);
    }
    Ok(match terminal {
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

fn provider_loss_result(
    placement: &PlannedGear,
    (play_id, request): (&str, conduit_kernel::RequestId),
    input_bytes: usize,
    output: &mut Vec<u8>,
) -> Result<ModelHostCompletion, String> {
    use conduit_ai::*;
    use sha2::{Digest, Sha256};
    let attempt = format!(
        "host-attempt/{:x}",
        Sha256::digest(
            format!(
                "{}:{}:{}",
                play_id,
                placement.placement_id.as_str(),
                request.0
            )
            .as_bytes()
        )
    );
    let result = ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: GENERATED_RESULT_VALUE_KIND.into(),
        payload: Vec::new(),
        implementation_identity: placement.implementation_id.as_str().into(),
        request_identity: attempt.clone(),
        run_identity: attempt,
        confidence: None,
        disposition: ModelResultDisposition::ProviderLost,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
        accounting: ModelWorkAccounting {
            input_bytes: input_bytes as u64,
            context_items: 0,
            output_bytes: 0,
            work_units: 0,
            history_items: 0,
        },
    };
    let encoded = serde_json::to_vec(&result).map_err(|error| error.to_string())?;
    let limit = super::local_model_back::configuration_count(placement, "maximum-output-bytes")?;
    if encoded.len() as u64 > limit {
        return Ok(ModelHostCompletion::ProviderLost);
    }
    output.clear();
    output.extend_from_slice(&encoded);
    Ok(ModelHostCompletion::Output)
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
