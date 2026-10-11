//! Existing local discovery and finite offer initialization.
use super::*;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OllamaDiscovery {
    pub endpoint: String,
    pub runtime_version: String,
    pub model_name: String,
    pub model_content_identity: String,
    pub model_bytes: u64,
    pub architecture: String,
    pub parameter_profile: String,
    pub quantization: String,
    pub context_length: u64,
    pub completion_supported: bool,
    pub embedding_supported: bool,
    pub vision_supported: bool,
}

impl OllamaDiscovery {
    pub fn discover(model: &str) -> Result<Self, String> {
        Self::discover_at(DEFAULT_OLLAMA_ENDPOINT, model)
    }

    pub fn discover_at(endpoint: &str, model: &str) -> Result<Self, String> {
        validate_endpoint(endpoint)?;
        if model.is_empty() || model.len() > conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES {
            return Err("local model name is empty or exceeds the identity bound".to_string());
        }
        let runtime_version =
            bounded_command("ollama", &["--version"], MAXIMUM_RUNTIME_IDENTITY_BYTES)?;
        let tags: TagsResponse =
            serde_json::from_slice(&curl_json(endpoint, "/api/tags", None)?)
                .map_err(|error| format!("decode local Ollama inventory: {error}"))?;
        let selected = tags
            .models
            .into_iter()
            .find(|candidate| model_names_match(&candidate.name, model))
            .ok_or_else(|| {
                "selected model is not already local; automatic download is forbidden".to_string()
            })?;
        let show_body = serde_json::to_vec(&json!({ "model": selected.name }))
            .map_err(|error| error.to_string())?;
        let show: ShowResponse =
            serde_json::from_slice(&curl_json(endpoint, "/api/show", Some(&show_body))?)
                .map_err(|error| format!("decode local Ollama model metadata: {error}"))?;
        let architecture = show
            .model_info
            .get("general.architecture")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&selected.details.family)
            .to_string();
        let context_key = format!("{architecture}.context_length");
        let context_length = show
            .model_info
            .get(&context_key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "local model metadata has no exact context length".to_string())?;
        Ok(Self {
            endpoint: endpoint.into(),
            runtime_version: runtime_version.trim().to_string(),
            model_name: selected.name,
            model_content_identity: selected.digest,
            model_bytes: selected.size,
            architecture,
            parameter_profile: selected.details.parameter_size,
            quantization: selected.details.quantization_level,
            context_length,
            completion_supported: show.capabilities.iter().any(|value| value == "completion"),
            embedding_supported: show.capabilities.iter().any(|value| value == "embedding"),
            vision_supported: show.capabilities.iter().any(|value| value == "vision"),
        })
    }

    pub fn initialize(
        self,
        admitted_memory_mib: u32,
        mut profiles: Vec<LocalModelKindProfile>,
    ) -> Result<OllamaLocalModelAdapter, String> {
        if profiles.contains(&LocalModelKindProfile::Generate)
            && !profiles.contains(&LocalModelKindProfile::GenerateFlow)
        {
            profiles.push(LocalModelKindProfile::GenerateFlow);
        }
        let needs_completion = profiles
            .iter()
            .any(|profile| !matches!(profile, LocalModelKindProfile::EmbedFiniteVector));
        if needs_completion && !self.completion_supported {
            return Err("local model does not advertise completion capability".to_string());
        }
        if profiles.contains(&LocalModelKindProfile::EmbedFiniteVector) && !self.embedding_supported
        {
            return Err("local model does not advertise embedding capability".to_string());
        }
        // A byte can require at most one byte-fallback token, so admitting no
        // more input bytes than the discovered context length is conservative
        // without pretending to know the provider's private tokenizer.
        let maximum_input_bytes = self
            .context_length
            .clamp(4_096, conduit_ai::MAXIMUM_LLM_INPUT_BYTES);
        let work = LlmWorkBounds::new(
            maximum_input_bytes,
            1,
            4_096,
            self.context_length.min(conduit_ai::MAXIMUM_LLM_WORK_UNITS),
            0,
        )
        .expect("discovered local-model bounds are clamped to native limits");
        let offer = LocalModelOffer {
            identity: LocalModelIdentity {
                runtime_name: "ollama".into(),
                runtime_version: self.runtime_version.clone(),
                runtime_build_identity: format!("ollama-local-api@1/{}", self.runtime_version),
                model_name: self.model_name.clone(),
                model_content_identity: self.model_content_identity.clone(),
                architecture: self.architecture,
                parameter_profile: self.parameter_profile,
                quantization: self.quantization,
            },
            limits: LocalModelLimits {
                work,
                model_bytes: self.model_bytes,
                admitted_memory_mib,
                compute: conduit_ai::LocalModelComputeNeed {
                    minimum_lanes: 1,
                    preferred_lanes: 2,
                    maximum_lanes: 4,
                    minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
                },
                maximum_in_flight: 1,
                maximum_queue_items: 1,
                maximum_queue_bytes: (work.maximum_input_bytes() + work.maximum_output_bytes())
                    as u32,
                cancellation_supported: false,
                cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
            },
            supported_profiles: profiles,
            initialized: true,
            lifecycle: LocalModelLifecycleState::Ready,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
        };
        offer
            .validate()
            .map_err(|error| format!("local Ollama offer validation: {error:?}"))?;
        let adapter = OllamaLocalModelAdapter {
            offer,
            endpoint: self.endpoint,
            model_name: self.model_name,
            next_request_sequence: 1,
            active_stream: None,
        };
        if needs_completion {
            // One token is not enough for every completion-capable model to
            // cross its private tokenizer's control-token prefix and emit
            // visible text. Keep the probe tiny and finite, but admit enough
            // output for the provider to demonstrate a usable completion.
            let warmup = adapter.generate("Reply with one word.", WARMUP_MAXIMUM_TOKENS, false)?;
            if warmup.response.is_empty() {
                return Err("local model warmup produced no output".to_string());
            }
        }
        Ok(adapter)
    }
}
