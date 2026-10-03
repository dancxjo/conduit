//! Hosted terminal realization of an exact Face, independent of its producer.
//!
//! Formatting and flushing never mint a Show. `TerminalMaskExecution` is the
//! integration seam for the ordinary planned Mask and its actual Host Calls.
//! Local reading/focus/paging do not invent portable navigation or app state.
use conduit_presentation::{
    FaceInteraction, FaceInteractionRefusal, ManifestationLifecycle, MaskInteractionCorrelation,
    MaskShow, Presentation,
};
use std::io::Write;

#[path = "terminal_face_mask/document.rs"]
mod document;
#[path = "terminal_face_mask/input.rs"]
mod input;
#[path = "terminal_face_mask/keyboard.rs"]
mod keyboard;
pub use input::{TerminalControl, TerminalInput, TerminalInputOutcome};
pub use keyboard::TerminalKeyboard;

pub const MAX_TERMINAL_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_TERMINAL_ROWS: usize = 65_536;
pub const MAX_TERMINAL_DRAFT_BYTES: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalError {
    InvalidFace,
    InvalidExtent,
    DocumentPressure,
    StaleFace,
    StaleShow,
    UnacknowledgedShow,
    UnknownControl,
    UnsupportedInput,
    InputPressure,
    MalformedInput,
    Interaction(FaceInteractionRefusal),
    Io(std::io::ErrorKind),
    Execution(String),
}
impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "terminal Mask: {self:?}")
    }
}
impl std::error::Error for TerminalError {}

/// Constructed only after this renderer writes and flushes the exact frame.
/// This is effect evidence, not an Available Show or a membership credential.
pub struct TerminalEffectReceipt {
    prepared_show: MaskShow,
    frame_sequence: u64,
    bytes_written: usize,
    digest: [u8; 32],
}
impl TerminalEffectReceipt {
    pub fn prepared_show(&self) -> &MaskShow {
        &self.prepared_show
    }
    pub fn frame_sequence(&self) -> u64 {
        self.frame_sequence
    }
    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }
    pub fn frame_digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

/// The implementation must retain the SAME ordinary Mask execution across all
/// methods. `begin_render` stops at its actual renderer Host Call;
/// `complete_render` completes that call and observes the actual Show Fore;
/// `interact` returns only after the typed interaction traverses its declared
/// Fore. This trait does not grant a formatter authority to fabricate a Show.
pub trait TerminalMaskExecution {
    fn begin_render(&mut self, face: &Presentation) -> Result<MaskShow, TerminalError>;
    fn complete_render(
        &mut self,
        receipt: &TerminalEffectReceipt,
    ) -> Result<MaskShow, TerminalError>;
    fn interact(
        &mut self,
        interaction: FaceInteraction,
    ) -> Result<MaskInteractionCorrelation, TerminalError>;
    fn close_without_input(&mut self) -> Result<(), TerminalError>;
    fn cancel(&mut self) -> Result<(), TerminalError>;
}

pub struct TerminalFaceMask {
    face: Presentation,
    columns: usize,
    rows: usize,
    document: Vec<document::Row>,
    inspect_document: Vec<document::Row>,
    top: usize,
    reading: usize,
    focus: Option<TerminalControl>,
    inspect: bool,
    drafts: Vec<Vec<Option<Vec<u8>>>>,
    active_show: Option<MaskShow>,
    pending_frame: Option<(u64, String, [u8; 32])>,
    frame_sequence: u64,
    submitted: bool,
}
impl TerminalFaceMask {
    pub fn prepare(face: Presentation, columns: usize, rows: usize) -> Result<Self, TerminalError> {
        face.validate().map_err(|_| TerminalError::InvalidFace)?;
        if !(40..=240).contains(&columns) || !(8..=100).contains(&rows) {
            return Err(TerminalError::InvalidExtent);
        }
        let (document, inspect_document) = document::prepare(&face, columns - 4)?;
        // Draft vectors have exact declared argument counts. Byte buffers are
        // allocated while editing, with a total 8192-byte cap; this is host UI
        // preparation, not growth hidden inside a running kernel Step.
        let drafts = face
            .actions
            .iter()
            .map(|a| vec![None; a.arguments.len()])
            .collect();
        Ok(Self {
            face,
            columns,
            rows,
            document,
            inspect_document,
            top: 0,
            reading: 0,
            focus: None,
            inspect: false,
            drafts,
            active_show: None,
            pending_frame: None,
            frame_sequence: 0,
            submitted: false,
        })
    }
    pub fn presentation(&self) -> &Presentation {
        &self.face
    }
    pub fn show(&self) -> Option<&MaskShow> {
        self.active_show.as_ref()
    }
    pub fn focused(&self) -> Option<TerminalControl> {
        self.focus
    }
    pub fn reading_clause_index(&self) -> Option<usize> {
        self.current_rows().get(self.reading).and_then(|r| r.clause)
    }
    pub fn inspecting(&self) -> bool {
        self.inspect
    }
    fn current_rows(&self) -> &[document::Row] {
        if self.inspect {
            &self.inspect_document
        } else {
            &self.document
        }
    }
    fn page_rows(&self) -> usize {
        self.rows - 5
    }

    /// Actual output effect only. A failed/partial write or flush yields no
    /// receipt and invalidates old input immediately.
    pub fn render<W: Write + ?Sized>(
        &mut self,
        output: &mut W,
        prepared: &MaskShow,
    ) -> Result<TerminalEffectReceipt, TerminalError> {
        use sha2::{Digest, Sha256};
        self.active_show = None;
        self.pending_frame = None;
        prepared
            .validate(&self.face)
            .map_err(|_| TerminalError::StaleFace)?;
        if prepared.show.lifecycle != ManifestationLifecycle::Prepared {
            return Err(TerminalError::UnacknowledgedShow);
        }
        let frame = document::frame(self);
        output
            .write_all(frame.as_bytes())
            .and_then(|_| output.flush())
            .map_err(|e| TerminalError::Io(e.kind()))?;
        self.frame_sequence = self
            .frame_sequence
            .checked_add(1)
            .ok_or(TerminalError::DocumentPressure)?;
        let digest = Sha256::digest(frame.as_bytes()).into();
        self.pending_frame = Some((
            self.frame_sequence,
            prepared.show_id.as_str().into(),
            digest,
        ));
        Ok(TerminalEffectReceipt {
            prepared_show: prepared.clone(),
            frame_sequence: self.frame_sequence,
            bytes_written: frame.len(),
            digest,
        })
    }

    /// Only the acknowledged result from the ordinary Mask execution enables
    /// input. Old receipts, another exact Show, and non-Available states refuse.
    pub fn bind_show(
        &mut self,
        receipt: TerminalEffectReceipt,
        available: MaskShow,
    ) -> Result<(), TerminalError> {
        let expected = (
            receipt.frame_sequence,
            receipt.prepared_show.show_id.as_str().into(),
            receipt.digest,
        );
        if self.pending_frame.take() != Some(expected) {
            return Err(TerminalError::StaleShow);
        }
        available
            .validate(&self.face)
            .map_err(|_| TerminalError::StaleFace)?;
        if available.show_id != receipt.prepared_show.show_id
            || available.planned_mask != receipt.prepared_show.planned_mask
        {
            return Err(TerminalError::StaleShow);
        }
        if available.show.lifecycle != ManifestationLifecycle::Available {
            return Err(TerminalError::UnacknowledgedShow);
        }
        self.active_show = Some(available);
        self.submitted = false;
        Ok(())
    }

    pub fn present<W: Write + ?Sized, E: TerminalMaskExecution>(
        &mut self,
        execution: &mut E,
        output: &mut W,
    ) -> Result<(), TerminalError> {
        let result = (|| {
            let prepared = execution.begin_render(&self.face)?;
            let receipt = self.render(output, &prepared)?;
            let available = execution.complete_render(&receipt)?;
            self.bind_show(receipt, available)
        })();
        if result.is_err() {
            self.active_show = None;
            let _ = execution.cancel();
        }
        result
    }

    fn check_show(&self, expected: &MaskShow) -> Result<(), TerminalError> {
        if expected.presentation_id != self.face.identity
            || expected.presentation_revision != self.face.revision
        {
            return Err(TerminalError::StaleFace);
        }
        if self.active_show.as_ref() != Some(expected) || self.submitted {
            return Err(TerminalError::StaleShow);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "terminal_face_mask/tests.rs"]
mod tests;
