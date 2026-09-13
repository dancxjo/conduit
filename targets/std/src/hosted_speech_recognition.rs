//! Exact local whisper.cpp discovery and bounded speech recognition.

use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_tongues::{SpeechRecognitionDisposition, SpeechRecognitionResult};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const MAXIMUM_EXECUTABLE_BYTES: u64 = 64 * 1024 * 1024;
const MAXIMUM_MODEL_BYTES: u64 = 256 * 1024 * 1024;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperDiscovery {
    executable: PathBuf,
    model: PathBuf,
    pub executable_sha256: String,
    pub model_sha256: String,
    pub model_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperFailure {
    MissingProvider,
    InvalidProvider,
    InvalidLimits,
    InvalidPcm,
    InvalidClip,
    UnsupportedPcmProfile,
    AudioOverflow,
    SpawnFailed,
    ProviderLost,
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
}

impl WhisperDiscovery {
    pub fn inspect(
        executable: impl AsRef<Path>,
        model: impl AsRef<Path>,
    ) -> Result<Self, WhisperFailure> {
        let executable = exact_file(executable.as_ref(), MAXIMUM_EXECUTABLE_BYTES)?;
        let model = exact_file(model.as_ref(), MAXIMUM_MODEL_BYTES)?;
        if !is_executable(&executable)? {
            return Err(WhisperFailure::InvalidProvider);
        }
        let model_bytes = model
            .metadata()
            .map_err(|_| WhisperFailure::InvalidProvider)?
            .len();
        Ok(Self {
            executable_sha256: digest_file(&executable)?,
            model_sha256: digest_file(&model)?,
            model_bytes,
            executable,
            model,
        })
    }

    pub fn initialize(self, limits: WhisperLimits) -> Result<WhisperSpeechAdapter, WhisperFailure> {
        let limits = limits.validate()?;
        let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
        let workspace =
            std::env::temp_dir().join(format!("conduit-whisper-{}-{sequence}", std::process::id()));
        std::fs::create_dir(&workspace).map_err(|_| WhisperFailure::InvalidProvider)?;
        Ok(WhisperSpeechAdapter {
            discovery: self,
            limits,
            workspace,
            last_receipt: None,
        })
    }
}

impl WhisperSpeechAdapter {
    pub fn discovery(&self) -> &WhisperDiscovery {
        &self.discovery
    }

    pub fn limits(&self) -> WhisperLimits {
        self.limits
    }

    pub fn recognize(
        &mut self,
        encoded: &[u8],
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
        self.recognize_payload(payload, audio_sha256, cancelled)
    }

    pub fn recognize_clip(
        &mut self,
        encoded: &[u8],
        cancelled: impl FnMut() -> bool,
    ) -> Result<Vec<u8>, WhisperFailure> {
        if encoded.len() > self.limits.maximum_audio_bytes as usize {
            return Err(WhisperFailure::AudioOverflow);
        }
        let clip =
            conduit_audio::decode_pcm_clip(encoded).map_err(|_| WhisperFailure::InvalidClip)?;
        if clip.profile.representation != PcmSampleRepresentation::Signed16LittleEndian
            || clip.profile.layout != PcmChannelLayout::Mono
            || clip.profile.sample_rate_hz != 16_000
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
        self.recognize_payload(&payload, audio_sha256, cancelled)
    }

    fn recognize_payload(
        &mut self,
        payload: &[u8],
        audio_sha256: [u8; 32],
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Vec<u8>, WhisperFailure> {
        let wav = self.workspace.join("input.wav");
        let output_base = self.workspace.join("result");
        write_wav(&wav, payload)?;
        let mut child = self.spawn(&wav, &output_base)?;
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
        let result = SpeechRecognitionResult {
            disposition: if transcript.is_empty() {
                SpeechRecognitionDisposition::NoSpeech
            } else {
                SpeechRecognitionDisposition::Recognized
            },
            text: (!transcript.is_empty()).then(|| transcript.to_owned()),
            audio_sha256,
        };
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

    fn spawn(&self, wav: &Path, output_base: &Path) -> Result<Child, WhisperFailure> {
        Command::new(&self.discovery.executable)
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

fn exact_file(path: &Path, maximum: u64) -> Result<PathBuf, WhisperFailure> {
    let path = path
        .canonicalize()
        .map_err(|_| WhisperFailure::MissingProvider)?;
    let metadata = path
        .metadata()
        .map_err(|_| WhisperFailure::MissingProvider)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(WhisperFailure::InvalidProvider);
    }
    Ok(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> Result<bool, WhisperFailure> {
    use std::os::unix::fs::PermissionsExt;
    Ok(path
        .metadata()
        .map_err(|_| WhisperFailure::InvalidProvider)?
        .permissions()
        .mode()
        & 0o111
        != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> Result<bool, WhisperFailure> {
    Ok(false)
}

fn digest_file(path: &Path) -> Result<String, WhisperFailure> {
    let mut file = File::open(path).map_err(|_| WhisperFailure::InvalidProvider)?;
    let mut digest = Sha256::new();
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut bytes)
            .map_err(|_| WhisperFailure::InvalidProvider)?;
        if read == 0 {
            break;
        }
        digest.update(&bytes[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(script: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "conduit-whisper-adapter-test-{}-{}",
            std::process::id(),
            NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let executable = root.join("whisper-cli");
        let model = root.join("model.bin");
        fs::write(&executable, script).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&model, b"bounded model").unwrap();
        (root, executable, model)
    }

    fn pcm(samples: &[i16]) -> Vec<u8> {
        let payload = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            16_000,
            PcmChannelLayout::Mono,
            samples.len() as u16,
            1,
            0,
            false,
        )
        .unwrap()
        .encode_frame(&payload)
        .unwrap()
    }

    fn limits(timeout: Duration) -> WhisperLimits {
        WhisperLimits {
            maximum_audio_bytes: conduit_tongues::MAXIMUM_RECOGNITION_AUDIO_BYTES as u32,
            maximum_text_bytes: conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES as u16,
            threads: 2,
            timeout,
        }
    }

    #[test]
    fn exact_provider_emits_canonical_recognition_and_receipt() {
        let (root, executable, model) = fixture(
            "#!/bin/sh\nout=\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = --output-file ]; then out=$2; shift 2; else shift; fi\ndone\nprintf 'Rosehip House, status\\n' > \"${out}.txt\"\n",
        );
        let discovery = WhisperDiscovery::inspect(&executable, &model).unwrap();
        let expected_model = discovery.model_sha256.clone();
        let mut adapter = discovery
            .initialize(limits(Duration::from_secs(2)))
            .unwrap();
        let audio = pcm(&[1, -2, 3, -4]);
        let encoded = adapter.recognize(&audio, || false).unwrap();
        let result = conduit_tongues::decode_speech_recognition_result(&encoded).unwrap();
        assert_eq!(result.disposition, SpeechRecognitionDisposition::Recognized);
        assert_eq!(result.text.as_deref(), Some("Rosehip House, status"));
        let receipt = adapter.take_receipt().unwrap();
        assert_eq!(receipt.audio_sha256, Sha256::digest(&audio).as_slice());
        assert_eq!(adapter.discovery().model_sha256, expected_model);
        drop(adapter);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn timeout_kills_provider_and_missing_output_is_failure() {
        let (root, executable, model) = fixture("#!/bin/sh\nsleep 2\n");
        let mut adapter = WhisperDiscovery::inspect(&executable, &model)
            .unwrap()
            .initialize(limits(Duration::from_millis(20)))
            .unwrap();
        assert_eq!(
            adapter.recognize(&pcm(&[1]), || false),
            Err(WhisperFailure::Timeout)
        );
        drop(adapter);
        fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
        let mut adapter = WhisperDiscovery::inspect(&executable, &model)
            .unwrap()
            .initialize(limits(Duration::from_secs(1)))
            .unwrap();
        assert_eq!(
            adapter.recognize(&pcm(&[1]), || false),
            Err(WhisperFailure::ReadFailed)
        );
        drop(adapter);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn profile_bounds_and_cancellation_fail_distinctly() {
        let (root, executable, model) = fixture("#!/bin/sh\nsleep 2\n");
        let mut adapter = WhisperDiscovery::inspect(&executable, &model)
            .unwrap()
            .initialize(limits(Duration::from_secs(1)))
            .unwrap();
        assert_eq!(
            adapter.recognize(&pcm(&[1]), || true),
            Err(WhisperFailure::Cancelled)
        );
        let stereo = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            16_000,
            PcmChannelLayout::StereoLeftRight,
            1,
            1,
            0,
            false,
        )
        .unwrap()
        .encode_frame(&[0, 0, 0, 0])
        .unwrap();
        assert_eq!(
            adapter.recognize(&stereo, || false),
            Err(WhisperFailure::UnsupportedPcmProfile)
        );
        drop(adapter);
        fs::remove_dir_all(root).unwrap();
    }
}
