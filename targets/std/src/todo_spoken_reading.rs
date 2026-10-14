//! Opaque current-Host Todo spoken facet preparation, without output authority.
use crate::{spoken_face_mask::SpokenBatch, StdHost};
use conduit_core::HostAdvertisement;
use conduit_face::{Face, MaskShow};
use conduit_todo_face::{TodoSpokenLook, TodoSpokenWording};
use std::ops::Range;

pub struct PreparedTodoSpokenReading {
    host: HostAdvertisement,
    face: Face,
    look: TodoSpokenLook,
    wording: TodoSpokenWording,
    ranges: Vec<Range<usize>>,
}
impl PreparedTodoSpokenReading {
    pub fn prepare(
        host: &StdHost,
        face: &Face,
        look: TodoSpokenLook,
        page: &[&str],
        locale: &str,
    ) -> Result<Self, String> {
        let wording = host.prepare_todo_spoken_wording(face, look.entry(), page, locale)?;
        if wording.identity() != look.template_identity()
            || wording.source_face() != face.identity.as_str()
            || wording.revision() != face.revision
        {
            return Err("Todo wording differs from selected Look and original Face".into());
        }
        let ranges = crate::spoken_template_reading::frame_ranges(wording.wording())?;
        let reconstructed: String = ranges
            .iter()
            .map(|r| &wording.wording()[r.clone()])
            .collect();
        if reconstructed != wording.wording() {
            return Err("Todo framing changed checked wording".into());
        }
        Ok(Self {
            host: host.advertisement().clone(),
            face: face.clone(),
            look,
            wording,
            ranges,
        })
    }
    pub fn look(&self) -> TodoSpokenLook {
        self.look
    }
    pub fn wording(&self) -> &TodoSpokenWording {
        &self.wording
    }
    /// Full original Face remains available for normal linear inspection.
    pub fn source_face(&self) -> &Face {
        &self.face
    }
    pub fn frames_count(&self) -> usize {
        self.ranges.len()
    }
    pub fn framed_wording(&self, index: usize) -> Result<&str, String> {
        let range = self
            .ranges
            .get(index)
            .ok_or("Todo frame exceeds admitted reading")?;
        Ok(&self.wording.wording()[range.clone()])
    }
    pub fn validate_current(&self, host: &StdHost, face: &Face) -> Result<(), String> {
        if host.advertisement() != &self.host || face != &self.face {
            return Err("Todo reading has stale Host or Face".into());
        }
        Ok(())
    }
    /// Prepare one bounded batch under an existing current available Show.
    /// This does not execute output or advance a reader cursor.
    pub fn prepare_frame_batch(
        &self,
        host: &StdHost,
        face: &Face,
        show: &MaskShow,
        index: usize,
    ) -> Result<SpokenBatch, String> {
        self.validate_current(host, face)?;
        let words = self.framed_wording(index)?;
        let batch = SpokenBatch::from_accepted_wording(
            face,
            show,
            words,
            format!(
                "todo-template/{}/{}/{index}",
                self.wording.plan().as_str(),
                show.show_id.as_str()
            ),
        )
        .map_err(|e| format!("Todo spoken frame: {e:?}"))?;
        let reconstructed: String = batch
            .segments
            .iter()
            .map(|s| s.segment.text.as_str())
            .collect();
        if reconstructed != words {
            return Err("Todo speech framing changed checked wording".into());
        }
        Ok(batch)
    }
}
