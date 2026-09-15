//! Boot-scoped bounded WAV artifact output for explicit evidence destinations.

use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{BootId, OfferGeneration, ResourcePoolId};
use std::io::Write;
use std::path::{Path, PathBuf};

const SAMPLE_RATE_HZ: u32 = 48_000;
const CHANNELS: u16 = 2;
const SAMPLE_BYTES: u16 = 2;
const MAXIMUM_PCM_BYTES: usize = conduit_semantic_catalog::AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS as usize
    * conduit_semantic_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES as usize
    * CHANNELS as usize
    * SAMPLE_BYTES as usize;

#[derive(Clone, Debug)]
pub struct WavArtifactSelection {
    destination: PathBuf,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

impl WavArtifactSelection {
    pub fn new(
        destination: impl AsRef<Path>,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, String> {
        let destination = destination.as_ref();
        let parent = destination
            .parent()
            .ok_or_else(|| "WAV artifact destination has no parent".to_string())?;
        if !parent.is_dir() || destination.file_name().is_none() {
            return Err("WAV artifact destination parent is unavailable".into());
        }
        Ok(Self {
            destination: destination.to_path_buf(),
            boot_id,
            offer_generation,
        })
    }

    pub fn pool_id(&self) -> ResourcePoolId {
        ResourcePoolId::from("std/audio/wav-artifact")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WavArtifactReport {
    pub pcm_bytes: u32,
    pub frames: u32,
    pub blocks: u16,
    pub completed: bool,
}

pub(crate) struct WavArtifactSession {
    selection: WavArtifactSelection,
    pcm: Vec<u8>,
    next_frame: u64,
    blocks: u16,
    completed: bool,
}

impl WavArtifactSession {
    pub(crate) fn prepare(selection: WavArtifactSelection) -> Self {
        Self {
            selection,
            pcm: Vec::with_capacity(MAXIMUM_PCM_BYTES),
            next_frame: 0,
            blocks: 0,
            completed: false,
        }
    }

    pub(crate) fn write_frame(&mut self, encoded: &[u8]) -> Result<(), String> {
        if self.completed {
            return Err("WAV artifact is already complete".into());
        }
        let (header, payload) = PcmFrameHeader::decode_frame(encoded)
            .map_err(|_| "WAV artifact received malformed PCM".to_string())?;
        if header.representation != PcmSampleRepresentation::Signed16LittleEndian
            || header.layout != PcmChannelLayout::StereoLeftRight
            || header.sample_rate_hz != SAMPLE_RATE_HZ
            || header.start_frame != self.next_frame
            || header.discontinuity
            || payload.len() != usize::from(header.frame_count) * 4
            || self.pcm.len().saturating_add(payload.len()) > self.pcm.capacity()
        {
            return Err("WAV artifact received unsupported or discontinuous PCM".into());
        }
        self.pcm.extend_from_slice(payload);
        self.next_frame = self
            .next_frame
            .checked_add(u64::from(header.frame_count))
            .ok_or_else(|| "WAV artifact frame extent overflowed".to_string())?;
        self.blocks = self
            .blocks
            .checked_add(1)
            .ok_or_else(|| "WAV artifact block extent overflowed".to_string())?;
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<(), String> {
        if self.completed || self.pcm.is_empty() {
            return Err("WAV artifact cannot complete in its current state".into());
        }
        write_wav(&self.selection.destination, &self.pcm)?;
        self.completed = true;
        Ok(())
    }

    pub(crate) fn report(&self) -> WavArtifactReport {
        WavArtifactReport {
            pcm_bytes: self.pcm.len() as u32,
            frames: u32::try_from(self.next_frame).unwrap_or(u32::MAX),
            blocks: self.blocks,
            completed: self.completed,
        }
    }
}

fn write_wav(path: &Path, pcm: &[u8]) -> Result<(), String> {
    let data_bytes = u32::try_from(pcm.len()).map_err(|_| "WAV data extent overflowed")?;
    let riff_bytes = data_bytes
        .checked_add(36)
        .ok_or("WAV RIFF extent overflowed")?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create new WAV artifact: {error}"))?;
    file.write_all(b"RIFF")
        .and_then(|()| file.write_all(&riff_bytes.to_le_bytes()))
        .and_then(|()| file.write_all(b"WAVEfmt \x10\0\0\0\x01\0\x02\0"))
        .and_then(|()| file.write_all(&SAMPLE_RATE_HZ.to_le_bytes()))
        .and_then(|()| file.write_all(&(SAMPLE_RATE_HZ * 4).to_le_bytes()))
        .and_then(|()| file.write_all(&4_u16.to_le_bytes()))
        .and_then(|()| file.write_all(&16_u16.to_le_bytes()))
        .and_then(|()| file.write_all(b"data"))
        .and_then(|()| file.write_all(&data_bytes.to_le_bytes()))
        .and_then(|()| file.write_all(pcm))
        .map_err(|error| format!("write WAV artifact: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(root: &Path) -> WavArtifactSelection {
        WavArtifactSelection::new(
            root.join("answer.wav"),
            BootId::from("boot/wav-test"),
            OfferGeneration(1),
        )
        .unwrap()
    }

    #[test]
    fn malformed_or_unfinished_pcm_never_creates_an_artifact() {
        let root = std::env::temp_dir().join(format!(
            "conduit-wav-artifact-refusal-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let destination = root.join("answer.wav");
        let mut session = WavArtifactSession::prepare(selection(&root));
        assert!(session.write_frame(b"not-pcm").is_err());
        assert!(session.finish().is_err());
        assert!(!destination.exists());
        drop(session);
        assert!(!destination.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
