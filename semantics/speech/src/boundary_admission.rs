//! Explicit native boundary realization; no punctuation or defaulting policy.
use crate::{semantic, VoiceBoundary, VoiceEvent};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum BoundaryRefusal {
    Kind(semantic::SpeechBoundarySpecification),
    Duration(semantic::SpeechDurationSpecification),
    Native(NativeBindingRefusal),
}
pub struct PreparedBoundary<'a> {
    intent: &'a semantic::SpeechPlannedBoundaryIntent,
    binding: &'a semantic::SpeechFormantBoundaryBinding,
    checked: semantic::SpeechBoundaryIntentMatch,
    duration: semantic::SpeechExactDuration,
    event: VoiceEvent,
}
impl<'a> PreparedBoundary<'a> {
    pub fn intent(&self) -> &'a semantic::SpeechPlannedBoundaryIntent {
        self.intent
    }
    pub fn binding(&self) -> &'a semantic::SpeechFormantBoundaryBinding {
        self.binding
    }
    pub fn checked(&self) -> &semantic::SpeechBoundaryIntentMatch {
        &self.checked
    }
    pub fn duration(&self) -> &semantic::SpeechExactDuration {
        &self.duration
    }
    pub fn event(&self) -> VoiceEvent {
        self.event
    }
}
pub fn prepare_boundary<'a>(
    intent: &'a semantic::SpeechPlannedBoundaryIntent,
    binding: &'a semantic::SpeechFormantBoundaryBinding,
) -> Result<PreparedBoundary<'a>, BoundaryRefusal> {
    let semantic::SpeechBoundarySpecification::Known(kind) = intent.kind() else {
        return Err(BoundaryRefusal::Kind(intent.kind().clone()));
    };
    let checked = semantic::SpeechBoundaryIntentMatch::new(*kind, *binding.kind())
        .map_err(BoundaryRefusal::Native)?;
    let semantic::SpeechDurationSpecification::Known(value) = intent.duration() else {
        return Err(BoundaryRefusal::Duration(intent.duration().clone()));
    };
    let duration =
        semantic::SpeechExactDuration::new(*value.denominator(), *value.numerator_seconds())
            .map_err(BoundaryRefusal::Native)?;
    let boundary = match binding.realization() {
        semantic::SpeechFormantBoundary::Word => VoiceBoundary::word,
        semantic::SpeechFormantBoundary::Phrase => VoiceBoundary::phrase,
        semantic::SpeechFormantBoundary::Turn => VoiceBoundary::turn,
    };
    Ok(PreparedBoundary {
        intent,
        binding,
        checked,
        duration,
        event: VoiceEvent::boundary(boundary),
    })
}
