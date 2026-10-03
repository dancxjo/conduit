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
                    let value = event.realization().ok_or(RenderRefusal::Arithmetic)?;
                    speech_realize(value)
                        .and_then(|realization| speech_voice_target(realization.phone))
                        .ok_or(RenderRefusal::Arithmetic)?
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
                    let value = event.realization().ok_or(RenderRefusal::Arithmetic)?;
                    let phone = speech_realize(value)
                        .ok_or(RenderRefusal::Arithmetic)?
                        .phone;
                    let target = speech_voice_target(phone).ok_or(RenderRefusal::Arithmetic)?;
                    let target = speech_phone_frame_target(SpeechTrajectoryInput {
                        phone,
                        frame: self.event_frame,
                        target,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    let (previous, previous_place) =
                        neighbor(events, self.event_index.checked_sub(1), target, true)?;
                    let (next, _) =
                        neighbor(events, self.event_index.checked_add(1), target, false)?;
                    let connected = speech_connected_target(SpeechConnectedInput {
                        target,
                        frame: self.event_frame,
                        previous,
                        next,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    let target = speech_vowel_onset(SpeechVowelOnsetInput {
                        phone,
                        target: connected.target,
                        frame: self.event_frame,
                        previous_place,
                        relation: previous.relation,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    let period =
                        speech_pitch_period(value.stress).ok_or(RenderRefusal::Arithmetic)?;
                    let period = speech_pitch_contour(SpeechPitchInput {
                        stress: value.stress,
                        base_period: period,
                        frame: self.event_frame,
                        total: target.frames,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    let frame = speech_frame(SpeechFrameInput {
                        attack: connected.attack,
                        release: connected.release,
                        target,
                        period,
                        frame: self.event_frame,
                        state: self.state,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    self.state = frame.state;
                    (
                        i16::try_from(frame.sample).map_err(|_| RenderRefusal::Arithmetic)?,
                        target.frames,
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
    /// Representation projection; pronunciation and realization policy stay in plots.
    pub fn realization(self) -> Option<RealizationInput> {
        match self {
            Self::segment(value) => Some(value),
            Self::pronounced(value) => Some(value.realization),
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
    let value = event.realization().ok_or(RenderRefusal::Arithmetic)?;
    let phone = speech_realize(value)
        .ok_or(RenderRefusal::Arithmetic)?
        .phone;
    let target = speech_voice_target(phone).ok_or(RenderRefusal::Arithmetic)?;
    let model = speech_phone_frame_target(SpeechTrajectoryInput {
        phone,
        target,
        frame: if end { target.frames - 1 } else { 0 },
    })
    .ok_or(RenderRefusal::Arithmetic)?;
    Ok((
        SpeechNeighborModel {
            relation: SpeechNeighborRelation::segment,
            model,
        },
        speech_stop_place(phone).ok_or(RenderRefusal::Arithmetic)?,
    ))
}
