//! Ordered quantitative preparation. This is neither phone/source resolution
//! nor commitment, an execution Plan, or complete utterance admission.
use crate::{
    control, duration, generated, intent_prosody, semantic::*, Renderer, VoiceBoundary, VoiceEvent,
    SAMPLE_RATE_HZ,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum UtteranceTimingRefusal {
    Segment(intent_prosody::IntentProsodyRefusal),
    BoundaryKind {
        event: usize,
        specification: SpeechBoundarySpecification,
    },
    BoundaryDuration {
        event: usize,
        specification: SpeechDurationSpecification,
    },
    MissingBoundaryBinding {
        event: usize,
    },
    AmbiguousBoundaryBinding {
        event: usize,
    },
    Native {
        event: usize,
        reason: NativeBindingRefusal,
    },
    Projection(duration::DurationRefusal),
    FrameRepresentation {
        event: usize,
    },
    Carrier,
}
#[derive(Debug)]
pub enum UtteranceTimingRenderRefusal {
    EventCount,
    EventShape { event: usize },
    Renderer(crate::RenderRefusal),
}

pub enum EventTimingReceipt<'a> {
    Segment {
        intent: &'a SpeechUtteranceIntentEventSegment,
        control: control::PreparedVoiceControl,
    },
    Boundary {
        intent: &'a SpeechUtteranceIntentEventBoundary,
        binding: &'a SpeechFormantBoundaryBinding,
        checked: SpeechBoundaryIntentMatch,
        event: VoiceEvent,
    },
}
/// Every span is projected on one cumulative grid, including silent boundaries.
/// All growth occurs during bounded preparation; rendering borrows frozen tapes.
pub struct PreparedUtteranceTiming<'a> {
    source: &'a SpeechUtteranceIntent,
    receipts: Vec<EventTimingReceipt<'a>>,
    spans: Vec<SpeechEventFrameSpan>,
    frames: Vec<i32>,
    compact: Vec<crate::SpeechEventVoiceControl>,
}
impl<'a> PreparedUtteranceTiming<'a> {
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.source
    }
    pub fn receipts(&self) -> &[EventTimingReceipt<'a>] {
        &self.receipts
    }
    pub fn spans(&self) -> &[SpeechEventFrameSpan] {
        &self.spans
    }
    /// Caller supplies separately resolved phone events. Exact source phone and
    /// phonological consistency are not established by this timing receipt.
    pub fn renderer<'r>(
        &'r self,
        events: &'r [VoiceEvent],
    ) -> Result<Renderer<'r>, UtteranceTimingRenderRefusal> {
        if events.len() != self.receipts.len() {
            return Err(UtteranceTimingRenderRefusal::EventCount);
        }
        for (event, (receipt, supplied)) in self.receipts.iter().zip(events).enumerate() {
            let matches = match receipt {
                EventTimingReceipt::Segment { .. } => !matches!(supplied, VoiceEvent::boundary(_)),
                EventTimingReceipt::Boundary {
                    event: expected, ..
                } => supplied == expected,
            };
            if !matches {
                return Err(UtteranceTimingRenderRefusal::EventShape { event });
            }
        }
        Renderer::prepare_timed(events, &self.frames)
            .and_then(|r| r.with_controls(&self.compact))
            .map_err(UtteranceTimingRenderRefusal::Renderer)
    }
}

pub fn prepare_utterance_timing<'a>(
    source: &'a SpeechUtteranceIntent,
    boundaries: &'a SpeechFormantBoundaryProfile,
) -> Result<PreparedUtteranceTiming<'a>, UtteranceTimingRefusal> {
    let mut receipts = Vec::with_capacity(source.events().as_slice().len());
    let mut durations = Vec::with_capacity(receipts.capacity());
    let mut compact = Vec::with_capacity(receipts.capacity());
    for (event, value) in source.events().as_slice().iter().enumerate() {
        match value {
            SpeechUtteranceIntentEvent::Segment(intent) => {
                let (duration, control) =
                    intent_prosody::known_segment_prosody(intent.prosody(), event)
                        .map_err(UtteranceTimingRefusal::Segment)?;
                durations.push(duration);
                compact.push(control.compact());
                receipts.push(EventTimingReceipt::Segment { intent, control });
            }
            SpeechUtteranceIntentEvent::Boundary(intent) => {
                let SpeechBoundarySpecification::Known(kind) = intent.kind() else {
                    return Err(UtteranceTimingRefusal::BoundaryKind {
                        event,
                        specification: intent.kind().clone(),
                    });
                };
                let mut matches = boundaries
                    .get()
                    .as_slice()
                    .iter()
                    .filter(|binding| binding.kind() == kind);
                let binding = matches
                    .next()
                    .ok_or(UtteranceTimingRefusal::MissingBoundaryBinding { event })?;
                if matches.next().is_some() {
                    return Err(UtteranceTimingRefusal::AmbiguousBoundaryBinding { event });
                }
                let checked = SpeechBoundaryIntentMatch::new(*kind, *binding.kind())
                    .map_err(|reason| UtteranceTimingRefusal::Native { event, reason })?;
                let SpeechDurationSpecification::Known(value) = intent.duration() else {
                    return Err(UtteranceTimingRefusal::BoundaryDuration {
                        event,
                        specification: intent.duration().clone(),
                    });
                };
                durations.push(
                    SpeechExactDuration::new(*value.denominator(), *value.numerator_seconds())
                        .map_err(|reason| UtteranceTimingRefusal::Native { event, reason })?,
                );
                let boundary = match binding.realization() {
                    SpeechFormantBoundary::Word => VoiceBoundary::word,
                    SpeechFormantBoundary::Phrase => VoiceBoundary::phrase,
                    SpeechFormantBoundary::Turn => VoiceBoundary::turn,
                };
                // No source cycle/intensity is invented: this ignored carrier
                // comes from a checked Plot and has no quantitative receipt.
                compact.push(
                    generated::speech_boundary_control(generated::SpeechStart::begin)
                        .ok_or(UtteranceTimingRefusal::Carrier)?,
                );
                receipts.push(EventTimingReceipt::Boundary {
                    intent,
                    binding,
                    checked,
                    event: VoiceEvent::boundary(boundary),
                });
            }
        }
    }
    let spans = duration::duration_spans(&durations, u64::from(SAMPLE_RATE_HZ))
        .map_err(UtteranceTimingRefusal::Projection)?;
    let frames = spans
        .iter()
        .enumerate()
        .map(|(event, span)| {
            i32::try_from(*span.frame_count())
                .map_err(|_| UtteranceTimingRefusal::FrameRepresentation { event })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedUtteranceTiming {
        source,
        receipts,
        spans,
        frames,
        compact,
    })
}
