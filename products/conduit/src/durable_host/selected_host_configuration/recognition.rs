//! Initialize exact declared recognition equipment without starting recognition.
use conduit_host_make::BaseInitialization;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::hosted_speech_recognition::{
    WhisperDiscovery, WhisperLimits, WhisperSpeechAdapter,
};
use serde::Deserialize;
use std::{io::Read, path::PathBuf, time::Duration};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameters {
    executable: PathBuf,
    executable_sha256: String,
    model: PathBuf,
    model_sha256: String,
    model_bytes: u64,
    language_coverage: PathBuf,
    language_coverage_sha256: String,
    threads: u8,
    timeout_milliseconds: u64,
}
fn parameters(initialization: &BaseInitialization) -> Result<Parameters, String> {
    let catalog = conduit_workspace_make::catalog();
    let contract = catalog
        .initialization_contracts
        .get(conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION)
        .ok_or("installed recognition initialization contract unavailable")?;
    initialization.checked_values(contract)?;
    let p: Parameters =
        serde_json::from_value(serde_json::to_value(initialization).map_err(|e| e.to_string())?)
            .map_err(|e| format!("recognition initialization parameters: {e}"))?;
    if ![&p.executable, &p.model, &p.language_coverage]
        .iter()
        .all(|p| p.is_absolute())
        || ![
            &p.executable_sha256,
            &p.model_sha256,
            &p.language_coverage_sha256,
        ]
        .iter()
        .all(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    {
        return Err(
            "recognition selection requires absolute paths and exact SHA-256 identities".into(),
        );
    }
    Ok(p)
}
pub(super) fn validate(initialization: &BaseInitialization) -> Result<(), String> {
    parameters(initialization).map(|_| ())
}
pub(super) fn initialize(
    initialization: &BaseInitialization,
) -> Result<WhisperSpeechAdapter, String> {
    let p = parameters(initialization)?;
    let discovery =
        WhisperDiscovery::inspect(&p.executable, &p.model).map_err(|e| e.to_string())?;
    if discovery.executable_sha256 != p.executable_sha256
        || discovery.model_sha256 != p.model_sha256
        || discovery.model_bytes != p.model_bytes
    {
        return Err("selected recognition executable/model identity changed".into());
    }
    let maximum = conduit_core::MAXIMUM_REALIZATION_PROPERTY_BYTES as u64;
    let file = std::fs::File::open(&p.language_coverage).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err("recognition Language coverage violates its finite file bound".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > maximum
        || super::super::digest(&bytes).strip_prefix("sha256:")
            != Some(p.language_coverage_sha256.as_str())
    {
        return Err("selected recognition Language coverage identity changed".into());
    }
    let coverage = conduit_language::LanguageCoverage::decode(&bytes)
        .map_err(|e| format!("recognition Language coverage: {e:?}"))?;
    discovery
        .declare_language_coverage(coverage)
        .map_err(|e| e.to_string())?
        .initialize(WhisperLimits::clip_recognition(
            p.threads,
            Duration::from_millis(p.timeout_milliseconds),
        ))
        .map_err(|e| e.to_string())
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[path = "recognition/tests.rs"]
mod tests;
