//! Speech custody composed with Audio's executed cumulative sample-grid laws.
//! Preparation does not grant commitment or playback authority.
use crate::{intent_admission, intent_sources, semantic::*};
use alloc::vec::Vec;
use conduit_audio::{
    AudioCumulativeFrameBasis, AudioCumulativeFrameCursor, AudioCumulativeFrameRequest,
    AudioCumulativeProjectionReceipt, AudioRateProjectionRefusal, AudioSampleProjectionQuantity,
    AudioSampleProjectionRequest, AudioSampleRateBasis, AudioTrajectoryAnchor,
    PreparedAudioSampleProjection,
};
use conduit_language::LanguageText;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum IntentDurationGridRefusal {
    MissingRate,
    ForeignAnchor,
    Occurrence(intent_admission::OccurrenceMembershipRefusal),
    Sources(intent_sources::IntentSourcesRefusal),
    Unresolved {
        event: usize,
        specification: SpeechDurationSpecification,
    },
    Native(NativeBindingRefusal),
    Projection(AudioRateProjectionRefusal),
}
impl From<NativeBindingRefusal> for IntentDurationGridRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Native(value)
    }
}
impl From<AudioRateProjectionRefusal> for IntentDurationGridRefusal {
    fn from(value: AudioRateProjectionRefusal) -> Self {
        Self::Projection(value)
    }
}

/// Original immutable carrier and text remain borrowed. Each step retains its
/// generic Audio request, executed conversion laws, exact endpoint and loss.
pub struct PreparedIntentDurationGrid<'a> {
    intent: &'a SpeechUtteranceIntent,
    text: &'a LanguageText,
    basis: AudioSampleRateBasis,
    steps: Vec<AudioCumulativeProjectionReceipt>,
}
impl<'a> PreparedIntentDurationGrid<'a> {
    pub fn intent(&self) -> &'a SpeechUtteranceIntent {
        self.intent
    }
    pub fn text(&self) -> &'a LanguageText {
        self.text
    }
    pub fn basis(&self) -> &AudioSampleRateBasis {
        &self.basis
    }
    pub fn steps(&self) -> &[AudioCumulativeProjectionReceipt] {
        &self.steps
    }
    /// Move exact owned Audio receipts into a frozen terminal preparation.
    pub fn into_steps(self) -> Vec<AudioCumulativeProjectionReceipt> {
        self.steps
    }
}

/// Explicit elapsed-second origin, cumulative floor endpoints, no independent
/// event rounding. The supported Audio append contract requires one denominator
/// across this finite tape (unreduced source fractions are retained verbatim).
/// Other denominators, numeric overflow and Audio's finite quantity bounds refuse.
/// Known zero-duration boundaries are permitted; unresolved states never default.
/// This subset resolves every source against one complete LanguageText; other
/// source-material kinds or revisions refuse rather than using an ambient store.
pub fn prepare_intent_duration_grid<'a>(
    intent: &'a SpeechUtteranceIntent,
    text: &'a LanguageText,
    anchor: &AudioTrajectoryAnchor,
    basis: Option<&AudioSampleRateBasis>,
) -> Result<PreparedIntentDurationGrid<'a>, IntentDurationGridRefusal> {
    let basis = basis.ok_or(IntentDurationGridRefusal::MissingRate)?;
    if basis.anchor() != anchor {
        return Err(IntentDurationGridRefusal::ForeignAnchor);
    }
    intent_admission::validate_intent_occurrences(intent)
        .map_err(IntentDurationGridRefusal::Occurrence)?;
    let source_count = intent
        .events()
        .as_slice()
        .iter()
        .map(|event| match event {
            SpeechUtteranceIntentEvent::Segment(v) => v.sources().len(),
            SpeechUtteranceIntentEvent::Boundary(v) => v.sources().len(),
        })
        .sum();
    let sources = alloc::vec![intent_sources::IntentSourceMaterial::Text(text); source_count];
    intent_sources::resolve_intent_sources(intent, &sources)
        .map_err(IntentDurationGridRefusal::Sources)?;
    let projector = PreparedAudioSampleProjection::new()?;
    let mut steps = Vec::with_capacity(intent.events().len());
    let mut cursor = None;
    for (event, value) in intent.events().as_slice().iter().enumerate() {
        let specification = match value {
            SpeechUtteranceIntentEvent::Segment(v) => v.prosody().duration(),
            SpeechUtteranceIntentEvent::Boundary(v) => v.duration(),
        };
        let SpeechDurationSpecification::Known(duration) = specification else {
            return Err(IntentDurationGridRefusal::Unresolved {
                event,
                specification: specification.clone(),
            });
        };
        let quantity = AudioSampleProjectionQuantity::duration(
            *duration.denominator(),
            *duration.numerator_seconds(),
        )?;
        let current = match cursor.take() {
            Some(cursor) => cursor,
            None => AudioCumulativeFrameCursor::new(
                AudioCumulativeFrameBasis::new(basis.clone(), *duration.denominator())?,
                0,
                0,
            )?,
        };
        let append = AudioSampleProjectionRequest::new(basis.clone(), quantity)?;
        let request = AudioCumulativeFrameRequest::new(append, current)?;
        let receipt = projector.append(&request.encode()?)?;
        cursor = Some(receipt.result().cursor().clone());
        steps.push(receipt);
    }
    Ok(PreparedIntentDurationGrid {
        intent,
        text,
        basis: basis.clone(),
        steps,
    })
}
