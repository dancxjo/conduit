//! Reobserve the exact declared ALSA endpoint before publishing its Base.
use super::super::{bounded_read, digest};
use conduit_host_make::BaseInitialization;
use conduit_std_host::hosted_microphone::{
    AlsaMicrophoneAdapter, AlsaMicrophoneDiscovery, MicrophoneLimits,
};
use serde::Deserialize;
use std::{path::PathBuf, time::Duration};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameters {
    executable: PathBuf,
    executable_sha256: String,
    card_id: String,
    device: u16,
    base_identity: String,
    capture_milliseconds: u32,
    timeout_milliseconds: u64,
}
fn parameters(
    initialization: &BaseInitialization,
    implementation: &str,
) -> Result<Parameters, String> {
    let catalog = conduit_workspace_make::catalog();
    let contract = catalog
        .initialization_contracts
        .get(implementation)
        .ok_or("installed microphone initialization contract unavailable")?;
    initialization.checked_values(contract)?;
    let parameters: Parameters =
        serde_json::from_value(serde_json::to_value(initialization).map_err(|e| e.to_string())?)
            .map_err(|e| format!("microphone initialization parameters: {e}"))?;
    if !parameters.executable.is_absolute()
        || parameters.executable_sha256.len() != 64
        || !parameters
            .executable_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(
            "microphone selection requires an absolute executable and exact SHA-256".into(),
        );
    }
    Ok(parameters)
}
pub(super) fn validate(
    initialization: &BaseInitialization,
    implementation: &str,
) -> Result<(), String> {
    parameters(initialization, implementation).map(|_| ())
}
pub(super) fn initialize(
    initialization: &BaseInitialization,
    implementation: &str,
) -> Result<AlsaMicrophoneAdapter, String> {
    let parameters = parameters(initialization, implementation)?;
    // Reject a changed executable before metadata discovery can start a process.
    let observed = digest(&bounded_read(&parameters.executable, 8 * 1024 * 1024)?);
    if observed.strip_prefix("sha256:") != Some(parameters.executable_sha256.as_str()) {
        return Err("selected microphone executable identity changed".into());
    }
    let discovery =
        AlsaMicrophoneDiscovery::inspect(&parameters.executable).map_err(|e| e.to_string())?;
    if discovery.executable_sha256 != parameters.executable_sha256 {
        return Err("microphone executable changed during discovery".into());
    }
    let matches: Vec<_> = discovery
        .observations
        .iter()
        .filter(|v| {
            v.card_id == parameters.card_id
                && v.device == parameters.device
                && v.base_identity == parameters.base_identity
        })
        .cloned()
        .collect();
    let [selected] = matches.as_slice() else {
        return Err("selected microphone endpoint identity is missing or ambiguous".into());
    };
    let format = match implementation {
        conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION => {
            conduit_std_host::hosted_microphone::AlsaMicrophoneFormat::ExactHardware
        }
        conduit_std_offers::CONVERTED_MICROPHONE_CLIP_IMPLEMENTATION => {
            conduit_std_host::hosted_microphone::AlsaMicrophoneFormat::AlsaPlug
        }
        _ => return Err("unsupported microphone implementation".into()),
    };
    discovery
        .initialize_with_format(
            selected,
            MicrophoneLimits {
                capture_milliseconds: parameters.capture_milliseconds,
                timeout: Duration::from_millis(parameters.timeout_milliseconds),
            },
            format,
        )
        .map_err(|e| e.to_string())
}
