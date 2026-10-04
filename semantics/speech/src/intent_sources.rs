//! Exact source coverage against explicitly supplied immutable materials.
//! This establishes reference resolution, not causality, authority, phonological
//! consistency or commitment. Native laws own every material/reference match.
use crate::{
    reference_admission::{
        resolve_phone, resolve_phoneme, ReferenceRefusal, ResolvedPhone, ResolvedPhoneme,
    },
    semantic::*,
    text_admission::{resolve_text, ResolvedText, TextReferenceRefusal},
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;

/// One explicitly supplied material per source, in original event/source order.
/// There is no ambient store, candidate search or implicit revision replacement.
#[derive(Clone, Copy)]
pub enum IntentSourceMaterial<'a> {
    Text(&'a LanguageText),
    Phone(&'a SpeechPhoneSequence),
    Phoneme(&'a SpeechPhonemeSequence),
    Recognition(&'a AsrRecognitionEnvelope),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentSourceLocation {
    pub event: usize,
    pub source: usize,
}
#[derive(Debug)]
pub enum IntentSourceReason {
    MaterialKind,
    Recognition(NativeBindingRefusal),
    Text(TextReferenceRefusal),
    Speech(ReferenceRefusal),
}
#[derive(Debug)]
pub enum IntentSourcesRefusal {
    MaterialCount {
        expected: usize,
        supplied: usize,
    },
    Source {
        location: IntentSourceLocation,
        reason: IntentSourceReason,
    },
}

pub enum ResolvedIntentSource<'a> {
    Text(ResolvedText<'a>),
    Phone(ResolvedPhone<'a>),
    Phoneme(ResolvedPhoneme<'a>),
    Recognition {
        reference: &'a LanguageSegmentRef,
        envelope: &'a AsrRecognitionEnvelope,
        checked: ListeningEventReferenceMatch,
    },
}
impl<'a> ResolvedIntentSource<'a> {
    pub fn reference(&self) -> &'a LanguageSegmentRef {
        match self {
            Self::Text(value) => value.reference(),
            Self::Phone(value) => value.reference(),
            Self::Phoneme(value) => value.reference(),
            Self::Recognition { reference, .. } => reference,
        }
    }
}
pub struct IntentSourceReceipt<'a> {
    location: IntentSourceLocation,
    resolved: ResolvedIntentSource<'a>,
}
impl<'a> IntentSourceReceipt<'a> {
    pub fn location(&self) -> IntentSourceLocation {
        self.location
    }
    pub fn resolved(&self) -> &ResolvedIntentSource<'a> {
        &self.resolved
    }
}
/// Complete ordered source coverage of this original intent. Growth is confined
/// to preparation and bounded by 256 events times at most eight sources.
pub struct PreparedIntentSources<'a> {
    intent: &'a SpeechUtteranceIntent,
    receipts: Vec<IntentSourceReceipt<'a>>,
}
impl<'a> PreparedIntentSources<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn receipts(&self) -> &[IntentSourceReceipt<'a>] {
        &self.receipts
    }
}
fn sources(event: &SpeechUtteranceIntentEvent) -> &[LanguageSegmentRef] {
    match event {
        SpeechUtteranceIntentEvent::Segment(value) => value.sources().as_slice(),
        SpeechUtteranceIntentEvent::Boundary(value) => value.sources().as_slice(),
    }
}

pub fn resolve_intent_sources<'a>(
    intent: &'a SpeechUtteranceIntent,
    materials: &[IntentSourceMaterial<'a>],
) -> Result<PreparedIntentSources<'a>, IntentSourcesRefusal> {
    let expected = intent
        .events()
        .as_slice()
        .iter()
        .map(|event| sources(event).len())
        .sum();
    if materials.len() != expected {
        return Err(IntentSourcesRefusal::MaterialCount {
            expected,
            supplied: materials.len(),
        });
    }
    let mut receipts = Vec::with_capacity(expected);
    for (event, value) in intent.events().as_slice().iter().enumerate() {
        for (source, reference) in sources(value).iter().enumerate() {
            let location = IntentSourceLocation { event, source };
            let material = materials[receipts.len()];
            let resolved = match (reference, material) {
                (LanguageSegmentRef::Text(_), IntentSourceMaterial::Text(value)) => {
                    resolve_text(reference, value)
                        .map(ResolvedIntentSource::Text)
                        .map_err(IntentSourceReason::Text)
                }
                (LanguageSegmentRef::Phone(_), IntentSourceMaterial::Phone(value)) => {
                    resolve_phone(reference, value)
                        .map(ResolvedIntentSource::Phone)
                        .map_err(IntentSourceReason::Speech)
                }
                (LanguageSegmentRef::Phoneme(_), IntentSourceMaterial::Phoneme(value)) => {
                    resolve_phoneme(reference, value)
                        .map(ResolvedIntentSource::Phoneme)
                        .map_err(IntentSourceReason::Speech)
                }
                (
                    LanguageSegmentRef::Recognition(value),
                    IntentSourceMaterial::Recognition(envelope),
                ) => ListeningEventRef::new(value.event_id().clone(), value.stream_id().clone())
                    .and_then(|reference| {
                        ListeningEventReferenceMatch::new(
                            envelope.event_id().clone(),
                            reference,
                            envelope.stream_id().clone(),
                        )
                    })
                    .map(|checked| ResolvedIntentSource::Recognition {
                        reference,
                        envelope,
                        checked,
                    })
                    .map_err(IntentSourceReason::Recognition),
                _ => Err(IntentSourceReason::MaterialKind),
            }
            .map_err(|reason| IntentSourcesRefusal::Source { location, reason })?;
            receipts.push(IntentSourceReceipt { location, resolved });
        }
    }
    Ok(PreparedIntentSources { intent, receipts })
}
