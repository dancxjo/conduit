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
    ControlCount,
    ControlDomain,
    ControlAfterStart,
    TimingCount,
    TimingDomain,
}

/// Allocation-free bounded transducer. This owns no timer, device, scheduler,
/// transport, or retries; a kernel Back may advance it by one admitted block.
#[derive(Clone, Copy)]
pub struct Renderer<'a> {
    events: &'a [VoiceEvent],
    frame_counts: Option<&'a [i32]>,
    controls: Option<&'a [SpeechEventVoiceControl]>,
    pitch: Option<&'a [Option<SpeechPitchProjectionInput>]>,
    cursor: RenderCursor,
}

/// Private traversal state, always paired with one immutable prepared event tape.
#[derive(Clone, Copy)]
pub(crate) struct RenderCursor {
    event_index: usize,
    event_frame: i32,
    state: SpeechFrameState,
    phase_q8: i32,
    rendered_frames: u64,
    total_frames: u64,
}
impl RenderCursor {
    pub(crate) fn prepare(events: &[VoiceEvent]) -> Result<Self, RenderRefusal> {
        Self::prepare_timed(events, None)
    }
    fn prepare_timed(
        events: &[VoiceEvent],
        frame_counts: Option<&[i32]>,
    ) -> Result<Self, RenderRefusal> {
        if frame_counts.is_some_and(|counts| counts.len() != events.len()) {
            return Err(RenderRefusal::TimingCount);
        }
        if events.len() > MAXIMUM_EVENTS {
            return Err(RenderRefusal::EventBound);
        }
        let mut total_frames = 0_u64;
        for (index, event) in events.iter().enumerate() {
            let frames = if let Some(counts) = frame_counts {
                let frames = counts[index];
                let admitted = speech_event_duration_admitted(SpeechEventDurationCheck {
                    boundary: matches!(event, VoiceEvent::boundary(_)),
                    frames,
                })
                .ok_or(RenderRefusal::Arithmetic)?;
                if !admitted {
                    return Err(RenderRefusal::TimingDomain);
                }
                if !matches!(event, VoiceEvent::boundary(_)) {
                    timed_model(*event, Some(frames))?;
                }
                frames
            } else {
                match event {
                    VoiceEvent::boundary(boundary) => {
                        speech_pause(*boundary).ok_or(RenderRefusal::Arithmetic)?
                    }
                    event => timed_model(*event, None)?.target.frames,
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
            phase_q8: 0,
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
        self.render_prepared(events, None, None, None, output)
    }
    fn render_prepared(
        &mut self,
        events: &[VoiceEvent],
        frame_counts: Option<&[i32]>,
        controls: Option<&[SpeechEventVoiceControl]>,
        pitch: Option<&[Option<SpeechPitchProjectionInput>]>,
        output: &mut [i16],
    ) -> Result<usize, RenderRefusal> {
        if output.len() > MAXIMUM_BLOCK_FRAMES {
            return Err(RenderRefusal::OutputBound);
        }
        let mut written = 0;
        while written < output.len() && self.event_index < events.len() {
            let explicit_frames = frame_counts.map(|counts| counts[self.event_index]);
            if explicit_frames == Some(0) {
                self.event_index += 1;
                continue;
            }
            let (sample, frames) = match events[self.event_index] {
                VoiceEvent::boundary(boundary) => {
                    let frame = speech_boundary_frame(SpeechBoundaryFrameInput {
                        state: self.state,
                        phase_q8: self.phase_q8,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    self.state = frame.state;
                    self.phase_q8 = frame.phase_q8;
                    (
                        i16::try_from(frame.sample).map_err(|_| RenderRefusal::Arithmetic)?,
                        match explicit_frames {
                            Some(frames) => frames,
                            None => speech_pause(boundary).ok_or(RenderRefusal::Arithmetic)?,
                        },
                    )
                }
                event => {
                    let model = timed_model(event, explicit_frames)?;
                    let phone = model.phone;
                    let target = model.target;
                    let (previous, previous_place) = neighbor(
                        events,
                        frame_counts,
                        self.event_index.checked_sub(1),
                        target,
                        true,
                    )?;
                    let (next, _) = neighbor(
                        events,
                        frame_counts,
                        self.event_index.checked_add(1),
                        target,
                        false,
                    )?;
                    let frames = target.frames;
                    let frame = speech_temporal_frame(SpeechTemporalRequest {
                        trajectory: SpeechTrajectoryInput {
                            phone,
                            target,
                            frame: self.event_frame,
                        },
                        context: SpeechTemporalContext {
                            cycle: match controls {
                                Some(controls) => SpeechFrameCycleControl {
                                    mode: controls[self.event_index].cycle_mode,
                                    phase_q8: self.phase_q8,
                                    period_q8: match pitch
                                        .and_then(|values| values[self.event_index])
                                    {
                                        Some(input) => i32::try_from(
                                            speech_pitch_period_q8(SpeechPitchProjectionInput {
                                                frame: u64::try_from(self.event_frame)
                                                    .map_err(|_| RenderRefusal::Arithmetic)?,
                                                ..input
                                            })
                                            .ok_or(RenderRefusal::Arithmetic)?,
                                        )
                                        .map_err(|_| RenderRefusal::Arithmetic)?,
                                        None => controls[self.event_index].period_q8,
                                    },
                                },
                                None => SpeechFrameCycleControl {
                                    mode: SpeechCycleControlMode::profile,
                                    phase_q8: self.phase_q8,
                                    period_q8: 0,
                                },
                            },
                            stress: model.stress,
                            state: self.state,
                            previous_place,
                            previous,
                            next,
                        },
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    self.state = frame.state;
                    self.phase_q8 = frame.phase_q8;
                    let sample = match controls {
                        Some(controls) => speech_amplitude_apply(SpeechAmplitudeApplyInput {
                            sample: frame.sample,
                            amplitude_q15: controls[self.event_index].amplitude_q15,
                        })
                        .ok_or(RenderRefusal::Arithmetic)?,
                        None => frame.sample,
                    };
                    (
                        i16::try_from(sample).map_err(|_| RenderRefusal::Arithmetic)?,
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
            frame_counts: None,
            controls: None,
            pitch: None,
            cursor: RenderCursor::prepare(events)?,
        })
    }
    /// Explicit per-event grid spans. Preparation validates every span before play.
    /// Source duration projection and its rounding receipt are separate preparation.
    pub fn prepare_timed(
        events: &'a [VoiceEvent],
        frame_counts: &'a [i32],
    ) -> Result<Self, RenderRefusal> {
        Ok(Self {
            events,
            frame_counts: Some(frame_counts),
            controls: None,
            pitch: None,
            cursor: RenderCursor::prepare_timed(events, Some(frame_counts))?,
        })
    }
    #[cfg(feature = "semantic-bindings")]
    pub(crate) fn prepare_timed_in(
        events: &'a [VoiceEvent],
        counts: &[i32],
        storage: &'a mut [i32],
    ) -> Result<Self, RenderRefusal> {
        if storage.len() < counts.len() {
            return Err(RenderRefusal::TimingCount);
        }
        let cursor = RenderCursor::prepare_timed(events, Some(counts))?;
        storage[..counts.len()].copy_from_slice(counts);
        Ok(Self {
            events,
            frame_counts: Some(&storage[..counts.len()]),
            controls: None,
            pitch: None,
            cursor,
        })
    }
    /// Add immutable controls before play, after validating every event. Existing exact
    /// duration spans and the selected-phone tape remain paired with this cursor.
    pub fn with_controls(
        mut self,
        controls: &'a [SpeechEventVoiceControl],
    ) -> Result<Self, RenderRefusal> {
        if self.rendered_frames() != 0 {
            return Err(RenderRefusal::ControlAfterStart);
        }
        if controls.len() != self.events.len() {
            return Err(RenderRefusal::ControlCount);
        }
        for control in controls {
            if !speech_voice_control_admitted(*control).ok_or(RenderRefusal::Arithmetic)? {
                return Err(RenderRefusal::ControlDomain);
            }
        }
        self.controls = Some(controls);
        Ok(self)
    }
    #[cfg(feature = "semantic-bindings")]
    pub(crate) fn with_pitch(
        mut self,
        pitch: &'a [Option<SpeechPitchProjectionInput>],
    ) -> Result<Self, RenderRefusal> {
        if self.rendered_frames() != 0 {
            return Err(RenderRefusal::ControlAfterStart);
        }
        if pitch.len() != self.events.len()
            || self.controls.is_none()
            || self.frame_counts.is_none()
        {
            return Err(RenderRefusal::ControlCount);
        }
        self.pitch = Some(pitch);
        Ok(self)
    }
    pub fn prepare_controlled(
        events: &'a [VoiceEvent],
        controls: &'a [SpeechEventVoiceControl],
    ) -> Result<Self, RenderRefusal> {
        Self::prepare(events)?.with_controls(controls)
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
        if self.frame_counts.is_none() && self.controls.is_none() {
            self.cursor.render(self.events, output)
        } else {
            self.cursor.render_prepared(
                self.events,
                self.frame_counts,
                self.controls,
                self.pitch,
                output,
            )
        }
    }
}

impl VoiceEvent {
    fn model(self) -> Option<SpeechRenderSegment> {
        let realization = match self {
            Self::phone(value) => return speech_direct_phone_model(value),
            Self::selected(value) => value,
            Self::segment(value) => speech_realize(value)?,
            Self::pronounced(value) => speech_realize(value.realization)?,
            Self::boundary(_) => return None,
        };
        speech_direct_phone_model(speech_selected_phone(realization)?)
    }
    /// Representation projection; pronunciation and realization policy stay in plots.
    pub fn realization(self) -> Option<RealizationInput> {
        match self {
            Self::segment(value) => Some(value),
            Self::pronounced(value) => Some(value.realization),
            Self::selected(value) => Some(value.input),
            Self::phone(_) | Self::boundary(_) => None,
        }
    }
}

// Exact adjacent-tape projection only. The plots decide transition eligibility,
// weights and envelopes; a pause is never skipped to find a different neighbor.
fn neighbor(
    events: &[VoiceEvent],
    frame_counts: Option<&[i32]>,
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
    let prepared = timed_model(*event, frame_counts.map(|counts| counts[index.unwrap()]))?;
    let endpoint = speech_neighbor_endpoint(SpeechNeighborEndpointInput {
        phone: prepared.phone,
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

// Representation dispatch only. Realization and duration policy remain plots.
fn timed_model(
    event: VoiceEvent,
    frames: Option<i32>,
) -> Result<SpeechRenderSegment, RenderRefusal> {
    let mut model = event.model().ok_or(RenderRefusal::Arithmetic)?;
    if let Some(frames) = frames {
        model.target = speech_duration_target(SpeechDurationTargetInput {
            target: model.target,
            frames,
        })
        .ok_or(RenderRefusal::Arithmetic)?;
    }
    Ok(model)
}
