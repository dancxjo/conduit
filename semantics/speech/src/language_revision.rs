//! Prepared ASR-to-language revisions. The original envelope remains the exact
//! recognition evidence; this adapter grants no playback commitment or accuracy.
use crate::semantic::*;
use alloc::{string::String, vec::Vec};
use conduit_language::{
    validate_text_revision, LanguageTextFinality, LanguageTextPriorRevision, LanguageTextRevision,
    LinguisticDerivationProvenance, TextRevisionRefusal,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

pub struct AsrLanguageBasis<'a> {
    pub stream: &'a ListeningStreamId,
    pub segment: &'a ListeningSegmentId,
    pub text: &'a LanguageTextId,
    pub language: &'a LanguageId,
}
pub struct PreparedAsrRevision<'a> {
    envelope: &'a AsrRecognitionEnvelope,
    segment: &'a ListeningSegmentId,
    revision: LanguageTextRevision,
}
impl PreparedAsrRevision<'_> {
    pub fn envelope(&self) -> &AsrRecognitionEnvelope {
        self.envelope
    }
    pub fn segment(&self) -> &ListeningSegmentId {
        self.segment
    }
    pub fn revision(&self) -> &LanguageTextRevision {
        &self.revision
    }
}
// The finite owned revision stays inline; cancellation needs only references.
// Boxing would add an allocation solely to balance these bounded outcomes.
#[allow(clippy::large_enum_variant)]
pub enum PreparedAsrChange<'a> {
    Revision(PreparedAsrRevision<'a>),
    Cancelled {
        envelope: &'a AsrRecognitionEnvelope,
        previous: &'a PreparedAsrRevision<'a>,
    },
}
#[derive(Debug)]
pub enum AsrLanguageRefusal {
    Stream,
    Segment,
    Source,
    Language,
    Sequence,
    Role,
    Unsupported,
    MissingPrevious,
    ReplacementRange,
    StableUtf8Boundary,
    StableSnapshot,
    Native(NativeBindingRefusal),
    Revision(TextRevisionRefusal),
}

/// Convert the provider snapshot's byte frontier explicitly to Unicode scalars.
/// Candidate identity and text must match the supplied segment/material.
/// The split fields must describe that same snapshot; malformed snapshots refuse.
pub fn stable_scalar_prefix(
    basis: &AsrLanguageBasis<'_>,
    expected_text: &str,
    candidate: &AsrTranscriptCandidate,
) -> Result<u32, AsrLanguageRefusal> {
    if candidate.candidate_id() != basis.segment {
        return Err(AsrLanguageRefusal::Segment);
    }
    if candidate.text() != expected_text {
        return Err(AsrLanguageRefusal::StableSnapshot);
    }
    let text = candidate.text().as_str();
    let bytes = *candidate.stable_prefix_utf8_bytes() as usize;
    let Some(stable) = text.get(..bytes) else {
        return Err(AsrLanguageRefusal::StableUtf8Boundary);
    };
    if stable != candidate.stable_text() || &text[bytes..] != candidate.unstable_text() {
        return Err(AsrLanguageRefusal::StableSnapshot);
    }
    Ok(stable.chars().count() as u32)
}

/// Supply revision identity, linguistic derivation provenance and consumer
/// commitment explicitly. Partial events replace the whole candidate; revised
/// events replace exactly their scalar range. Final ASR is not played audio.
#[allow(clippy::too_many_arguments)]
pub fn prepare_asr_revision<'a>(
    basis: &AsrLanguageBasis<'a>,
    envelope: &'a AsrRecognitionEnvelope,
    previous: Option<&'a PreparedAsrRevision<'a>>,
    revision_id: LanguageTextRevisionId,
    provenance: LinguisticDerivationProvenance,
    stable_prefix: Option<u32>,
    committed_prefix: u32,
    maximum_revisable_scalars: u32,
) -> Result<PreparedAsrChange<'a>, AsrLanguageRefusal> {
    use AsrLanguageRefusal::*;
    if envelope.stream_id() != basis.stream {
        return Err(Stream);
    }
    if let Some(old) = previous {
        if old.envelope.stream_id() != basis.stream {
            return Err(Stream);
        }
        if old.segment != basis.segment {
            return Err(Segment);
        }
        if old.revision.material().identity() != basis.text
            || old.revision.material().language() != basis.language
        {
            return Err(Source);
        }
        if envelope.sequence() <= old.envelope.sequence()
            || envelope.event_id() == old.envelope.event_id()
        {
            return Err(Sequence);
        }
    }
    let (segment, role, content, finality) = match envelope.event() {
        AsrRecognitionEvent::PartialHypothesis(event) => (
            event.segment_id(),
            event.role(),
            event.text().clone(),
            LanguageTextFinality::Partial,
        ),
        AsrRecognitionEvent::RevisedHypothesis(event) => {
            let old = previous.ok_or(MissingPrevious)?;
            let mut chars: Vec<_> = old.revision.material().text().chars().collect();
            let start = *event.replaces().start() as usize;
            let end = *event.replaces().end() as usize;
            if start > end || end > chars.len() {
                return Err(ReplacementRange);
            }
            chars.splice(start..end, event.text().chars());
            (
                event.segment_id(),
                event.role(),
                chars.into_iter().collect::<String>(),
                LanguageTextFinality::Partial,
            )
        }
        AsrRecognitionEvent::CommittedSegment(event) => {
            if event
                .language()
                .as_ref()
                .is_some_and(|hypothesis| hypothesis.language() != basis.language)
            {
                return Err(Language);
            }
            (
                event.segment_id(),
                event.role(),
                event.text().clone(),
                LanguageTextFinality::Final,
            )
        }
        AsrRecognitionEvent::HypothesisCancelled(event) => {
            if event.segment_id() != basis.segment {
                return Err(Segment);
            }
            if !matches!(event.role(), ListeningTextRole::Recognition) {
                return Err(Role);
            }
            return Ok(PreparedAsrChange::Cancelled {
                envelope,
                previous: previous.ok_or(MissingPrevious)?,
            });
        }
        _ => return Err(Unsupported),
    };
    if segment != basis.segment {
        return Err(Segment);
    }
    if !matches!(role, ListeningTextRole::Recognition) {
        return Err(Role);
    }
    let prior = previous
        .map(|old| {
            LanguageTextPriorRevision::new(
                old.revision.material().revision().clone(),
                *old.revision.sequence(),
            )
        })
        .transpose()
        .map_err(Native)?;
    let sequence = previous
        .map_or(Some(0), |old| old.revision.sequence().checked_add(1))
        .ok_or(Sequence)?;
    let stable = stable_prefix.or_else(|| previous.and_then(|old| *old.revision.stable_prefix()));
    let material = LanguageText::new(
        basis.text.clone(),
        basis.language.clone(),
        revision_id,
        content,
    )
    .map_err(Native)?;
    let revision =
        LanguageTextRevision::new(finality, material, prior, provenance, sequence, stable)
            .map_err(Native)?;
    validate_text_revision(
        previous.map(|old| &old.revision),
        &revision,
        committed_prefix,
        maximum_revisable_scalars,
    )
    .map_err(Revision)?;
    Ok(PreparedAsrChange::Revision(PreparedAsrRevision {
        envelope,
        segment,
        revision,
    }))
}
