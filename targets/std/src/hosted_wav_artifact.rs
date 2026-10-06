//! Bounded WAV artifacts at fixed or exact per-Play destinations.

use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{ActivePlayId, BootId, OfferGeneration, PlacementId, PlanId, ResourcePoolId};
use sha2::{Digest, Sha256};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SAMPLE_RATE_HZ: u32 = 48_000;
// Sixty-four retained 30-second stereo PCM Shows admit at most 368,642,816
// bytes including WAV headers. Reserve and partial names can coexist until
// a Play finishes or cancels, hence the larger finite directory scan bound.
const MAX_RETAINED_ARTIFACTS: usize = 64;
const MAX_WAV_BYTES: u64 = 44 + 30 * 48_000_u64 * 4;
const MAX_DIRECTORY_ENTRIES: usize = 3 * MAX_RETAINED_ARTIFACTS + 1;

#[derive(Debug)]
struct QuotaReservation(PathBuf);

impl Drop for QuotaReservation {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct QuotaLock(PathBuf);

impl QuotaLock {
    fn acquire(root: &Path) -> Result<Self, String> {
        let path = root.join(".quota-lock");
        std::fs::create_dir(&path).map_err(|e| format!("reserve WAV artifact quota lock: {e}"))?;
        Ok(Self(path))
    }
}

impl Drop for QuotaLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.0);
    }
}

#[derive(Clone, Debug)]
pub struct WavArtifactSelection {
    destination: PathBuf,
    per_play: bool,
    reservation: Option<Arc<QuotaReservation>>,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

impl WavArtifactSelection {
    pub fn is_unpublished(&self) -> bool {
        if self.per_play {
            self.has_retained_capacity(false).unwrap_or(false)
        } else {
            !self.destination.exists()
        }
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
            per_play: false,
            reservation: None,
            boot_id,
            offer_generation,
        })
    }

    /// Select a retained artifact directory before Boot advertisement. Each
    /// subsequent Play reserves one exact create-new name and bounded extent.
    pub fn per_play_root(
        root: impl AsRef<Path>,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, String> {
        let root = root.as_ref();
        if !root.is_dir()
            || root
                .symlink_metadata()
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
        {
            return Err("selected WAV artifact root is unavailable".into());
        }
        let root = root
            .canonicalize()
            .map_err(|e| format!("resolve selected WAV artifact root: {e}"))?;
        if root.to_str().is_none() || root.as_os_str().as_encoded_bytes().len() > 2_048 {
            return Err("selected WAV artifact root cannot provide a bounded locator".into());
        }
        let selection = Self {
            destination: root,
            per_play: true,
            reservation: None,
            boot_id,
            offer_generation,
        };
        if !selection.has_retained_capacity(false)? {
            return Err("selected WAV artifact retained quota is full".into());
        }
        Ok(selection)
    }

    fn has_retained_capacity(&self, owns_lock: bool) -> Result<bool, String> {
        let mut count = 0usize;
        let mut bytes = 0u64;
        let mut entries = 0usize;
        for entry in std::fs::read_dir(&self.destination).map_err(|e| e.to_string())? {
            entries += 1;
            if entries > MAX_DIRECTORY_ENTRIES {
                return Err("WAV artifact directory exceeds its finite scan bound".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name() == ".quota-lock" {
                if !owns_lock {
                    return Ok(false);
                }
                continue;
            }
            let metadata = entry.path().symlink_metadata().map_err(|e| e.to_string())?;
            if !metadata.is_file() {
                return Err("WAV artifact root contains an unsupported entry".into());
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".wav")
                || name.ends_with(".reserve")
                || name.contains(".wav.partial-")
            {
                count += 1;
                bytes = bytes
                    .checked_add(if name.ends_with(".wav") {
                        metadata.len()
                    } else {
                        MAX_WAV_BYTES
                    })
                    .ok_or("WAV artifact quota overflow")?;
            } else {
                return Err("WAV artifact directory contains an unknown file".into());
            }
        }
        Ok(count < MAX_RETAINED_ARTIFACTS
            && bytes
                .checked_add(MAX_WAV_BYTES)
                .is_some_and(|required| required <= MAX_RETAINED_ARTIFACTS as u64 * MAX_WAV_BYTES))
    }

    pub fn for_play(
        &self,
        plan_id: &PlanId,
        active_play_id: &ActivePlayId,
        placement_id: &PlacementId,
    ) -> Result<Self, String> {
        if !self.per_play {
            return Ok(self.clone());
        }
        let _lock = QuotaLock::acquire(&self.destination)?;
        if !self.has_retained_capacity(true)? {
            return Err("selected WAV artifact retained quota is full".into());
        }
        let mut digest = Sha256::new();
        for part in [
            self.boot_id.as_str(),
            plan_id.as_str(),
            active_play_id.as_str(),
            placement_id.as_str(),
        ] {
            digest.update((part.len() as u64).to_le_bytes());
            digest.update(part.as_bytes());
        }
        let destination = self
            .destination
            .join(format!("play-{:x}.wav", digest.finalize()));
        if destination.exists() {
            return Err("exact WAV artifact Play identity is already published".into());
        }
        let reservation_path = destination.with_extension("reserve");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&reservation_path)
            .map_err(|e| format!("reserve exact WAV artifact Play: {e}"))?;
        let mut exact = Self::new(destination, self.boot_id.clone(), self.offer_generation)?;
        exact.reservation = Some(Arc::new(QuotaReservation(reservation_path)));
        Ok(exact)
    }

    pub fn locator(&self) -> Option<String> {
        self.destination.to_str().map(str::to_owned)
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
        if selection.per_play {
            return Err("WAV artifact Play has no exact reserved destination".into());
        }
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
    pub(crate) fn locator(&self) -> Option<String> {
        self.completed.then(|| self.selection.locator()).flatten()
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
#[path = "hosted_wav_artifact_tests.rs"]
mod tests;
