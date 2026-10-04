//! Finite preparation traversal. Plots own exact summation and grid projection.
//! These receipts are timing preparation, not execution Plans or commitments.
use crate::{generated, semantic, timing, MAXIMUM_EVENTS};
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationRefusal {
    EventBound,
    Rate,
    Arithmetic { event: usize },
    Domain { event: usize },
}

/// Projects cumulative ends using floor, retaining each fractional remainder.
/// The last endpoint owns total length; individual fractions cannot cause drift.
/// Arithmetic uses finite checked U64 common multiples and refuses overflow.
pub fn duration_spans(
    durations: &[semantic::SpeechExactDuration],
    sample_rate_hz: u64,
) -> Result<Vec<semantic::SpeechEventFrameSpan>, DurationRefusal> {
    if durations.len() > MAXIMUM_EVENTS {
        return Err(DurationRefusal::EventBound);
    }
    let mut cumulative = semantic::SpeechExactDuration::new(1, 0)
        .map_err(|_| DurationRefusal::Domain { event: 0 })?;
    semantic::SpeechDurationAtRateRequest::new(cumulative.clone(), sample_rate_hz)
        .map_err(|_| DurationRefusal::Rate)?;
    let mut start_frame = 0;
    let mut spans = Vec::with_capacity(durations.len());
    for (event, duration) in durations.iter().enumerate() {
        let sum = generated::speech_duration_sum(generated::SpeechDurationSumInput {
            left_numerator: *cumulative.numerator_seconds(),
            left_denominator: *cumulative.denominator(),
            right_numerator: *duration.numerator_seconds(),
            right_denominator: *duration.denominator(),
        })
        .ok_or(DurationRefusal::Arithmetic { event })?;
        cumulative = semantic::SpeechExactDuration::new(sum.denominator, sum.numerator_seconds)
            .map_err(|_| DurationRefusal::Domain { event })?;
        let request =
            semantic::SpeechDurationAtRateRequest::new(cumulative.clone(), sample_rate_hz)
                .map_err(|_| DurationRefusal::Domain { event })?;
        let end = timing::duration_at_rate(&request).map_err(|error| match error {
            timing::TimingRefusal::Arithmetic => DurationRefusal::Arithmetic { event },
            timing::TimingRefusal::OutputDomain => DurationRefusal::Domain { event },
        })?;
        let frame_count = generated::speech_frame_span(generated::SpeechFrameSpanInput {
            start_frame,
            end_frame: *end.whole_frames(),
        })
        .ok_or(DurationRefusal::Arithmetic { event })?;
        let end_frame = *end.whole_frames();
        spans.push(
            semantic::SpeechEventFrameSpan::new(end, duration.clone(), frame_count, start_frame)
                .map_err(|_| DurationRefusal::Domain { event })?,
        );
        start_frame = end_frame;
    }
    Ok(spans)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationRenderRefusal {
    Duration(DurationRefusal),
    TimingCount,
    OutputSpace,
    FrameCount { event: usize },
    Renderer(crate::RenderRefusal),
}

/// Retained timing receipts and the allocation-free prepared renderer.
/// Metadata allocates during preparation; advancing the renderer does not.
pub struct PreparedDurationRender<'a> {
    renderer: crate::Renderer<'a>,
    spans: Vec<semantic::SpeechEventFrameSpan>,
}
impl<'a> PreparedDurationRender<'a> {
    pub fn renderer(&self) -> crate::Renderer<'a> {
        self.renderer
    }
    pub fn spans(&self) -> &[semantic::SpeechEventFrameSpan] {
        &self.spans
    }
}

/// Admit explicit exact durations into the fixed formant profile. Caller-owned
/// grid storage is unchanged on refusal and immutably borrowed during rendering.
/// No unknown/unspecified/alternative duration is resolved by this adapter.
pub fn prepare_duration_render<'a>(
    events: &'a [crate::VoiceEvent],
    durations: &[semantic::SpeechExactDuration],
    frame_storage: &'a mut [i32],
) -> Result<PreparedDurationRender<'a>, DurationRenderRefusal> {
    if events.len() != durations.len() {
        return Err(DurationRenderRefusal::TimingCount);
    }
    if frame_storage.len() < events.len() {
        return Err(DurationRenderRefusal::OutputSpace);
    }
    let spans = duration_spans(durations, u64::from(crate::SAMPLE_RATE_HZ))
        .map_err(DurationRenderRefusal::Duration)?;
    let mut counts = [0; MAXIMUM_EVENTS];
    for (event, span) in spans.iter().enumerate() {
        counts[event] = i32::try_from(*span.frame_count())
            .map_err(|_| DurationRenderRefusal::FrameCount { event })?;
    }
    let renderer =
        crate::Renderer::prepare_timed_in(events, &counts[..events.len()], frame_storage)
            .map_err(DurationRenderRefusal::Renderer)?;
    Ok(PreparedDurationRender { renderer, spans })
}
