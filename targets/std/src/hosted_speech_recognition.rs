//! Exact local whisper.cpp discovery and bounded speech recognition.

mod discovery;
pub use crate::hosted_language::{
    language_request_literal, read_language_coverage, read_language_request, HostedLanguageRefusal,
};
pub use discovery::WhisperDiscovery;

use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAXIMUM_DIAGNOSTIC_BYTES: usize = 8 * 1024;
static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhisperLimits {
    pub maximum_audio_bytes: u32,
    pub maximum_text_bytes: u16,
    pub threads: u8,
    pub timeout: Duration,
}

impl WhisperLimits {
    pub fn live_conversation(threads: u8, timeout: Duration) -> Self {
        let required = conduit_tongues::live_conversation_speech_requirements();
        Self {
            maximum_audio_bytes: required.recognition_audio_bytes(),
            maximum_text_bytes: required.recognized_text_bytes(),
            threads,
            timeout,
        }
    }

    fn validate(self) -> Result<Self, WhisperFailure> {
        if self.maximum_audio_bytes == 0
            || self.maximum_audio_bytes as usize > conduit_audio::MAXIMUM_PCM_CLIP_BYTES
            || self.maximum_text_bytes == 0
            || self.maximum_text_bytes as usize > conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES
            || !(1..=32).contains(&self.threads)
            || self.timeout.is_zero()
        {
            return Err(WhisperFailure::InvalidLimits);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperFailure {
    MissingProvider,
    InvalidProvider,
    InvalidLimits,
    Language(HostedLanguageRefusal),
    InvalidPcm,
    InvalidClip,
    UnsupportedPcmProfile,
    AudioOverflow,
    SpawnFailed,
    ProviderLost,
    ProviderChanged,
    ReadFailed,
    OutputOverflow,
    Timeout,
    Cancelled,
}

impl core::fmt::Display for WhisperFailure {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "Whisper provider refusal: {self:?}")
    }
}

impl std::error::Error for WhisperFailure {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperRecognitionReceipt {
    pub audio_sha256: [u8; 32],
    pub text_sha256: Option<String>,
    pub text_bytes: u16,
    pub diagnostic_bytes: u16,
}

pub struct WhisperSpeechAdapter {
    discovery: WhisperDiscovery,
    limits: WhisperLimits,
    workspace: PathBuf,
    last_receipt: Option<WhisperRecognitionReceipt>,
    proof_pcm_clip: Option<Vec<u8>>,
    evidence_text: Option<WhisperEvidenceText>,
}

#[derive(Clone)]
pub struct WhisperEvidenceText(Arc<Mutex<Vec<u8>>>);

impl WhisperEvidenceText {
    pub fn bytes(&self) -> Result<Vec<u8>, WhisperFailure> {
        self.0
            .lock()
            .map(|bytes| bytes.clone())
            .map_err(|_| WhisperFailure::ReadFailed)
    }
}

impl WhisperDiscovery {
    pub fn initialize(self, limits: WhisperLimits) -> Result<WhisperSpeechAdapter, WhisperFailure> {
        let limits = limits.validate()?;
        self.verify()?;
        let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
        let workspace =
            std::env::temp_dir().join(format!("conduit-whisper-{}-{sequence}", std::process::id()));
        std::fs::create_dir(&workspace).map_err(|_| WhisperFailure::InvalidProvider)?;
        Ok(WhisperSpeechAdapter {
            discovery: self,
            limits,
            workspace,
            last_receipt: None,
            proof_pcm_clip: None,
            evidence_text: None,
        })
    }
}

impl WhisperSpeechAdapter {
    /// Retain the bounded transcript from this adapter for proof publication.
    /// The complete maximum is allocated before Play so capture cannot hide
    /// runtime growth.
    pub fn enable_evidence_text(&mut self) -> WhisperEvidenceText {
        let capture = WhisperEvidenceText(Arc::new(Mutex::new(Vec::with_capacity(
            self.limits.maximum_text_bytes as usize,
        ))));
        self.evidence_text = Some(capture.clone());
        capture
    }

    #[cfg(feature = "local-model-proof")]
    pub(crate) fn set_proof_pcm_clip(&mut self, clip: Vec<u8>) -> Result<(), WhisperFailure> {
        let decoded =
            conduit_audio::decode_pcm_clip(&clip).map_err(|_| WhisperFailure::InvalidClip)?;
        if *decoded.profile.sample_rate_hz() != 16_000
            || *decoded.profile.layout() != conduit_audio::PcmChannelLayout::Mono
            || *decoded.profile.representation()
                != conduit_audio::PcmSampleRepresentation::Signed16LittleEndian
        {
            return Err(WhisperFailure::UnsupportedPcmProfile);
        }
        self.proof_pcm_clip = Some(clip);
        Ok(())
    }

    pub(crate) fn proof_pcm_clip(&self) -> Option<&[u8]> {
        self.proof_pcm_clip.as_deref()
    }

    pub fn discovery(&self) -> &WhisperDiscovery {
        &self.discovery
    }

    pub(crate) fn validate_language_declaration(
        &self,
        expected: &conduit_language::LanguageCoverage,
    ) -> Result<(), WhisperFailure> {
        if self.discovery.coverage.as_ref() != Some(expected) {
            return Err(WhisperFailure::Language(HostedLanguageRefusal::Artifact));
        }
        Ok(())
    }

    pub fn limits(&self) -> WhisperLimits {
        self.limits
    }

    pub fn provider_identity(&self) -> String {
        self.discovery.provider_identity()
    }

    pub fn offer(&self) -> conduit_core::CapabilityOffer {
        let mut offer = conduit_std_offers::whisper_speech_offer();
        offer.realization_properties =
            crate::hosted_language::properties(self.discovery.coverage.as_ref());
        offer
    }

    pub fn clip_offer(&self) -> conduit_core::CapabilityOffer {
        let mut offer = conduit_std_offers::whisper_clip_speech_offer();
        offer.realization_properties =
            crate::hosted_language::properties(self.discovery.coverage.as_ref());
        offer
    }

    pub fn recognize(
        &mut self,
        encoded: &[u8],
        language: &conduit_language::LanguageRequest,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Vec<u8>, WhisperFailure> {
        if encoded.len() > self.limits.maximum_audio_bytes as usize {
            return Err(WhisperFailure::AudioOverflow);
        }
        let (header, payload) =
            PcmFrameHeader::decode_frame(encoded).map_err(|_| WhisperFailure::InvalidPcm)?;
        if header.representation != PcmSampleRepresentation::Signed16LittleEndian
            || header.layout != PcmChannelLayout::Mono
            || header.sample_rate_hz != 16_000
            || header.discontinuity
        {
            return Err(WhisperFailure::UnsupportedPcmProfile);
        }
        let audio_sha256: [u8; 32] = Sha256::digest(encoded).into();
        self.recognize_payload(
            payload,
            audio_sha256,
            encoded.len() as u32,
            language,
            cancelled,
        )
    }

    pub fn recognize_clip(
        &mut self,
        encoded: &[u8],
        language: &conduit_language::LanguageRequest,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Vec<u8>, WhisperFailure> {
        if encoded.len() > self.limits.maximum_audio_bytes as usize {
            return Err(WhisperFailure::AudioOverflow);
        }
        let clip =
            conduit_audio::decode_pcm_clip(encoded).map_err(|_| WhisperFailure::InvalidClip)?;
        if *clip.profile.representation() != PcmSampleRepresentation::Signed16LittleEndian
            || *clip.profile.layout() != PcmChannelLayout::Mono
            || *clip.profile.sample_rate_hz() != 16_000
        {
            return Err(WhisperFailure::UnsupportedPcmProfile);
        }
        let payload_bytes = clip
            .blocks
            .iter()
            .try_fold(0_usize, |total, block| {
                total.checked_add(block.payload.len())
            })
            .ok_or(WhisperFailure::AudioOverflow)?;
        let mut payload = Vec::with_capacity(payload_bytes);
        for block in clip.blocks {
            payload.extend_from_slice(block.payload);
        }
        let audio_sha256: [u8; 32] = Sha256::digest(encoded).into();
        self.recognize_payload(
            &payload,
            audio_sha256,
            encoded.len() as u32,
            language,
            cancelled,
        )
    }

    fn recognize_payload(
        &mut self,
        payload: &[u8],
        audio_sha256: [u8; 32],
        audio_extent_bytes: u32,
        language: &conduit_language::LanguageRequest,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Vec<u8>, WhisperFailure> {
        if cancelled() {
            return Err(WhisperFailure::Cancelled);
        }
        // This trusted installed-provider profile verifies finite source bytes;
        // it does not claim hostile-code confinement of the operating system.
        self.discovery.verify()?;
        let wav = self.workspace.join("input.wav");
        let output_base = self.workspace.join("result");
        write_wav(&wav, payload)?;
        let mut child = self.spawn(&wav, &output_base, language)?;
        let stderr = child.stderr.take();
        let diagnostics =
            std::thread::spawn(move || bounded_read(stderr, MAXIMUM_DIAGNOSTIC_BYTES));
        let started = Instant::now();
        let status = loop {
            if cancelled() {
                stop(&mut child);
                let _ = diagnostics.join();
                return Err(WhisperFailure::Cancelled);
            }
            if started.elapsed() >= self.limits.timeout {
                stop(&mut child);
                let _ = diagnostics.join();
                return Err(WhisperFailure::Timeout);
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => std::thread::sleep(Duration::from_millis(2)),
                Err(_) => {
                    stop(&mut child);
                    let _ = diagnostics.join();
                    return Err(WhisperFailure::ProviderLost);
                }
            }
        };
        let diagnostics = diagnostics
            .join()
            .map_err(|_| WhisperFailure::ReadFailed)??;
        if !status.success() {
            return Err(WhisperFailure::ProviderLost);
        }
        let transcript_path = output_base.with_extension("txt");
        let transcript =
            std::fs::read_to_string(transcript_path).map_err(|_| WhisperFailure::ReadFailed)?;
        let transcript = transcript.trim();
        if transcript.len() > self.limits.maximum_text_bytes as usize {
            return Err(WhisperFailure::OutputOverflow);
        }
        if let Some(capture) = &self.evidence_text {
            let mut bytes = capture.0.lock().map_err(|_| WhisperFailure::ReadFailed)?;
            bytes.clear();
            bytes.extend_from_slice(transcript.as_bytes());
        }
        let result = if transcript.is_empty() {
            conduit_tongues::no_speech_result(
                audio_sha256,
                audio_extent_bytes,
                self.provider_identity(),
            )
        } else {
            conduit_tongues::recognized_result(
                audio_sha256,
                audio_extent_bytes,
                self.provider_identity(),
                transcript.to_owned(),
            )
        }
        .map_err(|_| WhisperFailure::OutputOverflow)?;
        let encoded_result = conduit_tongues::encode_speech_recognition_result(&result)
            .map_err(|_| WhisperFailure::OutputOverflow)?;
        let text_sha256 = (!transcript.is_empty())
            .then(|| format!("{:x}", Sha256::digest(transcript.as_bytes())));
        self.last_receipt = Some(WhisperRecognitionReceipt {
            audio_sha256,
            text_sha256,
            text_bytes: transcript.len() as u16,
            diagnostic_bytes: diagnostics.len() as u16,
        });
        Ok(encoded_result)
    }

    pub fn take_receipt(&mut self) -> Option<WhisperRecognitionReceipt> {
        self.last_receipt.take()
    }

    fn spawn(
        &self,
        wav: &Path,
        output_base: &Path,
        language: &conduit_language::LanguageRequest,
    ) -> Result<Child, WhisperFailure> {
        let private_language =
            crate::hosted_language::provider_language(self.discovery.coverage.as_ref(), language)
                .map_err(WhisperFailure::Language)?;
        Command::new(&self.discovery.executable)
            .args(["--language", private_language])
            .args(["--model"])
            .arg(&self.discovery.model)
            .args(["--file"])
            .arg(wav)
            .args(["--output-txt", "--output-file"])
            .arg(output_base)
            .args(["--no-timestamps", "--no-prints"])
            .args(["--threads"])
            .arg(self.limits.threads.to_string())
            .args(["--processors", "1"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| WhisperFailure::SpawnFailed)
    }
}

impl Drop for WhisperSpeechAdapter {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

fn write_wav(path: &Path, payload: &[u8]) -> Result<(), WhisperFailure> {
    let payload_len = u32::try_from(payload.len()).map_err(|_| WhisperFailure::AudioOverflow)?;
    let riff_len = payload_len
        .checked_add(36)
        .ok_or(WhisperFailure::AudioOverflow)?;
    let mut file = File::create(path).map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(b"RIFF")
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(&riff_len.to_le_bytes())
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(b"WAVEfmt \x10\0\0\0\x01\0\x01\0")
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(&16_000_u32.to_le_bytes())
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(&32_000_u32.to_le_bytes())
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(&2_u16.to_le_bytes())
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(&16_u16.to_le_bytes())
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(b"data")
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(&payload_len.to_le_bytes())
        .map_err(|_| WhisperFailure::InvalidProvider)?;
    file.write_all(payload)
        .map_err(|_| WhisperFailure::InvalidProvider)
}

fn bounded_read(reader: Option<impl Read>, maximum: usize) -> Result<Vec<u8>, WhisperFailure> {
    let Some(reader) = reader else {
        return Ok(Vec::new());
    };
    let mut bytes = Vec::with_capacity(maximum + 1);
    reader
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| WhisperFailure::ReadFailed)?;
    if bytes.len() > maximum {
        return Err(WhisperFailure::OutputOverflow);
    }
    Ok(bytes)
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests;
