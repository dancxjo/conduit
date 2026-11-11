//! Actual attempt outcomes and optional model-derived responses.
use crate::*;

#[cfg(feature = "conversation-flow")]
pub(crate) fn decode_outcome(encoded: &[u8]) -> Result<EpisodeOutcome, EpisodeRefusal> {
    use conduit_ai::{ModelDerivedResult, ModelResultDisposition, ModelResultProvenance};
    if encoded.len() > MAXIMUM_EPISODE_RESULT_BYTES {
        return Err(EpisodeRefusal::ResponseBoundExceeded);
    }
    let result: ModelDerivedResult =
        serde_json::from_slice(encoded).map_err(|_| EpisodeRefusal::ResponseMalformed)?;
    if serde_json::to_vec(&result).map_err(|_| EpisodeRefusal::Encoding)? != encoded {
        return Err(EpisodeRefusal::ResponseMalformed);
    }
    let contract = conduit_ai::llm_contract(conduit_ai::LLM_GENERATE_FLOW_KIND)
        .ok_or(EpisodeRefusal::Encoding)?;
    result
        .validate(&contract)
        .map_err(|_| EpisodeRefusal::ResponseMalformed)?;
    let ModelResultProvenance::ModelDerived = result.provenance;
    let truncated = match result.disposition {
        ModelResultDisposition::Produced => false,
        ModelResultDisposition::Truncated => true,
        ModelResultDisposition::ProviderLost => {
            return EpisodeOutcome::provider_lost(
                result.request_identity,
                result.implementation_identity,
            )
            .map_err(|_| EpisodeRefusal::ResponseBoundExceeded);
        }
        _ => return Err(EpisodeRefusal::ResponseNotProduced),
    };
    let content =
        core::str::from_utf8(&result.payload).map_err(|_| EpisodeRefusal::ResponseMalformed)?;
    EpisodeModelResponse::new(
        content.into(),
        result.implementation_identity,
        result.run_identity,
        truncated,
    )
    .and_then(EpisodeOutcome::produced)
    .map_err(|_| EpisodeRefusal::ResponseBoundExceeded)
}

impl EpisodeRecord {
    pub fn response(&self) -> Option<&EpisodeModelResponse> {
        match self.outcome() {
            EpisodeOutcome::Produced(outcome) => Some(outcome.response()),
            EpisodeOutcome::ProviderLost(_) => None,
        }
    }
}

impl EpisodeMemory {
    pub fn response(&self) -> Option<&EpisodeModelResponse> {
        match self.outcome() {
            EpisodeOutcome::Produced(outcome) => Some(outcome.response()),
            EpisodeOutcome::ProviderLost(_) => None,
        }
    }
}
