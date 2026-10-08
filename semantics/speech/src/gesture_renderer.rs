//! Bounded explicit-window projection into the existing checked Source frame DSP.
//! No implicit clock mapping, allophone selection, or general modulation model.
use crate::{
    common_acoustic_quantities::{
        execute, SpeechCommonAcousticExecution, SpeechCommonAcousticRefusal,
    },
    generated as dsp,
    resonator_programs::{DSP_PROFILE, DSP_Q8_PROFILE, FRAME_GATES},
    semantic::*,
    *,
};
use alloc::vec::Vec;
#[allow(dead_code)]
mod native_dsp {
    include!(concat!(env!("OUT_DIR"), "/gesture_dsp_types.rs"));
}
pub mod speech_gesture_dsp_programs {
    include!(concat!(env!("OUT_DIR"), "/gesture_dsp_programs.rs"));
}
use conduit_audio::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum SpeechGestureRenderRefusal {
    Admission(NativeBindingRefusal),
    Control(crate::control::ControlRefusal),
    Audio(AudioRateProjectionRefusal),
    Trajectory(AudioTrajectoryRefusal),
    Gesture(SpeechGestureRefusal),
    Coefficient(SpeechResonatorProjectionRefusal),
    Source(SpeechCommonAcousticRefusal),
    ForeignBasis,
    UnsupportedProfile,
    ResourceBound,
    DspArithmetic,
}
macro_rules! from {
    ($t:ty,$v:ident) => {
        impl From<$t> for SpeechGestureRenderRefusal {
            fn from(e: $t) -> Self {
                Self::$v(e)
            }
        }
    };
}
from!(NativeBindingRefusal, Admission);
from!(crate::control::ControlRefusal, Control);
from!(AudioRateProjectionRefusal, Audio);
from!(AudioTrajectoryRefusal, Trajectory);
from!(SpeechGestureRefusal, Gesture);
from!(SpeechResonatorProjectionRefusal, Coefficient);
from!(SpeechCommonAcousticRefusal, Source);
/// Fixed coefficients per prepared segment. The cursor starts from explicit zero
/// filter state; coefficient changes require a new segment and explicit reset.
pub struct PreparedSpeechGestureRenderer<'a> {
    original: &'a PreparedDeclaredPhoneGestures,
    basis: AudioSampleRateBasis,
    basis_frame: Vec<u8>,
    endpoints: Vec<AudioSampleProjectionReceipt>,
    trajectories: Vec<SpeechGestureAudioTrajectory>,
    quantity_proofs: Vec<AudioTrajectoryEvaluation>,
    coefficients: Vec<PreparedSpeechResonatorQ14>,
    target_tracks: Vec<PreparedAudioQuantityTrajectory>,
    selected_targets: Vec<AudioTrajectoryEvaluation>,
    cycle: AudioSampleProjectionReceipt,
    original_cycle: AudioCycleDuration,
    q8_initialization: Option<SpeechGestureDspQ8ProfileRequest>,
    original_cycle_frame: Vec<u8>,
    profile_executions: Vec<SpeechCommonAcousticExecution>,
    profile_frame: Vec<u8>,
    windows: SpeechGestureFrameWindows,
    windows_frame: Vec<u8>,
    first_frame: i32,
    last_frame: i32,
    target: dsp::SpeechAcousticTarget,
    period: i32,
    initial_state: dsp::SpeechFrameState,
    initial_phase_q8: i32,
}
impl<'a> PreparedSpeechGestureRenderer<'a> {
    pub fn original(&self) -> &PreparedDeclaredPhoneGestures {
        self.original
    }
    pub fn basis(&self) -> &AudioSampleRateBasis {
        &self.basis
    }
    pub fn basis_canonical(&self) -> &[u8] {
        &self.basis_frame
    }
    pub fn endpoint_projections(&self) -> &[AudioSampleProjectionReceipt] {
        &self.endpoints
    }
    pub fn trajectories(&self) -> &[SpeechGestureAudioTrajectory] {
        &self.trajectories
    }
    pub fn quantity_evaluations(&self) -> &[AudioTrajectoryEvaluation] {
        &self.quantity_proofs
    }
    pub fn target_tracks(&self) -> &[PreparedAudioQuantityTrajectory] {
        &self.target_tracks
    }
    pub fn selected_target_evaluations(&self) -> &[AudioTrajectoryEvaluation] {
        &self.selected_targets
    }
    pub fn coefficients(&self) -> &[PreparedSpeechResonatorQ14] {
        &self.coefficients
    }
    pub fn q8_initialization(&self) -> Option<&SpeechGestureDspQ8ProfileRequest> {
        self.q8_initialization.as_ref()
    }
    pub fn original_cycle(&self) -> &AudioCycleDuration {
        &self.original_cycle
    }
    pub fn original_cycle_canonical(&self) -> &[u8] {
        &self.original_cycle_frame
    }
    pub fn admitted_profile_canonical(&self) -> &[u8] {
        &self.profile_frame
    }
    pub fn profile_executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.profile_executions
    }
    pub fn cycle_projection(&self) -> &AudioSampleProjectionReceipt {
        &self.cycle
    }
    pub fn windows(&self) -> &SpeechGestureFrameWindows {
        &self.windows
    }
    pub fn windows_canonical(&self) -> &[u8] {
        &self.windows_frame
    }
    pub fn frame_range(&self) -> core::ops::Range<i32> {
        self.first_frame..self.last_frame
    }
    pub fn cursor(&self) -> SpeechGestureRenderCursor<'_, 'a> {
        SpeechGestureRenderCursor {
            owner: self,
            frame: self.first_frame,
            phase_q8: self.initial_phase_q8,
            state: self.initial_state,
        }
    }
    pub fn next(
        &self,
        cursor: &mut SpeechGestureRenderCursor<'_, 'a>,
    ) -> Result<Option<SpeechGestureRenderedFrame>, SpeechGestureRenderRefusal> {
        self.next_with_period(cursor, self.period)
    }
    pub(crate) fn next_with_period(
        &self,
        cursor: &mut SpeechGestureRenderCursor<'_, 'a>,
        period_q8: i32,
    ) -> Result<Option<SpeechGestureRenderedFrame>, SpeechGestureRenderRefusal> {
        if !core::ptr::eq(cursor.owner, self) {
            return Err(SpeechGestureRenderRefusal::ForeignBasis);
        }
        if cursor.frame == self.last_frame {
            return Ok(None);
        }
        if cursor.frame < self.first_frame || cursor.frame > self.last_frame {
            return Err(SpeechGestureRenderRefusal::ForeignBasis);
        }
        let query = SpeechGestureFrameQuery::new(cursor.frame, self.windows.clone())?;
        let mut executions = Vec::new();
        let gates =
            SpeechGestureFrameGates::decode(&execute(FRAME_GATES, query, &mut executions)?)?;
        let input = dsp::SpeechGestureControlledFrame {
            gates: dsp::SpeechGestureFrameGates {
                closure: *gates.closure(),
                release: *gates.release(),
                aspiration: *gates.aspiration(),
                frication: *gates.frication(),
                voiced: *gates.voiced(),
            },
            frame: dsp::SpeechFrameInput {
                cycle: dsp::SpeechFrameCycleControl {
                    mode: dsp::SpeechCycleControlMode::resolved,
                    phase_q8: cursor.phase_q8,
                    period_q8,
                },
                attack: false,
                release: false,
                target: self.target,
                period: 1,
                frame: cursor.frame,
                state: cursor.state,
            },
        };
        let input_canonical = encode_input(input)?;
        let result = dsp::speech_gesture_controlled_frame(input)
            .ok_or(SpeechGestureRenderRefusal::DspArithmetic)?;
        let output_canonical = encode_output(result)?;
        let output = SpeechGestureRenderedFrame {
            frame: cursor.frame,
            sample: result.sample,
            gates,
            executions,
            input_canonical,
            output_canonical,
            input_phase_q8: input.frame.cycle.phase_q8,
            output_phase_q8: result.phase_q8,
        };
        cursor.state = result.state;
        cursor.phase_q8 = result.phase_q8;
        cursor.frame += 1;
        Ok(Some(output))
    }
}
/// Opaque cursor cannot be forged or transferred by editing its state.
impl SpeechGestureRenderCursor<'_, '_> {
    pub(crate) fn frame_number(&self) -> i32 {
        self.frame
    }
}
pub struct SpeechGestureRenderCursor<'p, 's> {
    owner: &'p PreparedSpeechGestureRenderer<'s>,
    frame: i32,
    phase_q8: i32,
    state: dsp::SpeechFrameState,
}
pub struct SpeechGestureRenderedFrame {
    frame: i32,
    sample: i32,
    gates: SpeechGestureFrameGates,
    executions: Vec<SpeechCommonAcousticExecution>,
    input_canonical: Vec<u8>,
    output_canonical: Vec<u8>,
    input_phase_q8: i32,
    output_phase_q8: i32,
}
impl SpeechGestureRenderedFrame {
    pub fn input_phase_q8(&self) -> i32 {
        self.input_phase_q8
    }
    pub fn output_phase_q8(&self) -> i32 {
        self.output_phase_q8
    }
    pub fn dsp_input_canonical(&self) -> &[u8] {
        &self.input_canonical
    }
    pub fn dsp_output_canonical(&self) -> &[u8] {
        &self.output_canonical
    }
    pub fn frame(&self) -> i32 {
        self.frame
    }
    pub fn sample(&self) -> i32 {
        self.sample
    }
    pub fn gates(&self) -> &SpeechGestureFrameGates {
        &self.gates
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
}
fn projection(
    prepared: &PreparedAudioSampleProjection,
    basis: &AudioSampleRateBasis,
    time: &AudioTimeFraction,
) -> Result<AudioSampleProjectionReceipt, SpeechGestureRenderRefusal> {
    let request = AudioSampleProjectionRequest::new(
        basis.clone(),
        AudioSampleProjectionQuantity::duration(*time.denominator(), *time.numerator_seconds())?,
    )?;
    Ok(prepared.project(&request.encode()?)?)
}
fn frame(receipt: &AudioSampleProjectionReceipt) -> Result<i32, SpeechGestureRenderRefusal> {
    i32::try_from(*receipt.result().raw().whole_frames())
        .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)
}
/// Only the opaque declared-gesture owner enters this profile. A contextual
/// caller retains its existing opaque choice/common-IPA join outside this port.
pub fn prepare_speech_gesture_renderer<'a>(
    original: &'a PreparedDeclaredPhoneGestures,
    basis_frame: &[u8],
    cycle_frame: &[u8],
) -> Result<PreparedSpeechGestureRenderer<'a>, SpeechGestureRenderRefusal> {
    if original.profile_identity() != "speech/authored-acoustic-gesture-demo/1" {
        return Err(SpeechGestureRenderRefusal::UnsupportedProfile);
    }
    prepare_speech_gesture_renderer_at_time(
        original,
        basis_frame,
        cycle_frame,
        original.original_timing().nominal_start(),
    )
}
pub(crate) fn prepare_speech_gesture_renderer_at_time<'a>(
    original: &'a PreparedDeclaredPhoneGestures,
    basis_frame: &[u8],
    cycle_frame: &[u8],
    target_time: &AudioTimeFraction,
) -> Result<PreparedSpeechGestureRenderer<'a>, SpeechGestureRenderRefusal> {
    prepare_speech_gesture_renderer_with_cycle_profile(
        original,
        basis_frame,
        cycle_frame,
        target_time,
        false,
    )
}
pub(crate) fn prepare_speech_gesture_renderer_q8_at_time<'a>(
    original: &'a PreparedDeclaredPhoneGestures,
    basis_frame: &[u8],
    cycle_frame: &[u8],
    target_time: &AudioTimeFraction,
) -> Result<PreparedSpeechGestureRenderer<'a>, SpeechGestureRenderRefusal> {
    prepare_speech_gesture_renderer_with_cycle_profile(
        original,
        basis_frame,
        cycle_frame,
        target_time,
        true,
    )
}
fn prepare_speech_gesture_renderer_with_cycle_profile<'a>(
    original: &'a PreparedDeclaredPhoneGestures,
    basis_frame: &[u8],
    cycle_frame: &[u8],
    target_time: &AudioTimeFraction,
    fractional_q8: bool,
) -> Result<PreparedSpeechGestureRenderer<'a>, SpeechGestureRenderRefusal> {
    let basis = AudioSampleRateBasis::decode(basis_frame)?;
    if basis.anchor() != original.original_timing().anchor() {
        return Err(SpeechGestureRenderRefusal::ForeignBasis);
    }
    if ![8000, 16000, 48000].contains(basis.sample_rate_hz()) {
        return Err(SpeechGestureRenderRefusal::UnsupportedProfile);
    }
    let rate = PreparedAudioSampleProjection::new()?;
    let start = projection(&rate, &basis, original.original_timing().nominal_start())?;
    let end = projection(&rate, &basis, original.original_timing().nominal_end())?;
    let first_frame = frame(&start)?;
    let last_frame = frame(&end)?;
    if last_frame > 48000 || first_frame >= last_frame {
        return Err(SpeechGestureRenderRefusal::ResourceBound);
    }
    let mut endpoints = alloc::vec![start, end];
    let empty = SpeechGestureFrameWindow::new(0, 0)?;
    let (mut closure, mut release, mut aspiration, mut frication, mut voicing) = (
        empty.clone(),
        empty.clone(),
        empty.clone(),
        empty.clone(),
        empty,
    );
    let mut center_segments: [Vec<AudioTrajectorySegment>; 3] =
        core::array::from_fn(|_| Vec::new());
    let mut width_segments: [Vec<AudioTrajectorySegment>; 3] = core::array::from_fn(|_| Vec::new());
    let mut trajectories = Vec::new();
    let mut quantity_proofs = Vec::new();
    for gesture in original.gestures() {
        let begin = projection(&rate, &basis, gesture.start())?;
        let finish = projection(&rate, &basis, gesture.end())?;
        let window = SpeechGestureFrameWindow::new(frame(&finish)?, frame(&begin)?)?;
        endpoints.push(begin);
        endpoints.push(finish);
        let trajectory = gesture_to_audio_trajectory(&gesture.clone().encode()?)?;
        let evaluator = PreparedAudioQuantityTrajectory::new(trajectory.admitted_canonical())?;
        let query = AudioTrajectoryQuery::new(
            basis.anchor().clone(),
            AudioExactTimeOffset::new(
                *gesture.start().denominator(),
                *gesture.start().numerator_seconds(),
            )?,
        )?;
        let proof = evaluator.evaluate(&query.encode()?)?;
        match gesture.channel() {
            SpeechGestureChannel::FormantCenter | SpeechGestureChannel::FormantBandwidth => {
                let index = usize::try_from(*gesture.formant_index())
                    .map_err(|_| SpeechGestureRenderRefusal::UnsupportedProfile)?
                    .checked_sub(1)
                    .ok_or(SpeechGestureRenderRefusal::UnsupportedProfile)?;
                let slot = if matches!(gesture.channel(), SpeechGestureChannel::FormantCenter) {
                    center_segments.get_mut(index)
                } else {
                    width_segments.get_mut(index)
                }
                .ok_or(SpeechGestureRenderRefusal::UnsupportedProfile)?;
                slot.extend_from_slice(trajectory.trajectory().segments().as_slice());
            }
            SpeechGestureChannel::Closure => closure = window,
            SpeechGestureChannel::Release => release = window,
            SpeechGestureChannel::Aspiration => aspiration = window,
            SpeechGestureChannel::Frication => frication = window,
            SpeechGestureChannel::LaryngealVoicing => voicing = window,
        }
        trajectories.push(trajectory);
        quantity_proofs.push(proof);
    }
    let windows = SpeechGestureFrameWindows::new(
        aspiration, closure, last_frame, frication, release, voicing,
    )?;
    let windows_frame = windows.clone().encode()?;
    let windows = SpeechGestureFrameWindows::decode(&windows_frame)?;
    let mut target_tracks = Vec::new();
    let mut selected_targets = Vec::new();
    let mut frequencies = Vec::new();
    for segments in center_segments.into_iter().chain(width_segments) {
        let sequence = conduit_plot::rust_binding::BoundedSequence::try_from_iter(segments)
            .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?;
        let trajectory = AudioQuantityTrajectory::new(
            basis.anchor().clone(),
            AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
            AudioTrajectoryOutside::Refuse,
            AudioTrajectoryProvenance::new(
                AudioTrajectoryProvenanceKind::Derived,
                "speech/gesture-renderer-target-selection".into(),
                Some("2".into()),
            )?,
            sequence,
        )?;
        let prepared = PreparedAudioQuantityTrajectory::new(&trajectory.encode()?)?;
        let query = AudioTrajectoryQuery::new(
            basis.anchor().clone(),
            AudioExactTimeOffset::new(
                *target_time.denominator(),
                *target_time.numerator_seconds(),
            )?,
        )?;
        let proof = prepared.evaluate(&query.encode()?)?;
        let AudioTrajectoryQuantity::Frequency(f) = proof.result() else {
            return Err(SpeechGestureRenderRefusal::UnsupportedProfile);
        };
        frequencies.push(AudioFrequencyHz::new(*f.denominator(), *f.numerator_hz())?);
        target_tracks.push(prepared);
        selected_targets.push(proof);
    }
    let mut coefficients = Vec::new();
    for index in 0..3 {
        let request = SpeechResonatorProjectionRequest::new(
            SpeechResonatorCoefficientProfile::Q20Series8Q14Nearest,
            AudioResonator::new(frequencies[index + 3], frequencies[index])?,
            *basis.sample_rate_hz(),
        )?;
        coefficients.push(prepare_speech_resonator_q14(&request.encode()?)?);
    }
    let cycle_original = AudioCycleDuration::decode(cycle_frame)?;
    let cycle_request = AudioSampleProjectionRequest::new(
        basis.clone(),
        AudioSampleProjectionQuantity::cycle(
            *cycle_original.denominator(),
            *cycle_original.numerator_seconds(),
        )?,
    )?;
    let cycle = rate.project(&cycle_request.encode()?)?;
    let mut profile_executions = Vec::new();
    let (raw, q8_initialization) = if fractional_q8 {
        let request = SpeechCycleAtRateRequest::new(
            SpeechFundamentalCycle::new(
                *cycle_original.denominator(),
                *cycle_original.numerator_seconds(),
            )?,
            *basis.sample_rate_hz(),
        )?;
        let projected = crate::control::cycle_q8(&request)?;
        let admission = SpeechGestureDspQ8ProfileRequest::new(cycle.original().clone(), projected)?;
        let raw = SpeechGestureDspQ8RawProfile::decode(&execute(
            DSP_Q8_PROFILE,
            admission.clone(),
            &mut profile_executions,
        )?)?;
        let exact = SpeechGestureDspRawProfile::new(
            *raw.bypass_gain(),
            *raw.gain1(),
            *raw.gain2(),
            *raw.gain3(),
            i32::try_from(*raw.period_q8())
                .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?,
            *raw.phase_q8(),
        )?;
        (exact, Some(admission))
    } else {
        // Preserve the established exact-integer entrance and its refusals.
        let whole = *cycle.result().raw().whole_frames();
        let cycle_frames =
            i32::try_from(whole).map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?;
        let raw = SpeechGestureDspRawProfile::decode(&execute(
            DSP_PROFILE,
            SpeechGestureDspProfileRequest::new(
                cycle_frames,
                *cycle.result().raw().remainder_numerator(),
            )?,
            &mut profile_executions,
        )?)?;
        (raw, None)
    };
    let profile = SpeechGestureDspProfile::new(
        *raw.bypass_gain(),
        *raw.gain1(),
        *raw.gain2(),
        *raw.gain3(),
        *raw.period_q8(),
        *raw.phase_q8(),
    )?;
    let profile_frame = profile.clone().encode()?;
    let profile = SpeechGestureDspProfile::decode(&profile_frame)?;
    let period = *profile.period_q8();
    let initial_phase_q8 = *profile.phase_q8();
    let initial = native_dsp::SpeechFrameState::decode(&execute(
        speech_gesture_dsp_programs::INITIAL_STATE,
        native_dsp::SpeechStart::Begin,
        &mut profile_executions,
    )?)?;
    let initial_state = dsp::SpeechFrameState {
        voicing: *initial.voicing(),
        phase: *initial.phase(),
        noise: *initial.noise(),
        first1: *initial.first1(),
        first2: *initial.first2(),
        second1: *initial.second1(),
        second2: *initial.second2(),
        third1: *initial.third1(),
        third2: *initial.third2(),
    };
    let (b1, c1) = coefficients[0].dsp_coefficients()?;
    let (b2, c2) = coefficients[1].dsp_coefficients()?;
    let (b3, c3) = coefficients[2].dsp_coefficients()?;
    let target = dsp::SpeechAcousticTarget {
        f1: i32::try_from(
            *coefficients[0]
                .original()
                .resonator()
                .center()
                .numerator_hz(),
        )
        .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?,
        f2: i32::try_from(
            *coefficients[1]
                .original()
                .resonator()
                .center()
                .numerator_hz(),
        )
        .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?,
        f3: i32::try_from(
            *coefficients[2]
                .original()
                .resonator()
                .center()
                .numerator_hz(),
        )
        .map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?,
        b1,
        b2,
        b3,
        c1,
        c2,
        c3,
        gain1: *profile.gain1(),
        gain2: *profile.gain2(),
        gain3: *profile.gain3(),
        bypass_gain: *profile.bypass_gain(),
        voiced: 0,
        frication: 0,
        frames: last_frame,
        closure: 0,
    };
    Ok(PreparedSpeechGestureRenderer {
        original,
        basis,
        basis_frame: basis_frame.into(),
        endpoints,
        trajectories,
        quantity_proofs,
        coefficients,
        target_tracks,
        selected_targets,
        cycle,
        original_cycle: cycle_original,
        q8_initialization,
        original_cycle_frame: cycle_frame.into(),
        profile_executions,
        profile_frame,
        windows,
        windows_frame,
        first_frame,
        last_frame,
        target,
        period,
        initial_state,
        initial_phase_q8,
    })
}

pub(crate) fn transfer_greeting_cursor<'p, 's>(
    source: &SpeechGestureRenderCursor<'_, 's>,
    target: &'p PreparedSpeechGestureRenderer<'s>,
    executions: &mut Vec<SpeechCommonAcousticExecution>,
) -> Result<SpeechGestureRenderCursor<'p, 's>, SpeechGestureRenderRefusal> {
    let state = native_dsp::SpeechFrameState::decode(&execute(
        speech_gesture_dsp_programs::GREETING_FILTER_RESET,
        state_value(source.state)?,
        executions,
    )?)?;
    let state = dsp::SpeechFrameState {
        voicing: *state.voicing(),
        phase: *state.phase(),
        noise: *state.noise(),
        first1: *state.first1(),
        first2: *state.first2(),
        second1: *state.second1(),
        second2: *state.second2(),
        third1: *state.third1(),
        third2: *state.third2(),
    };
    Ok(SpeechGestureRenderCursor {
        owner: target,
        frame: source.frame,
        phase_q8: source.phase_q8,
        state,
    })
}

fn state_value(
    s: dsp::SpeechFrameState,
) -> Result<native_dsp::SpeechFrameState, NativeBindingRefusal> {
    native_dsp::SpeechFrameState::new(
        s.first1, s.first2, s.noise, s.phase, s.second1, s.second2, s.third1, s.third2, s.voicing,
    )
}
fn encode_input(i: dsp::SpeechGestureControlledFrame) -> Result<Vec<u8>, NativeBindingRefusal> {
    let g = i.gates;
    let f = i.frame;
    let t = f.target;
    let c = f.cycle;
    let target = native_dsp::SpeechAcousticTarget::new(
        t.b1,
        t.b2,
        t.b3,
        t.bypass_gain,
        t.c1,
        t.c2,
        t.c3,
        t.closure,
        t.f1,
        t.f2,
        t.f3,
        t.frames,
        t.frication,
        t.gain1,
        t.gain2,
        t.gain3,
        t.voiced,
    )?;
    let cycle = native_dsp::SpeechFrameCycleControl::new(
        match c.mode {
            dsp::SpeechCycleControlMode::profile => native_dsp::SpeechCycleControlMode::Profile,
            dsp::SpeechCycleControlMode::resolved => native_dsp::SpeechCycleControlMode::Resolved,
        },
        c.period_q8,
        c.phase_q8,
    )?;
    let frame = native_dsp::SpeechFrameInput::new(
        f.attack,
        cycle,
        f.frame,
        f.period,
        f.release,
        state_value(f.state)?,
        target,
    )?;
    let gates = native_dsp::SpeechGestureFrameGates::new(
        g.aspiration,
        g.closure,
        g.frication,
        g.release,
        g.voiced,
    )?;
    let value = native_dsp::SpeechGestureControlledFrame::new(frame, gates)?;
    let canonical = value.encode()?;
    native_dsp::SpeechGestureControlledFrame::decode(&canonical)?;
    Ok(canonical)
}
fn encode_output(o: dsp::SpeechFrameResult) -> Result<Vec<u8>, NativeBindingRefusal> {
    let value = native_dsp::SpeechFrameResult::new(o.phase_q8, o.sample, state_value(o.state)?)?;
    let canonical = value.encode()?;
    native_dsp::SpeechFrameResult::decode(&canonical)?;
    Ok(canonical)
}
