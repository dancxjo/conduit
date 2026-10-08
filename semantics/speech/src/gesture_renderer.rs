//! Bounded explicit-window projection into the existing checked Source frame DSP.
//! No implicit clock mapping, allophone selection, or general modulation model.
use crate::{
    common_acoustic_quantities::{
        execute, SpeechCommonAcousticExecution, SpeechCommonAcousticRefusal,
    },
    generated as dsp,
    resonator_programs::{DSP_PROFILE, FRAME_GATES},
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
    cycle: AudioSampleProjectionReceipt,
    original_cycle: AudioCycleDuration,
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
    pub fn coefficients(&self) -> &[PreparedSpeechResonatorQ14] {
        &self.coefficients
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
                    period_q8: self.period,
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
        };
        cursor.state = result.state;
        cursor.phase_q8 = result.phase_q8;
        cursor.frame += 1;
        Ok(Some(output))
    }
}
/// Opaque cursor cannot be forged or transferred by editing its state.
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
}
impl SpeechGestureRenderedFrame {
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
    let mut centers: [Option<AudioFrequencyHz>; 3] = [None, None, None];
    let mut widths: [Option<AudioFrequencyHz>; 3] = [None, None, None];
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
                let AudioTrajectoryQuantity::Frequency(f) = proof.result() else {
                    return Err(SpeechGestureRenderRefusal::UnsupportedProfile);
                };
                let index = usize::try_from(*gesture.formant_index())
                    .map_err(|_| SpeechGestureRenderRefusal::UnsupportedProfile)?
                    .checked_sub(1)
                    .ok_or(SpeechGestureRenderRefusal::UnsupportedProfile)?;
                let slot = if matches!(gesture.channel(), SpeechGestureChannel::FormantCenter) {
                    centers.get_mut(index)
                } else {
                    widths.get_mut(index)
                }
                .ok_or(SpeechGestureRenderRefusal::UnsupportedProfile)?;
                if slot
                    .replace(AudioFrequencyHz::new(*f.denominator(), *f.numerator_hz())?)
                    .is_some()
                {
                    return Err(SpeechGestureRenderRefusal::UnsupportedProfile);
                }
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
    let mut coefficients = Vec::new();
    for (center, width) in centers.into_iter().zip(widths) {
        let request = SpeechResonatorProjectionRequest::new(
            SpeechResonatorCoefficientProfile::Q20Series8Q14Nearest,
            AudioResonator::new(
                width.ok_or(SpeechGestureRenderRefusal::UnsupportedProfile)?,
                center.ok_or(SpeechGestureRenderRefusal::UnsupportedProfile)?,
            )?,
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
    // Exact integer period only: the general Q8/fractional-cycle profile is not
    // silently approximated. All authored cycle fractions remain in the receipt.
    let whole = *cycle.result().raw().whole_frames();
    let cycle_frames =
        i32::try_from(whole).map_err(|_| SpeechGestureRenderRefusal::ResourceBound)?;
    let mut profile_executions = Vec::new();
    let raw = SpeechGestureDspRawProfile::decode(&execute(
        DSP_PROFILE,
        SpeechGestureDspProfileRequest::new(
            cycle_frames,
            *cycle.result().raw().remainder_numerator(),
        )?,
        &mut profile_executions,
    )?)?;
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
        cycle,
        original_cycle: cycle_original,
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
