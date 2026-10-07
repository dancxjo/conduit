//! Immutable native tape and exact source/analysis basis for output feedback.
//! Full admitted facts remain borrowed; numeric queue delivery is not playback.
use crate::{
    intent_realization::PreparedIntentRealization,
    linguistic_prosody::LinguisticProsodyBasis,
    pitch_trajectory::{PreparedLinguisticPitch, PreparedUtterancePitch},
    semantic::*,
    Renderer,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum PlaybackPreparationRefusal {
    Source { event: usize },
    Native(NativeBindingRefusal),
    Renderer(crate::pitch_trajectory::PitchRefusal),
}
pub struct PlaybackLinguisticBinding<'a> {
    pub event: usize,
    pub admitted: &'a PreparedLinguisticPitch<'a>,
}
pub struct PreparedSpeechPlaybackTape<'a> {
    realized: &'a PreparedIntentRealization<'a>,
    pitch: &'a PreparedUtterancePitch<'a>,
    bindings: Vec<(usize, &'a PreparedLinguisticPitch<'a>)>,
    basis: SpeechPlaybackBasis,
}
impl<'a> PreparedSpeechPlaybackTape<'a> {
    pub fn basis(&self) -> &SpeechPlaybackBasis {
        &self.basis
    }
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.realized.source()
    }
    pub fn bindings(&self) -> &[(usize, &'a PreparedLinguisticPitch<'a>)] {
        &self.bindings
    }
    pub fn renderer(&self) -> Result<Renderer<'_>, PlaybackPreparationRefusal> {
        self.pitch
            .renderer(self.realized)
            .map_err(PlaybackPreparationRefusal::Renderer)
    }
    /// Snapshot equality includes complete material, admitted discourse/lexical
    /// alternatives, speech profile/trajectory and the exact immutable plan.
    pub fn same_snapshot(&self, other: &Self) -> bool {
        self.basis == other.basis
            && self.bindings.len() == other.bindings.len()
            && self.bindings.iter().zip(&other.bindings).all(
                |((event, lhs), (right_event, rhs))| {
                    event == right_event
                        && lhs.accepted() == rhs.accepted()
                        && match (lhs.requested(), rhs.requested()) {
                            (
                                LinguisticProsodyBasis::Rich(lhs),
                                LinguisticProsodyBasis::Rich(rhs),
                            ) => lhs.accepted() == rhs.accepted(),
                            (
                                LinguisticProsodyBasis::Fallback(lhs),
                                LinguisticProsodyBasis::Fallback(rhs),
                            ) => lhs.accepted() == rhs.accepted(),
                            _ => false,
                        }
                },
            )
    }
}
pub fn prepare_speech_playback_tape<'a>(
    realized: &'a PreparedIntentRealization<'a>,
    pitch: &'a PreparedUtterancePitch<'a>,
    offered: &[PlaybackLinguisticBinding<'a>],
    clock_id: u64,
) -> Result<PreparedSpeechPlaybackTape<'a>, PlaybackPreparationRefusal> {
    if realized.source() != pitch.source() || offered.len() > crate::MAXIMUM_EVENTS {
        return Err(PlaybackPreparationRefusal::Source { event: 0 });
    }
    let mut occurrences = Vec::with_capacity(offered.len());
    let mut bindings = Vec::with_capacity(offered.len());
    for (event, value) in realized.source().events().iter().enumerate() {
        if !matches!(value, SpeechUtteranceIntentEvent::Segment(_)) {
            continue;
        }
        let mut selected = offered.iter().filter(|binding| binding.event == event);
        let binding = selected
            .next()
            .ok_or(PlaybackPreparationRefusal::Source { event })?;
        if selected.next().is_some() {
            return Err(PlaybackPreparationRefusal::Source { event });
        }
        let accepted = binding.admitted.accepted();
        if !pitch
            .admissions()
            .iter()
            .any(|(index, admission)| *index == event && *admission == accepted.pitch())
        {
            return Err(PlaybackPreparationRefusal::Source { event });
        }
        let basis = binding.admitted.requested();
        let (analysis, mode, linguistic_evidence, linguistic_evidence_profile) = match basis {
            LinguisticProsodyBasis::Rich(value) => (
                SpeechPlaybackAnalysisSpecification::Known(
                    value.requested().discourse().analysis().clone(),
                ),
                SpeechPlaybackProsodyMode::Rich,
                value
                    .accepted()
                    .clone()
                    .encode()
                    .map_err(PlaybackPreparationRefusal::Native)?,
                value
                    .accepted()
                    .clone()
                    .into_structured()
                    .map_err(PlaybackPreparationRefusal::Native)?
                    .value_type()
                    .profile()
                    .map_err(|_| PlaybackPreparationRefusal::Source { event })?
                    .value_kind()
                    .as_str()
                    .into(),
            ),
            LinguisticProsodyBasis::Fallback(value) => (
                SpeechPlaybackAnalysisSpecification::unspecified(),
                SpeechPlaybackProsodyMode::Fallback,
                value
                    .accepted()
                    .clone()
                    .encode()
                    .map_err(PlaybackPreparationRefusal::Native)?,
                value
                    .accepted()
                    .clone()
                    .into_structured()
                    .map_err(PlaybackPreparationRefusal::Native)?
                    .value_type()
                    .profile()
                    .map_err(|_| PlaybackPreparationRefusal::Source { event })?
                    .value_kind()
                    .as_str()
                    .into(),
            ),
        };
        if linguistic_evidence.len() > 32768 {
            return Err(PlaybackPreparationRefusal::Source { event });
        }
        let linguistic_evidence = linguistic_evidence.iter().fold(
            alloc::string::String::with_capacity(linguistic_evidence.len() * 2),
            |mut text, byte| {
                use core::fmt::Write;
                write!(&mut text, "{byte:02x}").expect("String formatting");
                text
            },
        );
        let source = conduit_language::language_source_occurrence(
            basis.source().material(),
            basis.token().span(),
            LanguageTextSegmentKind::Word,
        )
        .map_err(|_| PlaybackPreparationRefusal::Source { event })?;
        occurrences.push(
            SpeechPlaybackOccurrenceBasis::new(
                analysis,
                event as u64,
                linguistic_evidence,
                "canonical-hex@1".into(),
                linguistic_evidence_profile,
                basis.source().clone(),
                mode,
                accepted.clone(),
                source,
                basis.token().identity().clone(),
            )
            .map_err(PlaybackPreparationRefusal::Native)?,
        );
        bindings.push((event, binding.admitted));
    }
    if bindings.len() != offered.len() || bindings.len() != pitch.admissions().len() {
        return Err(PlaybackPreparationRefusal::Source { event: 0 });
    }
    let basis = SpeechPlaybackBasis::new(
        clock_id,
        realized.source().clone(),
        BoundedSequence::try_from_iter(occurrences)
            .map_err(|_| PlaybackPreparationRefusal::Source { event: 0 })?,
        realized.profile().clone(),
    )
    .map_err(PlaybackPreparationRefusal::Native)?;
    let prepared = PreparedSpeechPlaybackTape {
        realized,
        pitch,
        bindings,
        basis,
    };
    prepared.renderer()?;
    Ok(prepared)
}
