//! Validated projection from a model-derived generation envelope to bounded text.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindId, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal,
};

use crate::{
    llm_contract, GeneratedTextChunk, ModelDerivedResult, ModelResultDisposition,
    GENERATED_RESULT_VALUE_KIND, GENERATED_TEXT_CHUNK_VALUE_KIND, LLM_GENERATE_KIND,
    MAXIMUM_GENERATED_TEXT_CHUNK_BYTES, MAXIMUM_GENERATED_TEXT_IN_FLIGHT_ITEMS, TEXT_VALUE_KIND,
};

pub const MODEL_RESULT_TO_TEXT_KIND: &str = "llm/result-to-text";
pub const MODEL_RESULT_FLOW_TO_TEXT_KIND: &str = "llm/result-flow-to-text";
pub const MODEL_RESULT_TO_TEXT_REVISION: &str = "conduit.llm/result-to-text@1";
pub const MODEL_RESULT_FLOW_TO_TEXT_REVISION: &str = "conduit.llm/result-flow-to-text@1";
pub const GENERATED_CHUNK_TO_TEXT_KIND: &str = "llm/generated-chunk-to-text";
pub const GENERATED_CHUNK_TO_TEXT_REVISION: &str = "conduit.llm/generated-chunk-to-text@1";
pub const MAXIMUM_MODEL_RESULT_ENVELOPE_BYTES: u32 = 65_536;
pub const MAXIMUM_MODEL_TEXT_BYTES: u32 = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelTextContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

impl ModelTextContract {
    pub fn into_semantic_capability_contract(self) -> Kind {
        Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: self.kind_id,
            kind_contract_revision: self.kind_contract_revision,
            inputs: self.inputs,
            outputs: self.outputs,
            configuration: Default::default(),
            semantic_laws: Default::default(),
            limits: self.limits,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelTextRefusal {
    EnvelopeBoundExceeded,
    MalformedEnvelope,
    NonCanonicalEnvelope,
    InvalidGenerationResult,
    NotProduced,
    InvalidUtf8,
    EmptyText,
    TextBoundExceeded,
}

pub fn model_result_to_text_contract() -> ModelTextContract {
    model_text_contract(
        MODEL_RESULT_TO_TEXT_KIND,
        MODEL_RESULT_TO_TEXT_REVISION,
        PortTemporal::Value,
    )
}

pub fn model_result_flow_to_text_contract() -> ModelTextContract {
    model_text_contract(
        MODEL_RESULT_FLOW_TO_TEXT_KIND,
        MODEL_RESULT_FLOW_TO_TEXT_REVISION,
        PortTemporal::Flow { closes: true },
    )
}

pub fn generated_chunk_to_text_contract() -> ModelTextContract {
    ModelTextContract {
        kind_id: kind_id(GENERATED_CHUNK_TO_TEXT_KIND),
        kind_contract_revision: KindIdentity::from(GENERATED_CHUNK_TO_TEXT_REVISION),
        inputs: vec![port_with_temporal(
            "chunk",
            GENERATED_TEXT_CHUNK_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port_with_temporal(
            "text",
            TEXT_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_GENERATED_TEXT_IN_FLIGHT_ITEMS,
            max_queue_bytes: crate::MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES as u32,
        },
    }
}

pub fn project_encoded_generated_chunk_text(encoded: &[u8]) -> Result<Vec<u8>, ModelTextRefusal> {
    let chunk = crate::decode_generated_text_chunk(encoded)
        .map_err(|_| ModelTextRefusal::MalformedEnvelope)?;
    project_generated_chunk_text(&chunk).map(|text| text.as_bytes().to_vec())
}

pub fn project_generated_chunk_text(chunk: &GeneratedTextChunk) -> Result<&str, ModelTextRefusal> {
    if chunk.text.is_empty() {
        return Err(ModelTextRefusal::EmptyText);
    }
    if chunk.text.len() > MAXIMUM_GENERATED_TEXT_CHUNK_BYTES {
        return Err(ModelTextRefusal::TextBoundExceeded);
    }
    Ok(&chunk.text)
}

fn model_text_contract(kind: &str, revision: &str, temporal: PortTemporal) -> ModelTextContract {
    ModelTextContract {
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        inputs: vec![port_with_temporal(
            "result",
            GENERATED_RESULT_VALUE_KIND,
            PortDirection::Input,
            temporal,
        )],
        outputs: vec![port_with_temporal(
            "text",
            TEXT_VALUE_KIND,
            PortDirection::Output,
            temporal,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_MODEL_RESULT_ENVELOPE_BYTES,
        },
    }
}

pub fn project_generated_text(encoded: &[u8]) -> Result<Vec<u8>, ModelTextRefusal> {
    if encoded.len() > MAXIMUM_MODEL_RESULT_ENVELOPE_BYTES as usize {
        return Err(ModelTextRefusal::EnvelopeBoundExceeded);
    }
    let result: ModelDerivedResult =
        serde_json::from_slice(encoded).map_err(|_| ModelTextRefusal::MalformedEnvelope)?;
    let canonical = serde_json::to_vec(&result).map_err(|_| ModelTextRefusal::MalformedEnvelope)?;
    if canonical != encoded {
        return Err(ModelTextRefusal::NonCanonicalEnvelope);
    }
    let contract = llm_contract(LLM_GENERATE_KIND).expect("generation contract is catalogued");
    result
        .validate(&contract)
        .map_err(|_| ModelTextRefusal::InvalidGenerationResult)?;
    if result.disposition != ModelResultDisposition::Produced {
        return Err(ModelTextRefusal::NotProduced);
    }
    if result.payload.is_empty() {
        return Err(ModelTextRefusal::EmptyText);
    }
    core::str::from_utf8(&result.payload).map_err(|_| ModelTextRefusal::InvalidUtf8)?;
    if result.payload.len() > MAXIMUM_MODEL_TEXT_BYTES as usize {
        return Err(ModelTextRefusal::TextBoundExceeded);
    }
    Ok(result.payload)
}

#[cfg(feature = "form-catalog")]
pub fn install_model_text_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{KindProjection, KindSignature};

    for contract in [
        model_result_to_text_contract(),
        model_result_flow_to_text_contract(),
        generated_chunk_to_text_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindProjection {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn port_with_temporal(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LlmDeterminismProfile, ModelResultProvenance, ModelWorkAccounting};

    fn encoded(payload: &[u8], disposition: ModelResultDisposition) -> Vec<u8> {
        serde_json::to_vec(&ModelDerivedResult {
            provenance: ModelResultProvenance::ModelDerived,
            payload_kind: GENERATED_RESULT_VALUE_KIND.into(),
            payload: payload.to_vec(),
            implementation_identity: "model/fixture".into(),
            request_identity: "request/fixture".into(),
            run_identity: "run/fixture".into(),
            confidence: None,
            disposition,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
            accounting: ModelWorkAccounting {
                input_bytes: 4,
                context_items: 1,
                output_bytes: payload.len() as u64,
                work_units: 8,
                history_items: 0,
            },
        })
        .unwrap()
    }

    #[test]
    fn only_canonical_produced_bounded_utf8_becomes_text() {
        let value = encoded(
            b"The upstairs temperature is 21 degrees Celsius.",
            ModelResultDisposition::Produced,
        );
        assert_eq!(
            project_generated_text(&value),
            Ok(b"The upstairs temperature is 21 degrees Celsius.".to_vec())
        );
        assert_eq!(
            project_generated_text(&encoded(b"partial", ModelResultDisposition::Truncated)),
            Err(ModelTextRefusal::NotProduced)
        );
        assert_eq!(
            project_generated_text(&encoded(&[0xff], ModelResultDisposition::Produced)),
            Err(ModelTextRefusal::InvalidUtf8)
        );
        assert_eq!(
            project_generated_text(&encoded(&vec![b'x'; 257], ModelResultDisposition::Produced),),
            Err(ModelTextRefusal::TextBoundExceeded)
        );
    }

    #[test]
    fn generated_chunks_project_as_incremental_text_without_batching() {
        let chunk = GeneratedTextChunk {
            sequence: 3,
            text: "incremental".into(),
        };
        assert_eq!(project_generated_chunk_text(&chunk), Ok("incremental"));
        assert_eq!(
            project_generated_chunk_text(&GeneratedTextChunk {
                sequence: 4,
                text: String::new(),
            }),
            Err(ModelTextRefusal::EmptyText)
        );
    }
}
