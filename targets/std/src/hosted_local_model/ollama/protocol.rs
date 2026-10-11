//! Existing private wire records and bounded loopback process transport.
use super::*;
use serde::Deserialize;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

#[derive(Deserialize)]
pub(super) struct TagsResponse {
    pub(super) models: Vec<TagModel>,
}

#[derive(Deserialize)]
pub(super) struct TagModel {
    pub(super) name: String,
    pub(super) digest: String,
    pub(super) size: u64,
    pub(super) details: ModelDetails,
}

#[derive(Deserialize)]
pub(super) struct ModelDetails {
    pub(super) family: String,
    pub(super) parameter_size: String,
    pub(super) quantization_level: String,
}

#[derive(Deserialize)]
pub(super) struct ShowResponse {
    pub(super) model_info: serde_json::Map<String, serde_json::Value>,
    pub(super) capabilities: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct GenerateResponse {
    pub(super) response: String,
    #[serde(default)]
    pub(super) done: Option<bool>,
    #[serde(default)]
    pub(super) done_reason: String,
    #[serde(default)]
    pub(super) prompt_eval_count: Option<u64>,
    #[serde(default)]
    pub(super) eval_count: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct ClassificationWire {
    pub(super) label: String,
}

#[derive(Deserialize)]
pub(super) struct ExtractionWire {
    pub(super) subject: String,
}

#[derive(Deserialize)]
pub(super) struct EmbedResponse {
    pub(super) embeddings: Vec<Vec<f32>>,
    #[serde(default)]
    pub(super) prompt_eval_count: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct InterpretationWire {
    pub(super) hypothesis: String,
    #[serde(default)]
    pub(super) referenced_evidence: Vec<String>,
    pub(super) confidence_permille: Option<u16>,
    #[serde(default)]
    pub(super) implications: Vec<String>,
    pub(super) disposition: String,
}

#[derive(Deserialize)]
pub(super) struct ChatResponse {
    pub(super) message: ChatMessage,
    #[serde(default)]
    pub(super) done: Option<bool>,
    #[serde(default)]
    pub(super) done_reason: String,
    #[serde(default)]
    pub(super) prompt_eval_count: Option<u64>,
    #[serde(default)]
    pub(super) eval_count: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct ChatMessage {
    pub(super) content: String,
}

pub(super) fn model_names_match(candidate: &str, requested: &str) -> bool {
    candidate == requested || candidate.strip_suffix(":latest") == Some(requested)
}

pub(in super::super) fn curl_json(
    endpoint: &str,
    path: &str,
    body: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let url = format!("{endpoint}{path}");
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

pub(super) fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    let Some(authority) = endpoint.strip_prefix("http://") else {
        return Err("Ollama endpoint must use explicit http:// loopback transport".into());
    };
    let host = authority
        .rsplit_once(':')
        .map_or(authority, |(host, _)| host);
    if !matches!(host, "127.0.0.1" | "localhost" | "[::1]")
        || authority.contains('/')
        || endpoint.ends_with(':')
    {
        return Err("Ollama endpoint must name one exact loopback origin without a path".into());
    }
    Ok(())
}

pub(super) fn bounded_command(
    program: &str,
    args: &[&str],
    maximum: usize,
) -> Result<String, String> {
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
