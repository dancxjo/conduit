//! Opaque current-Host preparation of one checked Look across bounded speech Plays.
//! Frames confer no cursor advancement, Show authority or output acknowledgement.
use crate::{spoken_face_mask::SpokenBatch, StdHost};
use conduit_core::HostAdvertisement;
use conduit_face::{Face, MaskShow, SpokenTemplateWording};
use sha2::{Digest, Sha256};
use std::ops::Range;

pub const MAX_SPOKEN_TEMPLATE_FRAME_BYTES: usize = 96;
pub const MAX_SPOKEN_TEMPLATE_FRAMES: usize = 64;

pub struct PreparedSpokenTemplateReading {
    host: HostAdvertisement,
    face: Face,
    receipt: SpokenTemplateWording,
    ranges: Vec<Range<usize>>,
}

impl PreparedSpokenTemplateReading {
    /// Only the actual current Host's checked-source factory can supply wording.
    /// A caller-supplied string or documentary receipt never admits a reading.
    pub fn prepare(host: &StdHost, face: &Face, entry: &str) -> Result<Self, String> {
        let receipt = host.prepare_spoken_template_wording(face, entry)?;
        if receipt.source_face_identity != face.identity.as_str()
            || receipt.source_face_revision != face.revision
            || receipt.wording_digest
                != <[u8; 32]>::from(Sha256::digest(receipt.wording.as_bytes()))
            || receipt.segments.join(" ") != receipt.wording
        {
            return Err("checked spoken template receipt differs from exact source wording".into());
        }
        let ranges = frame_ranges(&receipt.wording)?;
        let reconstructed: String = ranges
            .iter()
            .map(|range| &receipt.wording[range.clone()])
            .collect();
        if reconstructed != receipt.wording
            || <[u8; 32]>::from(Sha256::digest(reconstructed.as_bytes())) != receipt.wording_digest
        {
            return Err("spoken template framing changed source bytes or digest".into());
        }
        Ok(Self {
            host: host.advertisement().clone(),
            face: face.clone(),
            receipt,
            ranges,
        })
    }

    pub fn receipt(&self) -> &SpokenTemplateWording {
        &self.receipt
    }
    pub fn frames_count(&self) -> usize {
        self.ranges.len()
    }
    pub fn opening_wording(&self) -> &str {
        &self.receipt.wording[self.ranges[0].clone()]
    }
    pub fn framed_wording(&self, index: usize) -> Result<&str, String> {
        let range = self
            .ranges
            .get(index)
            .ok_or("spoken template frame index is outside admitted reading")?;
        Ok(&self.receipt.wording[range.clone()])
    }

    pub fn validate_current(&self, host: &StdHost, face: &Face) -> Result<(), String> {
        if host.advertisement() != &self.host || face != &self.face {
            return Err("spoken template reading has stale current Host or Face".into());
        }
        Ok(())
    }

    pub fn prepare_frame_batch(
        &self,
        host: &StdHost,
        face: &Face,
        show: &MaskShow,
        index: usize,
    ) -> Result<SpokenBatch, String> {
        self.validate_current(host, face)?;
        let wording = self.framed_wording(index)?;
        let identity = format!(
            "spoken-template/{}/{}/{index}",
            self.receipt.producer_plan_id.as_str(),
            show.show_id.as_str()
        );
        let batch = SpokenBatch::from_accepted_wording(face, show, wording, identity)
            .map_err(|error| format!("prepare current spoken template frame: {error:?}"))?;
        let reconstructed: String = batch
            .segments
            .iter()
            .map(|segment| segment.segment.text.as_str())
            .collect();
        if reconstructed != wording {
            return Err("speech batch changed exact template frame".into());
        }
        Ok(batch)
    }
}

fn word_boundary(text: &str, offset: usize) -> bool {
    text.is_char_boundary(offset)
        && (offset == 0
            || offset == text.len()
            || text[..offset]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
            || text[offset..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace))
}

fn frame_ranges(text: &str) -> Result<Vec<Range<usize>>, String> {
    if text.is_empty() || text.len() > conduit_face::MAX_SPOKEN_TEMPLATE_BYTES {
        return Err("spoken template script is empty or exceeds admitted bound".into());
    }
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < text.len() {
        if ranges.len() == MAX_SPOKEN_TEMPLATE_FRAMES {
            return Err("spoken template exceeds finite frame count".into());
        }
        let remaining = &text[start..];
        let maximum = remaining.len().min(MAX_SPOKEN_TEMPLATE_FRAME_BYTES);
        let end = (1..=maximum)
            .rev()
            .find(|end| word_boundary(remaining, *end))
            .ok_or("spoken template contains an overlong frame word")?;
        if remaining[..end].trim().is_empty() {
            return Err("spoken template framing would produce an empty speech turn".into());
        }
        // Established Tongues splitting may divide a long word inside this frame,
        // preserving every UTF8 byte. A 96B frame needs at most four segments
        // under its current 64B/halfway-whitespace splitter; never relax that bound.
        if conduit_tongues::MAXIMUM_COMMITTED_SEGMENTS < 9 {
            return Err("current Tongues segment capacity cannot admit this frame".into());
        }
        ranges.push(start..start + end);
        start += end;
    }
    Ok(ranges)
}

#[cfg(test)]
#[path = "spoken_template_reading/tests.rs"]
mod tests;
