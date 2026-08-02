use std::fs::File;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use conduit_core::{ArtifactDigest, ExecutorKind};
use conduit_panel::Node;
use conduit_runtime::{
    ExactHostedServiceBinding, Handler, InstalledArtifactRegistration,
    InstalledImplementationRegistration, Registry, RegistryError, RunIo, RuntimeError, Value,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::{
    CHAT_CONTRACT, CHAT_SCHEMA_VERSION, ChatAdmissionPool, ChatBounds, ChatError, ChatExecution,
    ChatNetworkRequirement, ChatProviderProfile, ChatProviderProfileInput, ChatReason,
    LOCAL_CHAT_AUTHORITY, execution_values, register_chat_contracts, request_from_node,
    runtime_error, validate_chat_config, validate_exact_local_binding,
};

const MAXIMUM_OBSERVATION_RESPONSE_BYTES: usize = 1024 * 1024;
const MAXIMUM_RESPONSE_FRAMING_BYTES: usize = 256 * 1024;
const MAXIMUM_HTTP_HEADER_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaObservation {
    pub endpoint: SocketAddr,
    pub binary_path: PathBuf,
    pub binary_digest: ArtifactDigest,
    pub binary_bytes: u64,
    pub server_version: String,
    pub model_name: String,
    pub model_artifact_id: String,
    pub model_digest: ArtifactDigest,
    pub model_bytes: u64,
    pub model_format: String,
    pub model_family: String,
    pub model_parameter_profile: String,
    pub model_quantization: String,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

#[derive(Clone, Debug)]
pub struct OllamaObserver {
    pub endpoint: SocketAddr,
    pub binary_path: PathBuf,
    pub model_name: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaObservationError {
    pub reason: ChatReason,
    pub message: String,
}

impl OllamaObservationError {
    fn new(reason: ChatReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for OllamaObservationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.reason.code(), self.message)
    }
}

impl std::error::Error for OllamaObservationError {}

impl OllamaObserver {
    /// Observes one explicitly named local installation without provisioning,
    /// starting a process, loading a model, or mutating the registry.
    pub fn observe(
        &self,
        observed_at_tick: u64,
        valid_until_tick: u64,
    ) -> Result<OllamaObservation, OllamaObservationError> {
        if !self.endpoint.ip().is_loopback() {
            return Err(OllamaObservationError::new(
                ChatReason::UnexpectedNetwork,
                "Ollama observation is restricted to an explicit loopback endpoint",
            ));
        }
        if observed_at_tick >= valid_until_tick {
            return Err(OllamaObservationError::new(
                ChatReason::StaleProvider,
                "Ollama observation validity window is empty",
            ));
        }
        let (binary_digest, binary_bytes) = observe_file(&self.binary_path)?;
        let version: VersionResponse = serde_json::from_slice(&http_request(
            self.endpoint,
            self.timeout,
            "GET",
            "/api/version",
            &[],
            MAXIMUM_OBSERVATION_RESPONSE_BYTES,
        )?)
        .map_err(|_| {
            OllamaObservationError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama version response is not the expected bounded JSON shape",
            )
        })?;
        let tags: TagsResponse = serde_json::from_slice(&http_request(
            self.endpoint,
            self.timeout,
            "GET",
            "/api/tags",
            &[],
            MAXIMUM_OBSERVATION_RESPONSE_BYTES,
        )?)
        .map_err(|_| {
            OllamaObservationError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama model inventory is not the expected bounded JSON shape",
            )
        })?;
        let model = tags
            .models
            .into_iter()
            .find(|candidate| {
                candidate.name == self.model_name || candidate.model == self.model_name
            })
            .ok_or_else(|| {
                OllamaObservationError::new(
                    ChatReason::ProviderUnavailable,
                    "explicitly requested Ollama model was not present",
                )
            })?;
        let model_digest = parse_sha256(&model.digest)?;
        let digest_text = model_digest.to_string();
        let suffix = digest_text
            .strip_prefix("sha256:")
            .and_then(|value| value.get(..16))
            .ok_or_else(|| {
                OllamaObservationError::new(
                    ChatReason::MalformedProviderOutput,
                    "Ollama model digest is not a canonical SHA-256 digest",
                )
            })?;
        Ok(OllamaObservation {
            endpoint: self.endpoint,
            binary_path: self.binary_path.clone(),
            binary_digest,
            binary_bytes,
            server_version: version.version,
            model_name: model.name,
            model_artifact_id: format!("conduit.ai/model/ollama-{suffix}"),
            model_digest,
            model_bytes: model.size,
            model_format: model.details.format,
            model_family: model.details.family,
            model_parameter_profile: model.details.parameter_size,
            model_quantization: model.details.quantization_level,
            observed_at_tick,
            valid_until_tick,
        })
    }
}

#[derive(Clone, Debug)]
pub struct OllamaInstallation {
    pub observation: OllamaObservation,
    pub profile: ChatProviderProfile,
    pub implementation_id: String,
}

impl OllamaInstallation {
    pub fn from_observation(
        observation: OllamaObservation,
        bounds: ChatBounds,
        maximum_concurrency: u16,
        latency_objective_millis: u64,
        latency_evidence_window_requests: u32,
    ) -> Result<Self, ChatError> {
        let profile = ChatProviderProfile::new(ChatProviderProfileInput {
            model_artifact_id: observation.model_artifact_id.clone(),
            model_artifact_digest: observation.model_digest,
            model_format: observation.model_format.clone(),
            model_family: observation.model_family.clone(),
            model_parameter_profile: observation.model_parameter_profile.clone(),
            model_quantization: observation.model_quantization.clone(),
            bounds,
            maximum_concurrency,
            network_requirement: ChatNetworkRequirement::LoopbackOnly,
            latency_objective_millis,
            latency_evidence_window_requests,
        })?;
        let digest = observation.binary_digest.to_string();
        let suffix = digest
            .strip_prefix("sha256:")
            .and_then(|value| value.get(..16))
            .ok_or_else(|| {
                ChatError::new(
                    ChatReason::UnsupportedProfile,
                    "observed Ollama binary digest is not canonical",
                )
            })?;
        Ok(Self {
            observation,
            profile,
            implementation_id: format!("conduit.ai/chat-ollama-{suffix}"),
        })
    }
}

pub fn install_observed_ollama_implementation(
    registry: &mut Registry,
    installation: OllamaInstallation,
) -> Result<(), RegistryError> {
    register_chat_contracts(registry);
    installation
        .profile
        .validate()
        .map_err(|error| RegistryError {
            code: error.reason.code(),
            message: error.message,
        })?;
    let adapter_bytes = include_bytes!("ollama.rs");
    let adapter_digest = ArtifactDigest::from_bytes(Sha256::digest(adapter_bytes).into());
    let build_recipe_digest =
        ArtifactDigest::from_bytes(Sha256::digest(b"cargo build -p conduit-ai").into());
    let implementation_id = installation.implementation_id.clone();
    let observation = installation.observation.clone();
    let profile = installation.profile.clone();
    let pool = Arc::new(
        ChatAdmissionPool::new(profile.maximum_concurrency).map_err(|error| RegistryError {
            code: error.reason.code(),
            message: error.message,
        })?,
    );
    let factory_implementation_id = implementation_id.clone();
    let factory_observation = observation.clone();
    let factory_profile = profile.clone();
    let validator_profile = profile.clone();
    registry.register_installed_implementation(InstalledImplementationRegistration {
        contract: &CHAT_CONTRACT,
        implementation_id: implementation_id.clone(),
        implementation_version: format!("ollama-{}", observation.server_version),
        executor: ExecutorKind::NativeInProcess,
        entrypoint_name: "ai-chat-ollama-loopback".to_owned(),
        entrypoint_adapter: "conduit/host-service-step".to_owned(),
        entrypoint_abi: "conduit/rust-in-process".to_owned(),
        entrypoint_protocol_version: CHAT_SCHEMA_VERSION,
        execution_profile: profile.pin(),
        artifacts: vec![
            InstalledArtifactRegistration {
                id: "conduit.ai/chat-ollama-adapter-artifact".to_owned(),
                digest: adapter_digest,
                media_type: "application/vnd.conduit.compiled-in-provider".to_owned(),
                byte_size: u64::try_from(adapter_bytes.len())
                    .expect("compiled adapter length fits u64"),
                target: Some(std::env::consts::ARCH.to_owned()),
                abi: Some("conduit/rust-in-process".to_owned()),
                builder: "conduit/rustc-workspace-build".to_owned(),
                source_digest: adapter_digest,
                build_recipe_digest,
                reproducible: true,
                license_expressions: vec!["MIT".to_owned(), "Apache-2.0".to_owned()],
                role: "adapter".to_owned(),
                required: true,
            },
            InstalledArtifactRegistration {
                id: "conduit.ai/host-artifact/ollama-server".to_owned(),
                digest: observation.binary_digest,
                media_type: "application/vnd.conduit.observed-native-executable".to_owned(),
                byte_size: observation.binary_bytes,
                target: Some(std::env::consts::ARCH.to_owned()),
                abi: None,
                builder: "conduit/explicit-host-observation".to_owned(),
                source_digest: observation.binary_digest,
                build_recipe_digest: observation.binary_digest,
                reproducible: false,
                license_expressions: Vec::new(),
                role: "server".to_owned(),
                required: true,
            },
            InstalledArtifactRegistration {
                id: observation.model_artifact_id.clone(),
                digest: observation.model_digest,
                media_type: "application/vnd.ollama.model".to_owned(),
                byte_size: observation.model_bytes,
                target: None,
                abi: None,
                builder: "conduit/explicit-host-observation".to_owned(),
                source_digest: observation.model_digest,
                build_recipe_digest: observation.model_digest,
                reproducible: false,
                license_expressions: Vec::new(),
                role: "model".to_owned(),
                required: true,
            },
        ],
        required_capabilities: vec![profile.capability_requirement(implementation_id)],
        required_authorities: vec![LOCAL_CHAT_AUTHORITY],
        required_effects: Vec::new(),
        minimum_plan_version: 0,
        maximum_plan_version: u32::MAX,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        coexistence_memory_bytes: 0,
        managed_lifecycle: None,
        factory: move || {
            Box::new(OllamaChatHandler {
                endpoint: factory_observation.endpoint,
                model_name: factory_observation.model_name.clone(),
                implementation_id: factory_implementation_id.clone(),
                profile: factory_profile.clone(),
                pool: Arc::clone(&pool),
                binding: None,
            }) as Box<dyn Handler>
        },
        validate_config: move |node: &Node| validate_chat_config(node, &validator_profile),
    })
}

struct OllamaChatHandler {
    endpoint: SocketAddr,
    model_name: String,
    implementation_id: String,
    profile: ChatProviderProfile,
    pool: Arc<ChatAdmissionPool>,
    binding: Option<ExactHostedServiceBinding>,
}

impl Handler for OllamaChatHandler {
    fn prepare(
        &mut self,
        _node: &Node,
        binding: ExactHostedServiceBinding,
    ) -> Result<(), RuntimeError> {
        validate_exact_local_binding(
            &binding,
            &self.implementation_id,
            &self.profile,
            &self.endpoint.to_string(),
        )
        .map_err(runtime_error)?;
        self.binding = Some(binding);
        Ok(())
    }

    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if self.binding.is_none() {
            return Err(runtime_error(ChatError::new(
                ChatReason::GrantDenied,
                "Ollama adapter was not prepared with an exact binding",
            )));
        }
        let _admission = self.pool.try_acquire().map_err(runtime_error)?;
        let request = request_from_node(node, inputs).map_err(runtime_error)?;
        request.validate().map_err(runtime_error)?;
        if !self.endpoint.ip().is_loopback() {
            return Err(runtime_error(ChatError::new(
                ChatReason::UnexpectedNetwork,
                "Ollama adapter is restricted to the exact loopback endpoint",
            )));
        }
        let execution = run_ollama_request(
            self.endpoint,
            &self.model_name,
            &self.implementation_id,
            &self.profile,
            &request,
        )
        .map_err(runtime_error)?;
        execution_values(execution).map_err(runtime_error)
    }
}

fn run_ollama_request(
    endpoint: SocketAddr,
    model_name: &str,
    implementation_id: &str,
    profile: &ChatProviderProfile,
    request: &super::ChatRequest,
) -> Result<ChatExecution, ChatError> {
    #[derive(Serialize)]
    struct Message<'a> {
        role: &'static str,
        content: &'a str,
    }
    #[derive(Serialize)]
    struct Body<'a> {
        model: &'a str,
        messages: Vec<Message<'a>>,
        stream: bool,
        keep_alive: u8,
    }
    let message = std::str::from_utf8(&request.message).map_err(|_| {
        ChatError::new(
            ChatReason::MalformedProviderOutput,
            "chat message is not UTF-8",
        )
    })?;
    let context = request
        .context
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| {
            ChatError::new(
                ChatReason::MalformedProviderOutput,
                "chat context is not UTF-8",
            )
        })?;
    let mut messages = Vec::with_capacity(2);
    if let Some(content) = context {
        messages.push(Message {
            role: "system",
            content,
        });
    }
    messages.push(Message {
        role: "user",
        content: message,
    });
    let body = serde_json::to_vec(&Body {
        model: model_name,
        messages,
        stream: true,
        keep_alive: 0,
    })
    .map_err(|_| {
        ChatError::new(
            ChatReason::MalformedProviderOutput,
            "bounded Ollama request could not be encoded",
        )
    })?;
    let timeout = Duration::from_millis(request.timeout_millis);
    let response = http_request(
        endpoint,
        timeout,
        "POST",
        "/api/chat",
        &body,
        MAXIMUM_RESPONSE_FRAMING_BYTES,
    )
    .map_err(|error| ChatError::new(error.reason, error.message))?;
    let mut chunks = Vec::new();
    let mut saw_terminal = false;
    for line in response.split(|byte| *byte == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let frame: ChatFrame = serde_json::from_slice(line).map_err(|_| {
            ChatError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama returned malformed NDJSON framing",
            )
        })?;
        if frame.model != model_name {
            return Err(ChatError::new(
                ChatReason::ModelMismatch,
                "Ollama response did not name the exact selected model",
            ));
        }
        if let Some(error) = frame.error {
            return Err(ChatError::new(
                ChatReason::ProviderLost,
                format!(
                    "Ollama reported a bounded provider error ({} bytes)",
                    error.len()
                ),
            ));
        }
        if let Some(message) = frame.message {
            if !message.content.is_empty() {
                chunks.push(message.content);
            }
        }
        if frame.done {
            saw_terminal = true;
        }
        if chunks.len() > usize::from(profile.maximum_chunks) {
            return Err(ChatError::new(
                ChatReason::ChunkOverflow,
                "Ollama exceeded the exact response chunk-count bound",
            ));
        }
    }
    if !saw_terminal {
        return Err(ChatError::new(
            ChatReason::ProviderLost,
            "Ollama response ended without a terminal frame",
        ));
    }
    ChatExecution::completed(chunks, profile, implementation_id)
}

#[derive(Deserialize)]
struct VersionResponse {
    version: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
    model: String,
    size: u64,
    digest: String,
    details: TagDetails,
}

#[derive(Deserialize)]
struct TagDetails {
    format: String,
    family: String,
    parameter_size: String,
    quantization_level: String,
}

#[derive(Deserialize)]
struct ChatFrame {
    model: String,
    #[serde(default)]
    message: Option<ChatFrameMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct ChatFrameMessage {
    content: String,
}

fn observe_file(path: &Path) -> Result<(ArtifactDigest, u64), OllamaObservationError> {
    let before = path.metadata().map_err(|error| {
        OllamaObservationError::new(
            ChatReason::ProviderUnavailable,
            format!("explicit Ollama binary cannot be inspected: {error}"),
        )
    })?;
    if !before.is_file() {
        return Err(OllamaObservationError::new(
            ChatReason::ProviderUnavailable,
            "explicit Ollama binary path is not a regular file",
        ));
    }
    let mut file = File::open(path).map_err(|error| {
        OllamaObservationError::new(
            ChatReason::ProviderUnavailable,
            format!("explicit Ollama binary cannot be opened: {error}"),
        )
    })?;
    let mut hasher = Sha256::new();
    let copied = std::io::copy(&mut file, &mut hasher).map_err(|error| {
        OllamaObservationError::new(
            ChatReason::ProviderLost,
            format!("explicit Ollama binary changed or became unreadable: {error}"),
        )
    })?;
    let after = path.metadata().map_err(|error| {
        OllamaObservationError::new(
            ChatReason::ProviderLost,
            format!("explicit Ollama binary disappeared after observation: {error}"),
        )
    })?;
    if copied != before.len() || before.len() != after.len() {
        return Err(OllamaObservationError::new(
            ChatReason::StaleProvider,
            "explicit Ollama binary changed during observation",
        ));
    }
    Ok((ArtifactDigest::from_bytes(hasher.finalize().into()), copied))
}

fn parse_sha256(value: &str) -> Result<ArtifactDigest, OllamaObservationError> {
    let hexadecimal = value.strip_prefix("sha256:").unwrap_or(value);
    if hexadecimal.len() != 64 || !hexadecimal.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(OllamaObservationError::new(
            ChatReason::MalformedProviderOutput,
            "Ollama model digest is not SHA-256",
        ));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in hexadecimal.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| {
            OllamaObservationError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama model digest is not ASCII hexadecimal",
            )
        })?;
        bytes[index] = u8::from_str_radix(text, 16).map_err(|_| {
            OllamaObservationError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama model digest is not hexadecimal",
            )
        })?;
    }
    Ok(ArtifactDigest::from_bytes(bytes))
}

fn http_request(
    endpoint: SocketAddr,
    timeout: Duration,
    method: &str,
    path: &str,
    body: &[u8],
    maximum_body_bytes: usize,
) -> Result<Vec<u8>, OllamaObservationError> {
    if !endpoint.ip().is_loopback()
        || matches!(endpoint.ip(), IpAddr::V6(ip) if ip.is_unicast_link_local())
    {
        return Err(OllamaObservationError::new(
            ChatReason::UnexpectedNetwork,
            "local chat attempted a non-loopback network endpoint",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&endpoint, timeout).map_err(map_io_error)?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(map_io_error)?;
    let host = match endpoint {
        SocketAddr::V4(_) => endpoint.to_string(),
        SocketAddr::V6(_) => format!("[{}]:{}", endpoint.ip(), endpoint.port()),
    };
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).map_err(map_io_error)?;
    stream.write_all(body).map_err(map_io_error)?;
    let maximum_response_bytes = maximum_body_bytes
        .checked_add(MAXIMUM_HTTP_HEADER_BYTES)
        .and_then(|value| value.checked_add(MAXIMUM_RESPONSE_FRAMING_BYTES))
        .ok_or_else(|| {
            OllamaObservationError::new(ChatReason::BoundsInvalid, "HTTP response bound overflowed")
        })?;
    let mut response = Vec::new();
    let mut limited = (&mut stream).take(
        u64::try_from(maximum_response_bytes).map_err(|_| {
            OllamaObservationError::new(
                ChatReason::BoundsInvalid,
                "HTTP response bound does not fit u64",
            )
        })? + 1,
    );
    limited.read_to_end(&mut response).map_err(map_io_error)?;
    if response.len() > maximum_response_bytes {
        return Err(OllamaObservationError::new(
            ChatReason::OutputOverflow,
            "Ollama HTTP response exceeded its framing bound",
        ));
    }
    decode_http_response(&response, maximum_body_bytes)
}

fn decode_http_response(
    response: &[u8],
    maximum_body_bytes: usize,
) -> Result<Vec<u8>, OllamaObservationError> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .ok_or_else(|| {
            OllamaObservationError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama HTTP response has no complete header",
            )
        })?;
    if header_end > MAXIMUM_HTTP_HEADER_BYTES {
        return Err(OllamaObservationError::new(
            ChatReason::OutputOverflow,
            "Ollama HTTP header exceeded its bound",
        ));
    }
    let header = std::str::from_utf8(&response[..header_end]).map_err(|_| {
        OllamaObservationError::new(
            ChatReason::MalformedProviderOutput,
            "Ollama HTTP header is not ASCII-compatible",
        )
    })?;
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_ascii_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| {
            OllamaObservationError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama HTTP response has no valid status",
            )
        })?;
    if !(200..300).contains(&status) {
        return Err(OllamaObservationError::new(
            ChatReason::ProviderUnavailable,
            format!("Ollama HTTP request failed with status {status}"),
        ));
    }
    let body = &response[header_end..];
    let decoded = if header.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.trim().eq_ignore_ascii_case("chunked")
        })
    }) {
        decode_chunked(body, maximum_body_bytes)?
    } else if let Some(length) = header.lines().find_map(|line| {
        line.split_once(':').and_then(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
    }) {
        if length > maximum_body_bytes || body.len() != length {
            return Err(OllamaObservationError::new(
                ChatReason::OutputOverflow,
                "Ollama HTTP body length violated its exact bound",
            ));
        }
        body.to_vec()
    } else {
        if body.len() > maximum_body_bytes {
            return Err(OllamaObservationError::new(
                ChatReason::OutputOverflow,
                "Ollama HTTP body exceeded its exact bound",
            ));
        }
        body.to_vec()
    };
    Ok(decoded)
}

fn decode_chunked(
    mut body: &[u8],
    maximum_body_bytes: usize,
) -> Result<Vec<u8>, OllamaObservationError> {
    let mut decoded = Vec::new();
    loop {
        let line_end = body
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| {
                OllamaObservationError::new(
                    ChatReason::MalformedProviderOutput,
                    "Ollama chunked response has incomplete framing",
                )
            })?;
        let length_text = std::str::from_utf8(&body[..line_end])
            .ok()
            .and_then(|value| value.split(';').next())
            .ok_or_else(|| {
                OllamaObservationError::new(
                    ChatReason::MalformedProviderOutput,
                    "Ollama chunk length is malformed",
                )
            })?;
        let length = usize::from_str_radix(length_text.trim(), 16).map_err(|_| {
            OllamaObservationError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama chunk length is not hexadecimal",
            )
        })?;
        body = &body[line_end + 2..];
        if length == 0 {
            return Ok(decoded);
        }
        if length > body.len() || body.get(length..length + 2) != Some(b"\r\n") {
            return Err(OllamaObservationError::new(
                ChatReason::MalformedProviderOutput,
                "Ollama chunk body is incomplete",
            ));
        }
        if decoded.len().saturating_add(length) > maximum_body_bytes {
            return Err(OllamaObservationError::new(
                ChatReason::OutputOverflow,
                "Ollama decoded body exceeded its exact bound",
            ));
        }
        decoded.extend_from_slice(&body[..length]);
        body = &body[length + 2..];
    }
}

fn map_io_error(error: std::io::Error) -> OllamaObservationError {
    let reason = if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        ChatReason::TimedOut
    } else {
        ChatReason::ProviderLost
    };
    OllamaObservationError::new(reason, format!("bounded Ollama transport failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunked_body_is_bounded_and_decoded() {
        let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nchat\r\n3\r\n-ok\r\n0\r\n\r\n";
        assert_eq!(decode_http_response(response, 16).unwrap(), b"chat-ok");
        assert_eq!(
            decode_http_response(response, 6).unwrap_err().reason,
            ChatReason::OutputOverflow
        );
    }

    #[test]
    fn model_digest_parser_is_exact() {
        let value = format!("sha256:{}", "ab".repeat(32));
        assert_eq!(parse_sha256(&value).unwrap().to_string(), value);
        assert!(parse_sha256("sha256:no").is_err());
    }

    #[test]
    fn observer_refuses_non_loopback_before_touching_the_host() {
        let observer = OllamaObserver {
            endpoint: "192.0.2.1:11434".parse().unwrap(),
            binary_path: PathBuf::from("/not/consulted"),
            model_name: "not-consulted".to_owned(),
            timeout: Duration::from_millis(1),
        };
        assert_eq!(
            observer.observe(10, 20).unwrap_err().reason,
            ChatReason::UnexpectedNetwork
        );
    }

    #[test]
    fn malformed_and_failed_http_responses_remain_distinct() {
        assert_eq!(
            decode_http_response(b"not-http", 16).unwrap_err().reason,
            ChatReason::MalformedProviderOutput
        );
        assert_eq!(
            decode_http_response(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n", 16,)
                .unwrap_err()
                .reason,
            ChatReason::ProviderUnavailable
        );
    }
}
