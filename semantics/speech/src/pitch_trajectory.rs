//! Exact endpoint-cycle intent, finite preparation, and shared cadence projection.
//! This proves controls at a cadence; it is not neural waveform evidence.
use crate::{generated, semantic::*, RenderRefusal, Renderer, SAMPLE_RATE_HZ};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum PitchRefusal {
    Source { event: usize },
    Duplicate { event: usize },
    Native(NativeBindingRefusal),
    Arithmetic,
    Control(crate::control::ControlRefusal),
    Renderer(RenderRefusal),
}
/// An offered trajectory names the exact global event; preparation verifies the
/// complete segment against the immutable utterance, including source refs.
pub struct OfferedSegmentPitch<'a> {
    pub event: usize,
    pub admission: &'a SpeechSegmentPitchAdmission,
}
pub struct PreparedUtterancePitch<'a> {
    source: &'a SpeechUtteranceIntent,
    admissions: Vec<(usize, &'a SpeechSegmentPitchAdmission)>,
    projections: Vec<Option<generated::SpeechPitchProjectionInput>>,
}
impl<'a> PreparedUtterancePitch<'a> {
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.source
    }
    pub fn admissions(&self) -> &[(usize, &'a SpeechSegmentPitchAdmission)] {
        &self.admissions
    }
    /// The same exact trajectory is sampled at caller-declared physical cadence.
    /// This is a cycle/Q8 receipt, not a formant or FARGAN waveform.
    pub fn at_frame(
        &self,
        event: usize,
        frame: u64,
        rate: u64,
    ) -> Result<SpeechCycleQ8AtRate, PitchRefusal> {
        let (_, admission) = self
            .admissions
            .iter()
            .find(|(index, _)| *index == event)
            .ok_or(PitchRefusal::Source { event })?;
        sample_at_frame(admission, frame, rate)
    }
    pub fn renderer<'r>(
        &'r self,
        realized: &'r crate::intent_realization::PreparedIntentRealization<'_>,
    ) -> Result<Renderer<'r>, PitchRefusal> {
        if realized.source() != self.source {
            return Err(PitchRefusal::Source { event: 0 });
        }
        realized
            .renderer()
            .map_err(|_| PitchRefusal::Source { event: 0 })?
            .with_pitch(&self.projections)
            .map_err(PitchRefusal::Renderer)
    }
}
fn input(
    value: &SpeechLinearPitchTrajectory,
    frame: u64,
    rate: u64,
) -> generated::SpeechPitchProjectionInput {
    generated::SpeechPitchProjectionInput {
        start_numerator: *value.start().numerator_seconds(),
        start_denominator: *value.start().denominator(),
        end_numerator: *value.end().numerator_seconds(),
        end_denominator: *value.end().denominator(),
        duration_numerator: *value.duration().numerator_seconds(),
        duration_denominator: *value.duration().denominator(),
        sample_rate_hz: rate,
        frame,
    }
}
pub fn sample_at_frame(
    admission: &SpeechSegmentPitchAdmission,
    frame: u64,
    rate: u64,
) -> Result<SpeechCycleQ8AtRate, PitchRefusal> {
    let request = SpeechPitchAtFrameRequest::new(admission.clone(), frame, rate)
        .map_err(PitchRefusal::Native)?;
    let fraction = generated::speech_pitch_cycle_fraction(input(
        request.admission().trajectory(),
        frame,
        rate,
    ))
    .ok_or(PitchRefusal::Arithmetic)?;
    let cycle = SpeechFundamentalCycle::new(fraction.denominator, fraction.numerator)
        .map_err(PitchRefusal::Native)?;
    let request = SpeechCycleAtRateRequest::new(cycle, rate).map_err(PitchRefusal::Native)?;
    crate::control::cycle_q8(&request).map_err(PitchRefusal::Control)
}
pub fn prepare_utterance_pitch<'a>(
    source: &'a SpeechUtteranceIntent,
    offered: &[OfferedSegmentPitch<'a>],
) -> Result<PreparedUtterancePitch<'a>, PitchRefusal> {
    let mut projections = alloc::vec![None; source.events().len()];
    let mut admissions = Vec::with_capacity(offered.len().min(source.events().len()));
    let mut admitted_frames = 0_u64;
    if offered.len() > source.events().len() {
        return Err(PitchRefusal::Source {
            event: source.events().len(),
        });
    }
    for offer in offered {
        let Some(SpeechUtteranceIntentEvent::Segment(segment)) =
            source.events().iter().nth(offer.event)
        else {
            return Err(PitchRefusal::Source { event: offer.event });
        };
        let exact_segment = SpeechPlannedSegmentIntent::new(
            segment.occurrence().clone(),
            segment.phone().clone(),
            segment.phoneme().clone(),
            segment.prosody().clone(),
            segment.provenance().clone(),
            segment.sources().clone(),
            segment.stress().clone(),
            segment.word_position().clone(),
        )
        .map_err(PitchRefusal::Native)?;
        if &exact_segment != offer.admission.segment() {
            return Err(PitchRefusal::Source { event: offer.event });
        }
        if projections[offer.event].is_some() {
            return Err(PitchRefusal::Duplicate { event: offer.event });
        }
        // Checked projection admission tests endpoint range before allocation-free play.
        let _initial = sample_at_frame(offer.admission, 0, u64::from(SAMPLE_RATE_HZ))?;
        let request = SpeechDurationAtRateRequest::new(
            offer.admission.trajectory().duration().clone(),
            u64::from(SAMPLE_RATE_HZ),
        )
        .map_err(PitchRefusal::Native)?;
        let duration =
            crate::timing::duration_at_rate(&request).map_err(|_| PitchRefusal::Arithmetic)?;
        let frames = *duration.whole_frames();
        admitted_frames = admitted_frames
            .checked_add(frames)
            .ok_or(PitchRefusal::Arithmetic)?;
        if admitted_frames > crate::MAXIMUM_UTTERANCE_FRAMES {
            return Err(PitchRefusal::Renderer(RenderRefusal::DurationBound));
        }
        let projection = input(offer.admission.trajectory(), 0, u64::from(SAMPLE_RATE_HZ));
        for frame in 0..=frames {
            let period = generated::speech_pitch_period_q8(generated::SpeechPitchProjectionInput {
                frame,
                ..projection
            })
            .ok_or(PitchRefusal::Arithmetic)?;
            let period_q8 = i32::try_from(period).map_err(|_| PitchRefusal::Arithmetic)?;
            if !generated::speech_voice_control_admitted(generated::SpeechEventVoiceControl {
                cycle_mode: generated::SpeechCycleControlMode::resolved,
                period_q8,
                amplitude_q15: 32768,
            })
            .ok_or(PitchRefusal::Arithmetic)?
            {
                return Err(PitchRefusal::Renderer(RenderRefusal::ControlDomain));
            }
        }
        projections[offer.event] = Some(projection);
        admissions.push((offer.event, offer.admission));
    }
    Ok(PreparedUtterancePitch {
        source,
        admissions,
        projections,
    })
}

/// Immutable preparation retains rich/fallback basis, offered profile, exact
/// source correlation and accepted endpoint law. It neither commits nor plays.
pub struct PreparedLinguisticPitch<'a> {
    basis: crate::linguistic_prosody::LinguisticProsodyBasis<'a>,
    accepted: SpeechLinguisticTrajectoryAdmission,
}
impl PreparedLinguisticPitch<'_> {
    pub fn requested(&self) -> &crate::linguistic_prosody::LinguisticProsodyBasis<'_> {
        &self.basis
    }
    pub fn accepted(&self) -> &SpeechLinguisticTrajectoryAdmission {
        &self.accepted
    }
}
/// Exact refused inputs remain available without relabeling them as accepted.
pub struct RefusedLinguisticPitch<'a> {
    basis: crate::linguistic_prosody::LinguisticProsodyBasis<'a>,
    binding: &'a SpeechLinguisticProsodyBinding,
    segment: &'a SpeechPlannedSegmentIntent,
    trajectory: &'a SpeechLinearPitchTrajectory,
    reason: PitchRefusal,
}
impl RefusedLinguisticPitch<'_> {
    pub fn requested(&self) -> &crate::linguistic_prosody::LinguisticProsodyBasis<'_> {
        &self.basis
    }
    pub fn binding(&self) -> &SpeechLinguisticProsodyBinding {
        self.binding
    }
    pub fn segment(&self) -> &SpeechPlannedSegmentIntent {
        self.segment
    }
    pub fn trajectory(&self) -> &SpeechLinearPitchTrajectory {
        self.trajectory
    }
    pub fn reason(&self) -> &PitchRefusal {
        &self.reason
    }
}
impl core::fmt::Debug for RefusedLinguisticPitch<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RefusedLinguisticPitch")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
pub fn prepare_linguistic_pitch<'a>(
    basis: crate::linguistic_prosody::LinguisticProsodyBasis<'a>,
    binding: &'a SpeechLinguisticProsodyBinding,
    segment: &'a SpeechPlannedSegmentIntent,
    trajectory: &'a SpeechLinearPitchTrajectory,
) -> Result<PreparedLinguisticPitch<'a>, RefusedLinguisticPitch<'a>> {
    let result = (|| -> Result<SpeechLinguisticTrajectoryAdmission, PitchRefusal> {
        let reference = conduit_language::language_source_occurrence(
            basis.source().material(),
            basis.token().span(),
            conduit_language::LanguageTextSegmentKind::Word,
        )
        .map_err(|_| PitchRefusal::Source { event: 0 })?;
        let expected = LanguageSegmentRef::text(
            *reference.kind(),
            reference.language().clone(),
            reference.range().clone(),
            reference.revision_id().clone(),
            reference.text_id().clone(),
        )
        .map_err(PitchRefusal::Native)?;
        if !segment.sources().iter().any(|value| value == &expected) {
            return Err(PitchRefusal::Source { event: 0 });
        }
        let pitch = SpeechSegmentPitchAdmission::new(
            *basis.choice().pitch(),
            segment.clone(),
            trajectory.clone(),
        )
        .map_err(PitchRefusal::Native)?;
        let accepted = SpeechLinguisticTrajectoryAdmission::new(
            binding.clone(),
            basis.source().material().language().clone(),
            pitch,
            basis.profile().identity().clone(),
            basis.choice().clone(),
        )
        .map_err(PitchRefusal::Native)?;
        Ok(accepted)
    })();
    match result {
        Ok(accepted) => Ok(PreparedLinguisticPitch { basis, accepted }),
        Err(reason) => Err(RefusedLinguisticPitch {
            basis,
            binding,
            segment,
            trajectory,
            reason,
        }),
    }
}
