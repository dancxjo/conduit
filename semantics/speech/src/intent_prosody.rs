//! Quantitative segment intent to retained timing and voice-control receipts.
//! No grammar, commitment, phone selection or unresolved-value policy lives here.
use crate::{control, duration, semantic, Renderer, VoiceEvent, MAXIMUM_EVENTS, SAMPLE_RATE_HZ};
use alloc::vec::Vec;

#[derive(Debug)]
pub enum IntentProsodyRefusal {
    EventBound,
    Native {
        event: usize,
        reason: conduit_plot::rust_binding::NativeBindingRefusal,
    },
    Duration {
        event: usize,
        specification: semantic::SpeechDurationSpecification,
    },
    Cycle {
        event: usize,
        specification: semantic::SpeechCycleSpecification,
    },
    Intensity {
        event: usize,
        specification: semantic::SpeechIntensitySpecification,
    },
    Projection(duration::DurationRefusal),
    Control {
        event: usize,
        reason: control::ControlRefusal,
    },
    FrameRepresentation {
        event: usize,
    },
}

#[derive(Debug)]
pub enum IntentProsodyRenderRefusal {
    Boundary { event: usize },
    Renderer(crate::RenderRefusal),
}

/// All owned storage is prepared before rendering. Source intent and exact
/// quantization receipts remain available; this value is not an execution Plan.
pub struct PreparedSegmentProsody<'a> {
    source: &'a [semantic::SpeechSegmentProsodyIntent],
    spans: Vec<semantic::SpeechEventFrameSpan>,
    controls: Vec<control::PreparedVoiceControl>,
    frames: Vec<i32>,
    compact: Vec<crate::SpeechEventVoiceControl>,
}
impl<'a> PreparedSegmentProsody<'a> {
    pub fn source(&self) -> &'a [semantic::SpeechSegmentProsodyIntent] {
        self.source
    }
    pub fn spans(&self) -> &[semantic::SpeechEventFrameSpan] {
        &self.spans
    }
    pub fn controls(&self) -> &[control::PreparedVoiceControl] {
        &self.controls
    }
    /// Caller explicitly pairs ordered segment prosody with ordered events.
    /// Renderer admission still checks counts, total length and control domains.
    /// This does not resolve occurrence/source references or admit boundaries.
    pub fn renderer<'r>(
        &'r self,
        events: &'r [VoiceEvent],
    ) -> Result<Renderer<'r>, IntentProsodyRenderRefusal> {
        for (event, value) in events.iter().enumerate() {
            if matches!(value, VoiceEvent::boundary(_)) {
                return Err(IntentProsodyRenderRefusal::Boundary { event });
            }
        }
        Renderer::prepare_timed(events, &self.frames)
            .and_then(|renderer| renderer.with_controls(&self.compact))
            .map_err(IntentProsodyRenderRefusal::Renderer)
    }
}

/// Only known quantitative values lower here. Unknown, unspecified,
/// not-applicable, variable and gradient values refuse with the exact original
/// specification. Defaulting requires a separately authored realization choice.
pub fn prepare_segment_prosody(
    source: &[semantic::SpeechSegmentProsodyIntent],
) -> Result<PreparedSegmentProsody<'_>, IntentProsodyRefusal> {
    if source.len() > MAXIMUM_EVENTS {
        return Err(IntentProsodyRefusal::EventBound);
    }
    let mut durations = Vec::with_capacity(source.len());
    let mut controls = Vec::with_capacity(source.len());
    for (event, intent) in source.iter().enumerate() {
        let semantic::SpeechDurationSpecification::Known(value) = intent.duration() else {
            return Err(IntentProsodyRefusal::Duration {
                event,
                specification: intent.duration().clone(),
            });
        };
        durations.push(
            semantic::SpeechExactDuration::new(*value.denominator(), *value.numerator_seconds())
                .map_err(|reason| IntentProsodyRefusal::Native { event, reason })?,
        );
        let semantic::SpeechCycleSpecification::Known(cycle) = intent.fundamental_cycle() else {
            return Err(IntentProsodyRefusal::Cycle {
                event,
                specification: intent.fundamental_cycle().clone(),
            });
        };
        let semantic::SpeechIntensitySpecification::Known(intensity) = intent.relative_intensity()
        else {
            return Err(IntentProsodyRefusal::Intensity {
                event,
                specification: intent.relative_intensity().clone(),
            });
        };
        let cycle =
            semantic::SpeechFundamentalCycle::new(*cycle.denominator(), *cycle.numerator_seconds())
                .map_err(|reason| IntentProsodyRefusal::Native { event, reason })?;
        let intensity = semantic::SpeechRelativeIntensity::new(
            *intensity.denominator(),
            *intensity.numerator(),
        )
        .map_err(|reason| IntentProsodyRefusal::Native { event, reason })?;
        controls.push(
            control::prepare_voice_control(Some(&cycle), &intensity)
                .map_err(|reason| IntentProsodyRefusal::Control { event, reason })?,
        );
    }
    let spans = duration::duration_spans(&durations, u64::from(SAMPLE_RATE_HZ))
        .map_err(IntentProsodyRefusal::Projection)?;
    let frames = spans
        .iter()
        .enumerate()
        .map(|(event, span)| {
            i32::try_from(*span.frame_count())
                .map_err(|_| IntentProsodyRefusal::FrameRepresentation { event })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let compact = controls
        .iter()
        .map(control::PreparedVoiceControl::compact)
        .collect();
    Ok(PreparedSegmentProsody {
        source,
        spans,
        controls,
        frames,
        compact,
    })
}
