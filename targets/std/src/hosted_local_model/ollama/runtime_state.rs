//! Bounded read-only provider sampling, independently of initialization/warm-up.
use super::protocol::{bounded_child, model_names_match, validate_endpoint};
use crate::hosted_local_model::{LocalModelRuntimeState, ModelResidencyState};
use serde::Deserialize;
use std::process::Command;
const MAXIMUM_SAMPLE_BYTES: usize = 65_536;
const MAXIMUM_LOADED_MODELS: usize = 32;
#[derive(Deserialize)]
struct LoadedModels {
    models: Vec<LoadedModel>,
}
#[derive(Deserialize)]
struct LoadedModel {
    name: String,
    digest: String,
    size: Option<u64>,
    size_vram: Option<u64>,
    context_length: Option<u64>,
    expires_at: Option<String>,
}
pub fn sample_ollama_runtime(
    endpoint: &str,
    name: &str,
    digest: &str,
) -> Result<LocalModelRuntimeState, String> {
    validate_endpoint(endpoint)?;
    if endpoint.contains(['@', '?', '#']) {
        return Err("runtime sample needs one direct loopback origin".into());
    }
    let maximum = conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES;
    if name.is_empty() || digest.is_empty() || name.len() > maximum || digest.len() > maximum {
        return Err("invalid pinned runtime model identity".into());
    }
    let mut state = decode(
        &read_only(endpoint, "/api/ps", MAXIMUM_SAMPLE_BYTES)?,
        name,
        digest,
    )?;
    state.runtime_version = Some(decode_version(&read_only(endpoint, "/api/version", 512)?)?);
    Ok(state)
}
fn read_only(endpoint: &str, path: &str, maximum: usize) -> Result<Vec<u8>, String> {
    let mut command = Command::new("curl");
    command.args([
        "--silent",
        "--show-error",
        "--fail",
        "--noproxy",
        "*",
        "--max-time",
        "0.25",
    ]);
    command.arg(format!("{endpoint}{path}"));
    bounded_child(command, None, maximum)
}
fn decode_version(bytes: &[u8]) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Version {
        version: String,
    }
    if bytes.len() > 512 {
        return Err("runtime version response exceeds byte bound".into());
    }
    let value: Version =
        serde_json::from_slice(bytes).map_err(|_| "malformed runtime version response")?;
    if value.version.is_empty()
        || value.version.len() > conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES
    {
        return Err("runtime version exceeds identity bound".into());
    }
    Ok(value.version)
}
pub(super) fn version_profile_matches(expected: &str, actual: Option<&str>) -> bool {
    let expected = expected
        .strip_prefix("ollama version is ")
        .unwrap_or(expected);
    actual.is_some_and(|actual| !actual.is_empty() && actual == expected)
}

fn decode(bytes: &[u8], name: &str, digest: &str) -> Result<LocalModelRuntimeState, String> {
    let maximum_identity = conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES;
    if bytes.len() > MAXIMUM_SAMPLE_BYTES
        || name.is_empty()
        || digest.is_empty()
        || name.len() > maximum_identity
        || digest.len() > maximum_identity
    {
        return Err("runtime model sample exceeds identity or byte bounds".into());
    }
    let response: LoadedModels =
        serde_json::from_slice(bytes).map_err(|_| "malformed loaded-model sample")?;
    if response.models.len() > MAXIMUM_LOADED_MODELS {
        return Err("loaded-model sample exceeds entry bound".into());
    }
    let mut selected = None;
    for model in response.models {
        if model.name.is_empty()
            || model.digest.is_empty()
            || model.name.len() > maximum_identity
            || model.digest.len() > maximum_identity
            || model
                .expires_at
                .as_ref()
                .is_some_and(|value| value.len() > 128)
        {
            return Err("loaded-model sample contains invalid identity metadata".into());
        }
        if !model_names_match(&model.name, name) {
            continue;
        }
        if model.digest != digest {
            return Err("loaded model content differs from pinned identity".into());
        }
        if selected.is_some() {
            return Err("ambiguous duplicate loaded model".into());
        }
        selected = Some(LocalModelRuntimeState {
            residency: ModelResidencyState::ResidentWithoutEpoch,
            runtime_version: None,
            resident_bytes: model.size,
            vram_bytes: model.size_vram,
            context_length: model.context_length,
            provider_expiry: model.expires_at,
        });
    }
    Ok(selected.unwrap_or(LocalModelRuntimeState {
        residency: ModelResidencyState::NotResident,
        runtime_version: None,
        resident_bytes: None,
        vram_bytes: None,
        context_length: None,
        provider_expiry: None,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loaded_is_not_initialized_or_a_fabricated_residency_epoch() {
        let state=decode(br#"{"models":[{"name":"pinned","digest":"exact","size":4096,"size_vram":0,"expires_at":"provider-wall-time"}]}"#,"pinned","exact").unwrap();
        assert_eq!(state.residency, ModelResidencyState::ResidentWithoutEpoch);
        assert_eq!(state.resident_bytes, Some(4096));
        assert_eq!(state.vram_bytes, Some(0));
        assert_eq!(state.context_length, None);
        assert_eq!(
            decode(
                br#"{"models":[{"name":"pinned","digest":"exact"}]}"#,
                "pinned",
                "exact"
            )
            .unwrap()
            .resident_bytes,
            None
        );
        assert_eq!(
            decode(br#"{"models":[]}"#, "pinned", "exact")
                .unwrap()
                .residency,
            ModelResidencyState::NotResident
        );
    }
    #[test]
    fn malformed_rebound_duplicate_and_oversized_inventory_refuse() {
        for raw in [br#"{"models":[{"name":"pinned","digest":"changed"}]}"#.as_slice(),
            br#"{"models":[{"name":"pinned","digest":"exact"},{"name":"pinned","digest":"exact"}]}"#,
            br#"{"models":[{"name":"pinned","digest":"exact","size":-1}]}"#,br#"{}"#] {
            assert!(decode(raw,"pinned","exact").is_err());
        }
        assert!(decode(&vec![b' '; MAXIMUM_SAMPLE_BYTES + 1], "pinned", "exact").is_err());
        let models = serde_json::json!({"models":(0..33).map(|i|serde_json::json!({"name":format!("model-{i}"),"digest":"exact"})).collect::<Vec<_>>()});
        assert!(decode(&serde_json::to_vec(&models).unwrap(), "pinned", "exact").is_err());
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;
    #[test]
    fn runtime_version_is_independent_bounded_and_exactly_profile_matched() {
        let actual = decode_version(br#"{"version":"0.23.2"}"#).unwrap();
        assert!(version_profile_matches(
            "ollama version is 0.23.2",
            Some(&actual)
        ));
        assert!(version_profile_matches("0.23.2", Some(&actual)));
        assert!(!version_profile_matches(
            "ollama version is 0.23.1",
            Some(&actual)
        ));
        assert!(!version_profile_matches("ollama version is 0.23.2", None));
        for raw in [
            br#"{}"#.as_slice(),
            br#"{"version":""}"#,
            br#"{"version":0}"#,
        ] {
            assert!(decode_version(raw).is_err());
        }
        assert!(decode_version(&vec![b' '; 513]).is_err());
    }
}
