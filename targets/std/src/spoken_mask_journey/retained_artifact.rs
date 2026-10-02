//! Correlate the retained WAV container with the acknowledged PCM receipt.
use super::SpokenMaskExecution;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct RetainedSpokenMaskExecution {
    pub execution: SpokenMaskExecution,
    pub artifact: RetainedWavArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedWavArtifact {
    pub path: PathBuf,
    /// Hash of the entire WAV, including its container header.
    pub wav_sha256: String,
    pub wav_bytes: u64,
    /// Exact PCM digest and artifact identity from the acknowledged Show.
    pub pcm_sha256: String,
    pub artifact_identity: String,
}

pub(super) fn inspect(
    path: &Path,
    receipt: &conduit_presentation::SpokenMaskArtifactReceipt,
) -> Result<RetainedWavArtifact, String> {
    // The selected writer produces one canonical 44-byte stereo S16 WAV header.
    // Bound the evidence read by the already acknowledged PCM extent.
    let expected = u64::from(receipt.pcm_bytes) + 44;
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() != expected {
        return Err("retained WAV extent differs from acknowledged PCM".into());
    }
    let mut wav = Vec::with_capacity(expected as usize);
    file.take(expected + 1)
        .read_to_end(&mut wav)
        .map_err(|error| error.to_string())?;
    if wav.len() as u64 != expected
        || wav.len() < 44
        || &wav[0..4] != b"RIFF"
        || &wav[8..12] != b"WAVE"
        || &wav[12..16] != b"fmt "
        || &wav[36..40] != b"data"
        || u32::from_le_bytes(wav[40..44].try_into().unwrap()) != receipt.pcm_bytes
        || format!("{:x}", Sha256::digest(&wav[44..])) != receipt.content_sha256
    {
        return Err("retained WAV does not match acknowledged artifact".into());
    }
    Ok(RetainedWavArtifact {
        path: path.to_path_buf(),
        wav_sha256: format!("{:x}", Sha256::digest(&wav)),
        wav_bytes: expected,
        pcm_sha256: receipt.content_sha256.clone(),
        artifact_identity: receipt.artifact_identity.clone(),
    })
}
