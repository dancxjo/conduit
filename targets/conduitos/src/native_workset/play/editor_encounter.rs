//! Native editing observes a document; only its connected state Back commits it.
use super::*;
use conduit_semantic_catalog::{TextDocument, TextEdit};

#[derive(Clone, Copy)]
pub(super) struct EditorObservation {
    pub(super) revision: u64,
    text: NativeFace,
}
impl EditorObservation {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, PlayRefusal> {
        let document = TextDocument::decode(bytes).map_err(|_| PlayRefusal::Kernel)?;
        Ok(Self {
            revision: document.revision,
            text: NativeFace::new(document.text)?,
        })
    }
    /// The finite physical profile appends, inserts newline, and deletes one
    /// Unicode scalar at the end. No local mutation predicts owner acceptance.
    fn edit(&self, fragment: &[u8], output: &mut [u8]) -> Result<Option<usize>, PlayRefusal> {
        let text = self.text.text();
        let end = text.chars().count() as u32;
        let (start, insert_text) = if fragment == b"\x08" {
            if end == 0 {
                return Ok(None);
            }
            (end - 1, &[][..])
        } else {
            (end, fragment)
        };
        TextEdit {
            base_revision: self.revision,
            start,
            end,
            insert_text,
        }
        .encode_into(output)
        .map(Some)
        .map_err(|_| PlayRefusal::Kernel)
    }
}

impl NativeWorksetPlay {
    pub(super) fn present_editor(
        &mut self,
        plot: usize,
        request: HostCallRequest,
    ) -> Result<(), PlayRefusal> {
        let bytes = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|_| PlayRefusal::Kernel)?;
        let observed = EditorObservation::decode(bytes)?;
        if self.pending[plot].is_some() || self.faces[plot].is_some() {
            return Err(PlayRefusal::InputPressure);
        }
        self.faces[plot] = Some(observed.text);
        self.editor_observations[plot] = Some(observed);
        self.pending[plot] = Some(request);
        Ok(())
    }

    pub(super) fn editor_key(&mut self, plot: usize, event: KeyEvent) -> Result<bool, PlayRefusal> {
        let observed = self.editor_observations[plot].ok_or(PlayRefusal::Preparation)?;
        let Some(fragment) = self.keymaps[plot]
            .apply(event)
            .map_err(|_| PlayRefusal::Kernel)?
        else {
            return Ok(true);
        };
        let mut encoded = [0; conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES as usize];
        let Some(length) = observed.edit(fragment.as_slice(), &mut encoded)? else {
            return Ok(true);
        };
        let request = self.pending[plot].ok_or(PlayRefusal::InputPressure)?;
        self.output(request, Some(&encoded[..length]))?;
        self.pending[plot] = None;
        self.drive()?;
        Ok(true)
    }
}

impl NativeWorksetPlay {
    /// Load authorized Source through the already selected editor Host Call.
    /// The caller owns Source/provenance authorization; this boundary owns exact
    /// editor admission and refuses destructive replacement of an existing draft.
    pub(crate) fn open_source_editor(
        &mut self,
        plot: usize,
        source: &str,
    ) -> Result<(), PlayRefusal> {
        self.ensure_applications_live()?;
        if self.cancelled || self.active_body_play.is_none() {
            return Err(PlayRefusal::Cancelled);
        }
        if source.len() > 256 {
            return Err(PlayRefusal::EditorSourceBound);
        }
        let owner = self
            .input_owners
            .get(plot)
            .and_then(Option::as_ref)
            .ok_or(PlayRefusal::InputOwnership)?;
        if owner.kind_id.as_str() != conduit_semantic_catalog::FACE_EDITOR_KIND
            || owner.value_kind.as_str() != conduit_semantic_catalog::TEXT_EDIT_VALUE_KIND
            || owner.port_id.as_str() != "edit"
        {
            return Err(PlayRefusal::InputOwnership);
        }
        let observed = self.editor_observations[plot].ok_or(PlayRefusal::Preparation)?;
        if !observed.text.text().is_empty() {
            return Err(PlayRefusal::EditorDraftExists);
        }
        if self.faces[plot].is_some() {
            return Err(PlayRefusal::InputPressure);
        }
        let request = self.pending[plot].ok_or(PlayRefusal::InputPressure)?;
        if self.binding(request.node)?.effect != Effect::Editor {
            return Err(PlayRefusal::InputOwnership);
        }
        let expected = observed
            .revision
            .checked_add(1)
            .ok_or(PlayRefusal::Kernel)?;
        let mut bytes = [0; conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES as usize];
        let length = TextEdit {
            base_revision: observed.revision,
            start: 0,
            end: 0,
            insert_text: source.as_bytes(),
        }
        .encode_into(&mut bytes)
        .map_err(|_| PlayRefusal::Kernel)?;
        self.output(request, Some(&bytes[..length]))?;
        self.pending[plot] = None;
        self.drive()?;
        let current = self.editor_observations[plot].ok_or(PlayRefusal::Kernel)?;
        if current.revision != expected || current.text.text() != source {
            return Err(PlayRefusal::Kernel);
        }
        Ok(())
    }
}
