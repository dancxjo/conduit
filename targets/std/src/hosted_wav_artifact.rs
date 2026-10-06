//! Boot-scoped bounded WAV artifact output for explicit evidence destinations.

use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{BootId, OfferGeneration, ResourcePoolId};
use sha2::{Digest, Sha256};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const SAMPLE_RATE_HZ: u32 = 48_000;

#[derive(Clone, Debug)]
pub struct WavArtifactSelection {
    destination: PathBuf,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

impl WavArtifactSelection {
    pub fn is_unpublished(&self) -> bool {
        !self.destination.exists()
    }

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
    temporary: PathBuf,
    file: Option<std::fs::File>,
    bytes: u32,
    digest: Sha256,
    maximum_frames: u64,
    maximum_blocks: u32,
    clock: Option<u64>,
    next_frame: u64,
    blocks: u16,
    completed: bool,
    failed: bool,
}

impl WavArtifactSession {
    #[cfg(test)]
    pub(crate) fn prepare(selection: WavArtifactSelection) -> Self {
        Self::prepare_bounded(selection, 3_072, 16_384).expect("default WAV bounds")
    }
    pub(crate) fn prepare_bounded(
        selection: WavArtifactSelection,
        blocks: u32,
        millis: u32,
    ) -> Result<Self, String> {
        if blocks == 0 || blocks > 32_768 || millis == 0 || millis > 30_000 {
            return Err("invalid admitted WAV work budget".into());
        }
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temporary = selection.destination.with_extension(format!(
            "wav.partial-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Ok(Self {
            selection,
            temporary,
            file: None,
            bytes: 0,
            digest: Sha256::new(),
            maximum_frames: u64::from(millis) * u64::from(SAMPLE_RATE_HZ) / 1000,
            maximum_blocks: blocks,
            clock: None,
            next_frame: 0,
            blocks: 0,
            completed: false,
            failed: false,
        })
    }

    pub(crate) fn write_frame(&mut self, encoded: &[u8]) -> Result<(), String> {
        if self.failed {
            return Err("WAV artifact has already failed".into());
        }
        let result = self.append_frame(encoded);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn append_frame(&mut self, encoded: &[u8]) -> Result<(), String> {
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
            || self
                .next_frame
                .saturating_add(u64::from(header.frame_count))
                > self.maximum_frames
            || u32::from(self.blocks) >= self.maximum_blocks
            || self.clock.is_some_and(|clock| clock != header.clock_id)
        {
            return Err("WAV artifact received unsupported or discontinuous PCM".into());
        }
        if self.file.is_none() {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.temporary)
                .map_err(|e| format!("create partial WAV: {e}"))?;
            file.write_all(&[0; 44]).map_err(|e| e.to_string())?;
            self.file = Some(file);
        }
        self.file
            .as_mut()
            .expect("partial WAV")
            .write_all(payload)
            .map_err(|e| format!("append PCM: {e}"))?;
        self.bytes = self
            .bytes
            .checked_add(payload.len() as u32)
            .ok_or("WAV extent overflow")?;
        self.digest.update(payload);
        self.clock.get_or_insert(header.clock_id);
        self.next_frame += u64::from(header.frame_count);
        self.blocks = self
            .blocks
            .checked_add(1)
            .ok_or("WAV block extent overflow")?;
        Ok(())
    }
    pub(crate) fn finish(&mut self) -> Result<(), String> {
        if self.completed || self.failed || self.bytes == 0 {
            return Err("WAV artifact cannot complete in its current state".into());
        }
        let file = self.file.as_mut().ok_or("WAV has no PCM file")?;
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        write_header(file, self.bytes)?;
        file.sync_all().map_err(|e| e.to_string())?;
        self.file.take();
        std::fs::hard_link(&self.temporary, &self.selection.destination)
            .map_err(|e| format!("publish completed WAV: {e}"))?;
        self.completed = true;
        // Publication succeeded; cleanup failure must not relabel valid output as failed.
        let _ = std::fs::remove_file(&self.temporary);
        Ok(())
    }
    pub(crate) fn report(&self) -> WavArtifactReport {
        WavArtifactReport {
            pcm_bytes: self.bytes,
            frames: self.next_frame as u32,
            blocks: self.blocks,
            completed: self.completed,
        }
    }
    pub(crate) fn content_sha256(&self) -> Option<String> {
        self.completed
            .then(|| format!("{:x}", self.digest.clone().finalize()))
    }
}
impl Drop for WavArtifactSession {
    fn drop(&mut self) {
        self.file.take();
        let _ = std::fs::remove_file(&self.temporary);
    }
}
fn write_header(file: &mut std::fs::File, data_bytes: u32) -> Result<(), String> {
    let riff_bytes = data_bytes
        .checked_add(36)
        .ok_or("WAV RIFF extent overflowed")?;
    file.write_all(b"RIFF")
        .and_then(|()| file.write_all(&riff_bytes.to_le_bytes()))
        .and_then(|()| file.write_all(b"WAVEfmt \x10\0\0\0\x01\0\x02\0"))
        .and_then(|()| file.write_all(&SAMPLE_RATE_HZ.to_le_bytes()))
        .and_then(|()| file.write_all(&(SAMPLE_RATE_HZ * 4).to_le_bytes()))
        .and_then(|()| file.write_all(&4_u16.to_le_bytes()))
        .and_then(|()| file.write_all(&16_u16.to_le_bytes()))
        .and_then(|()| file.write_all(b"data"))
        .and_then(|()| file.write_all(&data_bytes.to_le_bytes()))
        .map_err(|error| format!("write WAV header: {error}"))
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
    #[test]
    fn bounded_long_artifact_is_incremental_and_unfinished_output_is_not_published() {
        let root = std::env::temp_dir().join(format!("conduit-long-wav-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let mut session =
            WavArtifactSession::prepare_bounded(selection(&root), 4000, 20000).unwrap();
        let mut bytes = Vec::new();
        for block in 0..3750_u64 {
            let header = PcmFrameHeader::new(
                PcmSampleRepresentation::Signed16LittleEndian,
                48000,
                PcmChannelLayout::StereoLeftRight,
                256,
                9,
                block * 256,
                false,
            )
            .unwrap();
            bytes.clear();
            bytes.extend_from_slice(&header.encode());
            bytes.resize(bytes.len() + 1024, 0);
            session.write_frame(&bytes).unwrap();
        }
        assert!(!root.join("answer.wav").exists());
        assert_eq!(
            std::fs::metadata(&session.temporary).unwrap().len(),
            44 + 20 * 48000 * 4
        );
        session.finish().unwrap();
        assert_eq!(session.report().frames, 20 * 48000);
        assert!(session.content_sha256().is_some());
        drop(session);
        std::fs::remove_file(root.join("answer.wav")).unwrap();
        let mut unfinished = WavArtifactSession::prepare(selection(&root));
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            48000,
            PcmChannelLayout::StereoLeftRight,
            1,
            9,
            0,
            false,
        )
        .unwrap();
        let mut frame = header.encode().to_vec();
        frame.extend_from_slice(&[0; 4]);
        unfinished.write_frame(&frame).unwrap();
        assert!(
            unfinished.write_frame(&frame).is_err(),
            "repeated frame is discontinuous"
        );
        assert!(
            unfinished.finish().is_err(),
            "a failed write cannot become a completed artifact"
        );
        assert!(!root.join("answer.wav").exists());
        drop(unfinished);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir(root).unwrap();
    }
}
