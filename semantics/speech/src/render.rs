//! Caller-owned finite traversal for the compiled plot Back. All speech
//! equations and realization choices are generated from the checked source.
use crate::generated::*;

pub const SAMPLE_RATE_HZ: u32 = 8000;
pub const MAXIMUM_EVENTS: usize = 256;
pub const MAXIMUM_BLOCK_FRAMES: usize = 128;
pub const MAXIMUM_UTTERANCE_FRAMES: u64 = SAMPLE_RATE_HZ as u64 * 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderRefusal {
    EventBound,
    DurationBound,
    Arithmetic,
    OutputBound,
}

#[derive(Clone, Copy)]
struct Resonator {
    y1: i64,
    y2: i64,
}
impl Resonator {
    const ZERO: Self = Self { y1: 0, y2: 0 };
    fn advance(&mut self, sample: i64, gain: i64, b: i64, c: i64) -> Result<i64, RenderRefusal> {
        let drive = speech_drive(DriveInput { sample, gain }).ok_or(RenderRefusal::Arithmetic)?;
        let value = speech_resonator(ResonatorInput {
            drive,
            b,
            c,
            y1: self.y1,
            y2: self.y2,
        })
        .and_then(speech_limit)
        .ok_or(RenderRefusal::Arithmetic)?;
        self.y2 = self.y1;
        self.y1 = value;
        Ok(value)
    }
}

/// Allocation-free bounded transducer. This owns no timer, device, scheduler,
/// transport, or retries; a kernel Back may advance it by one admitted block.
#[derive(Clone, Copy)]
pub struct Renderer<'a> {
    events: &'a [VoiceEvent],
    event_index: usize,
    event_frame: i64,
    phase: i64,
    noise: i64,
    filters: [Resonator; 3],
    rendered_frames: u64,
    total_frames: u64,
}
impl<'a> Renderer<'a> {
    pub fn prepare(events: &'a [VoiceEvent]) -> Result<Self, RenderRefusal> {
        if events.len() > MAXIMUM_EVENTS {
            return Err(RenderRefusal::EventBound);
        }
        let mut total_frames = 0_u64;
        for event in events {
            let frames = match event {
                VoiceEvent::segment(value) => {
                    speech_realize(*value)
                        .and_then(|realization| speech_voice_target(realization.phone))
                        .ok_or(RenderRefusal::Arithmetic)?
                        .frames
                }
                VoiceEvent::boundary(boundary) => {
                    speech_pause(*boundary).ok_or(RenderRefusal::Arithmetic)?
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
            events,
            event_index: 0,
            event_frame: 0,
            phase: 0,
            noise: 1,
            filters: [Resonator::ZERO; 3],
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
    pub fn render(&mut self, output: &mut [i16]) -> Result<usize, RenderRefusal> {
        if output.len() > MAXIMUM_BLOCK_FRAMES {
            return Err(RenderRefusal::OutputBound);
        }
        let mut written = 0;
        while written < output.len() && self.event_index < self.events.len() {
            let (sample, frames) = match self.events[self.event_index] {
                VoiceEvent::boundary(boundary) => {
                    (0, speech_pause(boundary).ok_or(RenderRefusal::Arithmetic)?)
                }
                VoiceEvent::segment(value) => {
                    let target = speech_realize(value)
                        .and_then(|realization| speech_voice_target(realization.phone))
                        .ok_or(RenderRefusal::Arithmetic)?;
                    let period =
                        speech_pitch_period(value.stress).ok_or(RenderRefusal::Arithmetic)?;
                    self.noise = speech_noise(self.noise).ok_or(RenderRefusal::Arithmetic)?;
                    let excitation = speech_excitation(ExcitationInput {
                        phase: self.phase,
                        period,
                        noise: self.noise,
                        voiced: target.voiced,
                        frication: target.frication,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    self.phase = speech_phase(PhaseInput {
                        phase: self.phase,
                        period,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    let first =
                        self.filters[0].advance(excitation, target.gain1, target.b1, target.c1)?;
                    let second =
                        self.filters[1].advance(excitation, target.gain2, target.b2, target.c2)?;
                    let third =
                        self.filters[2].advance(excitation, target.gain3, target.b3, target.c3)?;
                    let envelope = speech_envelope(EnvelopeInput {
                        frame: self.event_frame,
                        total: target.frames,
                        closure: target.closure,
                    })
                    .ok_or(RenderRefusal::Arithmetic)?;
                    let sample = speech_mix(MixInput {
                        first,
                        second,
                        third,
                        envelope,
                    })
                    .and_then(speech_limit)
                    .ok_or(RenderRefusal::Arithmetic)?;
                    (
                        i16::try_from(sample).map_err(|_| RenderRefusal::Arithmetic)?,
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
