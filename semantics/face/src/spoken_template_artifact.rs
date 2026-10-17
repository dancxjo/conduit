//! Inspectable opening-artifact metadata, never template, Plan or effect authority.
use crate::{Face, SpokenTemplateWording, MAX_SPOKEN_TEMPLATE_BYTES, SPOKEN_TEMPLATE_LOOKS_SOURCE};
use alloc::string::String;
use conduit_core::PlotIdentity;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenTemplateOpeningEvidence {
    /// Complete checked template wording, including material not in this WAV.
    pub wording: SpokenTemplateWording,
    /// Exact bounded prefix handed to the opening synthesis Play.
    pub opening_wording: String,
    /// Bounded full-script framing metadata; remaining frames are not proven spoken.
    pub frame_count: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpokenTemplateOpeningRefusal {
    Face,
    Source,
    Wording,
    Coverage,
    Opening,
    Frames,
}

impl SpokenTemplateOpeningEvidence {
    /// Validates receipt correspondence only. Public metadata can be forged;
    /// the owner must compare its privately selected source Plan and receipt.
    /// This opening artifact does not prove remaining wording was synthesized,
    /// played, captured, or heard, and creates no current Look/Show grant.
    pub fn validate(&self, face: &Face) -> Result<(), SpokenTemplateOpeningRefusal> {
        use SpokenTemplateOpeningRefusal as Refusal;
        face.validate().map_err(|_| Refusal::Face)?;
        let wording = &self.wording;
        if wording.source_face_identity != face.identity.as_str()
            || wording.source_face_revision != face.revision
        {
            return Err(Refusal::Face);
        }
        let entry = match (
            wording.template_identity.as_str(),
            wording.template_revision,
        ) {
            ("spoken/brief@1", 1) => "spoken/brief-template",
            ("spoken/expanded@1", 1) => "spoken/expanded-template",
            ("spoken/control@1", 1) => "spoken/control-template",
            _ => return Err(Refusal::Source),
        };
        let source_digest: [u8; 32] =
            Sha256::digest(SPOKEN_TEMPLATE_LOOKS_SOURCE.as_bytes()).into();
        if wording.template_source_digest != source_digest
            || wording.producer_plan_id.as_str().is_empty()
        {
            return Err(Refusal::Source);
        }
        let checked = check_syntax_document(
            &parse_syntax_document(SPOKEN_TEMPLATE_LOOKS_SOURCE),
            &StartupCatalog::new(),
        )
        .map_err(|_| Refusal::Source)?;
        let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
            .map_err(|_| Refusal::Source)?
            .expanded;
        let expected_plot = PlotIdentity {
            source_document_id: expanded.source_document_id,
            checked_plot_id: expanded.checked_plot_id,
            expanded_plot_id: expanded.expanded_plot_id,
        };
        if wording.producer_plot != expected_plot {
            return Err(Refusal::Source);
        }
        let digest: [u8; 32] = Sha256::digest(wording.wording.as_bytes()).into();
        if wording.wording.is_empty()
            || wording.wording.len() > MAX_SPOKEN_TEMPLATE_BYTES
            || wording.wording_digest != digest
            || wording.segments.is_empty()
            || wording.segments.len() > 9
            || wording
                .segments
                .iter()
                .any(|words| words.is_empty() || words.len() > MAX_SPOKEN_TEMPLATE_BYTES)
            || wording.segments.join(" ") != wording.wording
        {
            return Err(Refusal::Wording);
        }
        let (expected_segments, expected_wording) =
            crate::spoken_template_looks::expected_spoken_template_wording(face, entry)
                .map_err(|_| Refusal::Coverage)?;
        if wording.segments != expected_segments || wording.wording != expected_wording {
            return Err(Refusal::Coverage);
        }
        let plan = crate::plan_face_utterances(face).map_err(|_| Refusal::Face)?;
        if wording.utterance_plan_digest != plan.digest {
            return Err(Refusal::Coverage);
        }
        let selected = if entry == "spoken/control-template" {
            crate::prepare_spoken_control_slots(face, &[]).map_err(|_| Refusal::Coverage)?
        } else {
            crate::prepare_spoken_template_slots(face, &[]).map_err(|_| Refusal::Coverage)?
        };
        if wording.selected_clause_indices != selected.selected_clause_indices()
            || wording.selected_action_ids != selected.selected_action_ids()
            || selected.selected_clause_indices().iter().any(|index| {
                plan.clauses
                    .get(*index as usize)
                    .is_none_or(|clause| !wording.wording.contains(&clause.text))
            })
        {
            return Err(Refusal::Coverage);
        }
        if self.opening_wording.is_empty()
            || self.opening_wording.len() > 256
            || !wording.wording.is_char_boundary(self.opening_wording.len())
            || !wording.wording.starts_with(&self.opening_wording)
        {
            return Err(Refusal::Opening);
        }
        let frames = usize::from(self.frame_count);
        if !(1..=64).contains(&frames)
            || frames > wording.wording.len()
            || frames * 256 < wording.wording.len()
        {
            return Err(Refusal::Frames);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "spoken_template_artifact_tests.rs"]
mod tests;
