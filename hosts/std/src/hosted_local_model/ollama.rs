use super::{HostedLocalModelAdapter, LocalModelAdapterTerminal};
use conduit_ai::{
    LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelIdentity,
    LocalModelKindProfile, LocalModelLifecycleState, LocalModelLimits, LocalModelOffer,
    ModelDerivedResult, ModelResultDisposition, ModelResultProvenance, ModelWorkAccounting,
};
use conduit_core::PlannedGear;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

const OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
const MAXIMUM_INVENTORY_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_RUNTIME_IDENTITY_BYTES: usize = 4 * 1024;
const REQUEST_TIMEOUT_SECONDS: &str = "120";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OllamaDiscovery {
    pub runtime_version: String,
    pub model_name: String,
    pub model_content_identity: String,
    pub model_bytes: u64,
    pub architecture: String,
    pub parameter_profile: String,
    pub quantization: String,
    pub context_length: u64,
    pub completion_supported: bool,
}

pub struct OllamaLocalModelAdapter {
    offer: LocalModelOffer,
    model_name: String,
    next_request_sequence: u64,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    digest: String,
    size: u64,
    details: ModelDetails,
}

#[derive(Deserialize)]
struct ModelDetails {
    family: String,
    parameter_size: String,
    quantization_level: String,
}

#[derive(Deserialize)]
struct ShowResponse {
    model_info: serde_json::Map<String, serde_json::Value>,
    capabilities: Vec<String>,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    #[serde(default)]
    done_reason: String,
    #[serde(default)]
    prompt_eval_count: u64,
    #[serde(default)]
    eval_count: u64,
}

impl OllamaDiscovery {
    pub fn discover(model: &str) -> Result<Self, String> {
        if model.is_empty() || model.len() > conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES {
            return Err("local model name is empty or exceeds the identity bound".to_string());
        }
        let runtime_version =
            bounded_command("ollama", &["--version"], MAXIMUM_RUNTIME_IDENTITY_BYTES)?;
        let tags: TagsResponse = serde_json::from_slice(&curl_json("/api/tags", None)?)
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
        let show: ShowResponse = serde_json::from_slice(&curl_json("/api/show", Some(&show_body))?)
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
            runtime_version: runtime_version.trim().to_string(),
            model_name: selected.name,
            model_content_identity: selected.digest,
            model_bytes: selected.size,
            architecture,
            parameter_profile: selected.details.parameter_size,
            quantization: selected.details.quantization_level,
            context_length,
            completion_supported: show.capabilities.iter().any(|value| value == "completion"),
        })
    }

    pub fn initialize(
        self,
        admitted_memory_mib: u32,
        profiles: Vec<LocalModelKindProfile>,
    ) -> Result<OllamaLocalModelAdapter, String> {
        if !self.completion_supported {
            return Err("local model does not advertise completion capability".to_string());
        }
        let work = LlmWorkBounds {
            maximum_input_bytes: 4_096,
            maximum_context_items: 1,
            maximum_output_bytes: 4_096,
            maximum_work_units: self.context_length.min(4_096),
            maximum_history_items: 0,
        };
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
                maximum_in_flight: 1,
                maximum_queue_items: 4,
                maximum_queue_bytes: (work.maximum_input_bytes * 4) as u32,
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
            model_name: self.model_name,
            next_request_sequence: 1,
        };
        let warmup = adapter.generate("Reply with one word.", 1)?;
        if warmup.response.is_empty() {
            return Err("local model warmup produced no output".to_string());
        }
        Ok(adapter)
    }
}

impl OllamaLocalModelAdapter {
    fn generate(&self, input: &str, maximum_tokens: u64) -> Result<GenerateResponse, String> {
        let body = serde_json::to_vec(&json!({
            "model": self.model_name,
            "prompt": input,
            "stream": false,
            "keep_alive": "5m",
            "options": { "num_predict": maximum_tokens }
        }))
        .map_err(|error| error.to_string())?;
        serde_json::from_slice(&curl_json("/api/generate", Some(&body))?)
            .map_err(|error| format!("decode local Ollama inference: {error}"))
    }
}

impl HostedLocalModelAdapter for OllamaLocalModelAdapter {
    fn offer(&self) -> &LocalModelOffer {
        &self.offer
    }

    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        output.clear();
        let Ok(input) = std::str::from_utf8(input) else {
            return LocalModelAdapterTerminal::Failed;
        };
        let maximum_output_bytes = configuration_count(placement, "maximum-output-bytes")
            .unwrap_or(self.offer.limits.work.maximum_output_bytes);
        // The portable result slot carries payload plus exact provenance/accounting.
        // Reserve bounded envelope headroom instead of asking the provider to fill it.
        let maximum_tokens = maximum_output_bytes
            .saturating_sub(2_048)
            .checked_div(8)
            .unwrap_or(1)
            .clamp(1, 64);
        let generated = match self.generate(input, maximum_tokens) {
            Ok(generated) => generated,
            Err(_) => return LocalModelAdapterTerminal::ProviderLost,
        };
        let truncated = generated.done_reason == "length";
        let payload = generated.response.into_bytes();
        if payload.is_empty() || payload.len() as u64 > maximum_output_bytes {
            return LocalModelAdapterTerminal::Failed;
        }
        let sequence = self.next_request_sequence;
        self.next_request_sequence = self.next_request_sequence.saturating_add(1);
        let contract = match conduit_ai::llm_contract(placement.kind_id.as_str()) {
            Some(contract) => contract,
            None => return LocalModelAdapterTerminal::Refused,
        };
        let result = ModelDerivedResult {
            provenance: ModelResultProvenance::ModelDerived,
            payload_kind: contract.result_payload_kind.as_str().into(),
            accounting: ModelWorkAccounting {
                input_bytes: input.len() as u64,
                context_items: 1,
                output_bytes: payload.len() as u64,
                work_units: generated
                    .prompt_eval_count
                    .saturating_add(generated.eval_count),
                history_items: 0,
            },
            payload,
            implementation_identity: format!(
                "{}/{}",
                conduit_ai::LOCAL_MODEL_IMPLEMENTATION,
                self.offer.identity.model_content_identity
            ),
            request_identity: format!("request/local-model/{sequence}"),
            run_identity: format!("run/local-model/{sequence}"),
            confidence: None,
            disposition: if truncated {
                ModelResultDisposition::Truncated
            } else {
                ModelResultDisposition::Produced
            },
            determinism: self.offer.determinism,
        };
        if result.validate(&contract).is_err() {
            return LocalModelAdapterTerminal::Failed;
        }
        match serde_json::to_vec(&result) {
            Ok(encoded) if encoded.len() as u64 <= maximum_output_bytes => {
                output.extend_from_slice(&encoded);
                if truncated {
                    LocalModelAdapterTerminal::Truncated
                } else {
                    LocalModelAdapterTerminal::Produced
                }
            }
            _ => LocalModelAdapterTerminal::Failed,
        }
    }
}

fn configuration_count(placement: &PlannedGear, key: &str) -> Option<u64> {
    placement.configuration.iter().find_map(|entry| {
        (entry.key == key)
            .then_some(&entry.value)
            .and_then(|value| match value {
                conduit_core::ConfigurationValue::U64(value) => Some(*value),
                _ => None,
            })
    })
}

fn model_names_match(candidate: &str, requested: &str) -> bool {
    candidate == requested || candidate.strip_suffix(":latest") == Some(requested)
}

fn curl_json(path: &str, body: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let url = format!("{OLLAMA_ENDPOINT}{path}");
    let mut command = Command::new("curl");
    command.args([
        "--silent",
        "--show-error",
        "--fail",
        "--max-time",
        REQUEST_TIMEOUT_SECONDS,
        "--header",
        "content-type: application/json",
    ]);
    if body.is_some() {
        command.args(["--data-binary", "@-"]);
    }
    command.arg(url);
    bounded_child(command, body, MAXIMUM_INVENTORY_BYTES)
}

fn bounded_command(program: &str, args: &[&str], maximum: usize) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(args);
    let bytes = bounded_child(command, None, maximum)?;
    String::from_utf8(bytes).map_err(|_| format!("{program} output is not UTF-8"))
}

fn bounded_child(
    mut command: Command,
    input: Option<&[u8]>,
    maximum: usize,
) -> Result<Vec<u8>, String> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input).map_err(|error| error.to_string())?;
    }
    let mut output = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| "bounded command has no stdout".to_string())?
        .take((maximum + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|error| error.to_string())?;
    if output.len() > maximum {
        let _ = child.kill();
        let _ = child.wait();
        return Err("bounded command output exceeded its admitted ceiling".to_string());
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("bounded command exited with {status}"));
    }
    Ok(output)
}
