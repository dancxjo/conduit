//! Caller-owned finite traversal for the compiled plot Back. All speech
//! equations and realization choices are generated from the checked source.
use crate::generated::*;

pub const SAMPLE_RATE_HZ: u32 = RENDER_PROFILE.sample_rate_hz;
pub const MAXIMUM_EVENTS: usize = RENDER_PROFILE.maximum_events as usize;
pub const MAXIMUM_BLOCK_FRAMES: usize = RENDER_PROFILE.maximum_block_frames as usize;
pub const MAXIMUM_UTTERANCE_FRAMES: u64 = RENDER_PROFILE.maximum_utterance_frames;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderRefusal {
    EventBound,
    DurationBound,
    Arithmetic,
    OutputBound,
}

/// Allocation-free bounded transducer. This owns no timer, device, scheduler,
/// transport, or retries; a kernel Back may advance it by one admitted block.
#[derive(Clone, Copy)]
pub struct Renderer<'a> {
    events: &'a [VoiceEvent],
    cursor: RenderCursor,
}

/// Private traversal state, always paired with one immutable prepared event tape.
#[derive(Clone, Copy)]
pub(crate) struct RenderCursor {
    event_index: usize,
    event_frame: i32,
    state: SpeechFrameState,
    rendered_frames: u64,
    total_frames: u64,
}
impl RenderCursor {
    pub(crate) fn prepare(events: &[VoiceEvent]) -> Result<Self, RenderRefusal> {
        if events.len() > MAXIMUM_EVENTS {
            return Err(RenderRefusal::EventBound);
        }
        let mut total_frames = 0_u64;
        for event in events {
            let frames = match event {
                VoiceEvent::boundary(boundary) => {
                    speech_pause(*boundary).ok_or(RenderRefusal::Arithmetic)?
                }
                event => {
                    event
                        .model()
                        .ok_or(RenderRefusal::Arithmetic)?
                        .target
                        .frames
                }
            };
            total_frames = total_frames
                .checked_add(u64::try_from(frames).map_err(|_| RenderRefusal::Arithmetic)?)
                .ok_or(RenderRefusal::DurationBound)?;
        }
        if total_frames > MAXIMUM_UTTERANCE_FRAMES {
            return Err(RenderRefusal::DurationBound);
        }
        Ok(Self {
            event_index: 0,
            event_frame: 0,
            state: speech_initial_state(SpeechStart::begin).ok_or(RenderRefusal::Arithmetic)?,
            rendered_frames: 0,
            total_frames,
        })
    }
    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }
    pub fn rendered_frames(&self) -> u64 {
        self.rendered_frames
    }
    pub fn is_complete(&self) -> bool {
        self.rendered_frames == self.total_frames
    }
    /// Produces at most one finite block. Commit this copy only after atomic
    /// output acceptance; pressure leaves the original state untouched.
    pub(crate) fn render(
        &mut self,
        events: &[VoiceEvent],
        output: &mut [i16],
    ) -> Result<usize, RenderRefusal> {
        if output.len() > MAXIMUM_BLOCK_FRAMES {
            return Err(RenderRefusal::OutputBound);
        }
        let mut written = 0;
        while written < output.len() && self.event_index < events.len() {
            let (sample, frames) = match events[self.event_index] {
                VoiceEvent::boundary(boundary) => {
                    let frame =
                        speech_boundary_frame(self.state).ok_or(RenderRefusal::Arithmetic)?;
                    self.state = frame.state;
                    (
                        i16::try_from(frame.sample).map_err(|_| RenderRefusal::Arithmetic)?,
                        speech_pause(boundary).ok_or(RenderRefusal::Arithmetic)?,
                    )
                }
                event => {
                    let model = event.model().ok_or(RenderRefusal::Arithmetic)?;
                    let phone = model.realization.phone;
                    let target = model.target;
                    let (previous, previous_place) =
                        neighbor(events, self.event_index.checked_sub(1), target, true)?;
                    let (next, _) =
                        neighbor(events, self.event_index.checked_add(1), target, false)?;
                    let frames = target.frames;
                    let frame = speech_temporal_frame(SpeechTemporalRequest {
                        trajectory: SpeechTrajectoryInput {
                            phone,
                            target,
                            frame: self.event_frame,
                        },
                        context: SpeechTemporalContext {
                            stress: model.realization.input.stress,
                            state: self.state,
                            previous_place,
                            previous,
                            next,
                        },
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    self.state = frame.state;
                    (
                        i16::try_from(frame.sample).map_err(|_| RenderRefusal::Arithmetic)?,
                        frames,
                    )
                }
            };
            output[written] = sample;
            written += 1;
            self.rendered_frames += 1;
            self.event_frame += 1;
            if self.event_frame == frames {
                self.event_frame = 0;
                self.event_index += 1;
            }
        }
        Ok(written)
    }
}

impl<'a> Renderer<'a> {
    pub fn prepare(events: &'a [VoiceEvent]) -> Result<Self, RenderRefusal> {
        Ok(Self {
            events,
            cursor: RenderCursor::prepare(events)?,
        })
    }
    pub fn total_frames(&self) -> u64 {
        self.cursor.total_frames()
    }
    pub fn rendered_frames(&self) -> u64 {
        self.cursor.rendered_frames()
    }
    pub fn is_complete(&self) -> bool {
        self.cursor.is_complete()
    }
    /// Advances only this caller-owned copy; commit it after output acceptance.
    pub fn render(&mut self, output: &mut [i16]) -> Result<usize, RenderRefusal> {
        self.cursor.render(self.events, output)
    }
}

impl VoiceEvent {
    fn model(self) -> Option<SpeechSegmentModel> {
        let realization = match self {
            Self::selected(value) => value,
            Self::segment(value) => speech_realize(value)?,
            Self::pronounced(value) => speech_realize(value.realization)?,
            Self::boundary(_) => return None,
        };
        speech_selected_segment_model(realization)
    }
    /// Representation projection; pronunciation and realization policy stay in plots.
    pub fn realization(self) -> Option<RealizationInput> {
        match self {
            Self::segment(value) => Some(value),
            Self::pronounced(value) => Some(value.realization),
            Self::selected(value) => Some(value.input),
            Self::boundary(_) => None,
        }
    }
}

// Exact adjacent-tape projection only. The plots decide transition eligibility,
// weights and envelopes; a pause is never skipped to find a different neighbor.
fn neighbor(
    events: &[VoiceEvent],
    index: Option<usize>,
    neutral: SpeechAcousticTarget,
    end: bool,
) -> Result<(SpeechNeighborModel, SpeechStopPlace), RenderRefusal> {
    let Some(event) = index.and_then(|index| events.get(index)) else {
        return Ok((
            SpeechNeighborModel {
                relation: SpeechNeighborRelation::sequence_edge,
                model: neutral,
            },
            SpeechStopPlace::not_stop,
        ));
    };
    if let VoiceEvent::boundary(boundary) = event {
        let relation = match boundary {
            VoiceBoundary::word => SpeechNeighborRelation::word_boundary,
            VoiceBoundary::phrase => SpeechNeighborRelation::phrase_boundary,
            VoiceBoundary::turn => SpeechNeighborRelation::turn_boundary,
        };
        return Ok((
            SpeechNeighborModel {
                relation,
                model: neutral,
            },
            SpeechStopPlace::not_stop,
        ));
    }
    let prepared = event.model().ok_or(RenderRefusal::Arithmetic)?;
    let endpoint = speech_neighbor_endpoint(SpeechNeighborEndpointInput {
        phone: prepared.realization.phone,
        target: prepared.target,
        side: if end {
            SpeechEndpointSide::end
        } else {
            SpeechEndpointSide::start
        },
    })
    .ok_or(RenderRefusal::Arithmetic)?;
    Ok((endpoint.neighbor, endpoint.place))
}
