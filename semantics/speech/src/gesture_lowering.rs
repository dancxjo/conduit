//! Exact bounded authored acoustic gestures. A declared selected phone is not
//! itself evidence of contextual selection; the production choice bridge retains
//! the existing opaque allophone owner separately.
use crate::{
    common_acoustic_quantities::{
        boolean, execute, SpeechCommonAcousticExecution, SpeechCommonAcousticRefusal,
    },
    gesture_programs::*,
    semantic::*,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum SpeechGestureRefusal {
    Admission(NativeBindingRefusal),
    Execution(SpeechCommonAcousticRefusal),
    UnsupportedSymbol,
    UnselectedChoice,
    UnsupportedFeatures,
    ForeignOccurrence,
    ForeignBasis,
    Conflict,
    UnsupportedRendererProjection,
}
impl From<NativeBindingRefusal> for SpeechGestureRefusal {
    fn from(e: NativeBindingRefusal) -> Self {
        Self::Admission(e)
    }
}
impl From<SpeechCommonAcousticRefusal> for SpeechGestureRefusal {
    fn from(e: SpeechCommonAcousticRefusal) -> Self {
        Self::Execution(e)
    }
}
pub struct PreparedDeclaredPhoneGestures {
    pub(crate) event: SpeechUtteranceIntentEvent,
    pub(crate) membership: SpeechOccurrenceMembership,
    pub(crate) phone: SpeechPhone,
    pub(crate) choice: SpeechAllophoneChoiceState,
    pub(crate) timing: SpeechGestureTiming,
    pub(crate) original_frames: Vec<Vec<u8>>,
    pub(crate) executions: Vec<SpeechCommonAcousticExecution>,
    pub(crate) gestures: Vec<SpeechAcousticGesture>,
    pub(crate) admitted_frames: Vec<Vec<u8>>,
    pub(crate) profile: &'static str,
}
impl PreparedDeclaredPhoneGestures {
    pub fn original_event(&self) -> &SpeechUtteranceIntentEvent {
        &self.event
    }
    pub fn membership(&self) -> &SpeechOccurrenceMembership {
        &self.membership
    }
    pub fn declared_phone(&self) -> &SpeechPhone {
        &self.phone
    }
    pub fn declared_choice(&self) -> &SpeechAllophoneChoiceState {
        &self.choice
    }
    pub fn original_timing(&self) -> &SpeechGestureTiming {
        &self.timing
    }
    /// In order: segment, membership, phone, choice, original timing.
    pub fn original_canonical_frames(&self) -> &[Vec<u8>] {
        &self.original_frames
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn gestures(&self) -> &[SpeechAcousticGesture] {
        &self.gestures
    }
    pub fn admitted_canonical_frames(&self) -> &[Vec<u8>] {
        &self.admitted_frames
    }
    pub fn profile_identity(&self) -> &'static str {
        self.profile
    }
    pub fn conflict_policy(&self) -> SpeechGestureConflictPolicy {
        SpeechGestureConflictPolicy::IndependentChannelsRefuseSameChannelOverlap
    }
    /// Width-to-coefficient projection, clock mapping and renderer effects are not
    /// supplied by this component. Callers cannot mistake traces for playable PCM.
    pub fn require_renderer_projection(&self) -> Result<(), SpeechGestureRefusal> {
        Err(SpeechGestureRefusal::UnsupportedRendererProjection)
    }
}
/// Ordinary allocating preparation of explicitly declared material. Context
/// authority is supplied by the existing opaque allophone choice at composition.
pub fn prepare_declared_phone_gestures(
    segment_frame: &[u8],
    membership_frame: &[u8],
    phone_frame: &[u8],
    choice_frame: &[u8],
    timing_frame: &[u8],
) -> Result<PreparedDeclaredPhoneGestures, SpeechGestureRefusal> {
    prepare_declared_phone_gestures_with_profile(
        segment_frame,
        membership_frame,
        phone_frame,
        choice_frame,
        timing_frame,
        false,
    )
}
pub(crate) fn prepare_declared_phone_gestures_with_profile(
    segment_frame: &[u8],
    membership_frame: &[u8],
    phone_frame: &[u8],
    choice_frame: &[u8],
    timing_frame: &[u8],
    reviewed_greeting_features: bool,
) -> Result<PreparedDeclaredPhoneGestures, SpeechGestureRefusal> {
    let event = SpeechUtteranceIntentEvent::decode(segment_frame)?;
    let SpeechUtteranceIntentEvent::Segment(segment) = &event else {
        return Err(SpeechGestureRefusal::ForeignOccurrence);
    };
    let membership = SpeechOccurrenceMembership::decode(membership_frame)?;
    let phone = SpeechPhone::decode(phone_frame)?;
    let choice = SpeechAllophoneChoiceState::decode(choice_frame)?;
    let timing = SpeechGestureTiming::decode(timing_frame)?;
    if segment.occurrence() != membership.occurrence() {
        return Err(SpeechGestureRefusal::ForeignOccurrence);
    }
    // Re-admit the original occurrence against the complete original basis laws.
    SpeechOccurrenceMembership::new(
        membership.inventory_id().clone(),
        membership.language().clone(),
        segment.occurrence().clone(),
        membership.revision_id().clone(),
        membership.utterance_id().clone(),
    )?;
    if let PhoneSpecification::Known(required) = segment.phone() {
        SpeechPhoneDefinitionMatch::new(phone.identity().clone(), required.clone())?;
    }
    let mut executions = Vec::new();
    if reviewed_greeting_features {
        crate::greeting_gestures::admit_reviewed_features(&phone, &mut executions)?;
    } else if !phone.features().get().as_slice().is_empty() {
        return Err(SpeechGestureRefusal::UnsupportedFeatures);
    }
    if !boolean(CHOICE, choice.clone(), &mut executions)? {
        return Err(SpeechGestureRefusal::UnselectedChoice);
    }
    let symbol = SpeechGestureSymbol::new(phone.ipa().clone())?;
    let class = match SpeechGestureSymbolResult::decode(&execute(SYMBOL, symbol, &mut executions)?)?
    {
        SpeechGestureSymbolResult::Supported(v) => v,
        SpeechGestureSymbolResult::Unsupported => {
            return Err(SpeechGestureRefusal::UnsupportedSymbol)
        }
    };
    let eligible = SpeechGestureU32Timing::new(timing.clone())?;
    let phases = SpeechGestureRawPhases::decode(&execute(FIFTHS, eligible, &mut executions)?)?;
    let formants = SpeechGestureRawFormants::decode(&execute(FORMANTS, class, &mut executions)?)?;
    let mut gestures = Vec::new();
    let mut admitted_frames = Vec::new();
    for channel in [
        SpeechGestureChannel::LaryngealVoicing,
        SpeechGestureChannel::Aspiration,
        SpeechGestureChannel::Frication,
        SpeechGestureChannel::Closure,
        SpeechGestureChannel::Release,
        SpeechGestureChannel::FormantCenter,
        SpeechGestureChannel::FormantBandwidth,
    ] {
        if !boolean(
            ROLE,
            SpeechGestureRoleRequest::new(channel, class)?,
            &mut executions,
        )? {
            continue;
        }
        let raw = SpeechGestureRawWindow::decode(&execute(
            WINDOW,
            SpeechGestureWindowRequest::new(channel, phases.clone(), class)?,
            &mut executions,
        )?)?;
        let start = conduit_audio::AudioTimeFraction::new(*raw.denominator(), *raw.start())?;
        let end = conduit_audio::AudioTimeFraction::new(*raw.denominator(), *raw.end())?;
        let values: Vec<(u32, conduit_audio::AudioTrajectoryQuantity)> = match channel {
            SpeechGestureChannel::FormantCenter => [
                (1, *formants.first_center()),
                (2, *formants.second_center()),
                (3, *formants.third_center()),
            ]
            .into_iter()
            .map(|(i, n)| {
                Ok((
                    i,
                    conduit_audio::AudioTrajectoryQuantity::frequency(*formants.denominator(), n)?,
                ))
            })
            .collect::<Result<_, NativeBindingRefusal>>()?,
            SpeechGestureChannel::FormantBandwidth => [
                (1, *formants.first_bandwidth()),
                (2, *formants.second_bandwidth()),
                (3, *formants.third_bandwidth()),
            ]
            .into_iter()
            .map(|(i, n)| {
                Ok((
                    i,
                    conduit_audio::AudioTrajectoryQuantity::frequency(*formants.denominator(), n)?,
                ))
            })
            .collect::<Result<_, NativeBindingRefusal>>()?,
            _ => alloc::vec![(
                0,
                conduit_audio::AudioTrajectoryQuantity::amplitude(
                    *raw.gain_denominator(),
                    *raw.gain_numerator()
                )?
            )],
        };
        for (index, quantity) in values {
            let gesture = SpeechAcousticGesture::new(
                timing.anchor().clone(),
                channel,
                end.clone(),
                index,
                segment.occurrence().clone(),
                segment.provenance().clone(),
                quantity,
                SpeechGestureShape::Step,
                segment.sources().clone(),
                start.clone(),
            )?;
            let frame = gesture.encode()?;
            let gesture = SpeechAcousticGesture::decode(&frame)?;
            admitted_frames.push(frame);
            gestures.push(gesture);
        }
    }
    Ok(PreparedDeclaredPhoneGestures {
        profile: "speech/authored-acoustic-gesture-demo/1",
        event,
        membership,
        phone,
        choice,
        timing,
        original_frames: [
            segment_frame,
            membership_frame,
            phone_frame,
            choice_frame,
            timing_frame,
        ]
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect(),
        executions,
        gestures,
        admitted_frames,
    })
}

pub struct SpeechGestureOverlapReceipt {
    originals: [SpeechAcousticGesture; 2],
    frames: [Vec<u8>; 2],
    executions: Vec<SpeechCommonAcousticExecution>,
    conflict: bool,
}
impl SpeechGestureOverlapReceipt {
    pub fn originals(&self) -> &[SpeechAcousticGesture; 2] {
        &self.originals
    }
    pub fn original_canonical_frames(&self) -> &[Vec<u8>; 2] {
        &self.frames
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn conflicts(&self) -> bool {
        self.conflict
    }
    pub fn require_compatible(&self) -> Result<(), SpeechGestureRefusal> {
        if self.conflict {
            Err(SpeechGestureRefusal::Conflict)
        } else {
            Ok(())
        }
    }
}
/// Compare one declared pair under the named independent-channel policy. A
/// foreign anchor or exact rational basis is refused, never silently converted.
pub fn compare_gesture_overlap(
    left: &[u8],
    right: &[u8],
) -> Result<SpeechGestureOverlapReceipt, SpeechGestureRefusal> {
    let left_g = SpeechAcousticGesture::decode(left)?;
    let right_g = SpeechAcousticGesture::decode(right)?;
    SpeechGestureCommonBasis::new(left_g.clone())?;
    SpeechGestureCommonBasis::new(right_g.clone())?;
    if left_g.start().denominator() != right_g.start().denominator() {
        return Err(SpeechGestureRefusal::ForeignBasis);
    }
    let mut executions = Vec::new();
    if !boolean(
        crate::common_acoustic_programs::ANCHOR,
        SpeechAcousticAnchorComparison::new(left_g.anchor().clone(), right_g.anchor().clone())?,
        &mut executions,
    )? {
        return Err(SpeechGestureRefusal::ForeignBasis);
    }
    let input = SpeechGestureOverlapRequest::new(
        *left_g.channel(),
        *left_g.end().numerator_seconds(),
        *left_g.formant_index(),
        *left_g.start().numerator_seconds(),
        *right_g.channel(),
        *right_g.end().numerator_seconds(),
        *right_g.formant_index(),
        *right_g.start().numerator_seconds(),
    )?;
    let conflict = boolean(CONFLICT, input, &mut executions)?;
    Ok(SpeechGestureOverlapReceipt {
        originals: [left_g, right_g],
        frames: [left.to_vec(), right.to_vec()],
        executions,
        conflict,
    })
}

pub struct SpeechGestureAudioTrajectory {
    original: SpeechAcousticGesture,
    original_frame: Vec<u8>,
    trajectory: conduit_audio::AudioQuantityTrajectory,
    admitted_frame: Vec<u8>,
}
impl SpeechGestureAudioTrajectory {
    pub fn original(&self) -> &SpeechAcousticGesture {
        &self.original
    }
    pub fn original_canonical(&self) -> &[u8] {
        &self.original_frame
    }
    pub fn trajectory(&self) -> &conduit_audio::AudioQuantityTrajectory {
        &self.trajectory
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted_frame
    }
}
/// Exact field-copy admission to the existing Audio step trajectory carrier.
/// This does not claim that Audio's separate U8 numeric evaluation profile can
/// evaluate every admitted frequency, or that any renderer implements it.
pub fn gesture_to_audio_trajectory(
    frame: &[u8],
) -> Result<SpeechGestureAudioTrajectory, SpeechGestureRefusal> {
    use conduit_audio::*;
    let original = SpeechAcousticGesture::decode(frame)?;
    SpeechGestureCommonBasis::new(original.clone())?;
    let start = AudioExactTimeOffset::new(
        *original.start().denominator(),
        *original.start().numerator_seconds(),
    )?;
    let end = AudioExactTimeOffset::new(
        *original.end().denominator(),
        *original.end().numerator_seconds(),
    )?;
    let segment = AudioTrajectorySegment::new(
        end,
        AudioTrajectoryInterpolation::Step,
        original.quantity().clone(),
        original.quantity().clone(),
        start,
    )?;
    let provenance = AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Derived,
        "speech/authored-acoustic-gesture-demo".into(),
        Some("1".into()),
    )?;
    let trajectory = AudioQuantityTrajectory::new(
        original.anchor().clone(),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance,
        conduit_plot::rust_binding::BoundedSequence::try_from_iter([segment])
            .map_err(|_| SpeechGestureRefusal::ForeignBasis)?,
    )?;
    let admitted_frame = trajectory.encode()?;
    let trajectory = AudioQuantityTrajectory::decode(&admitted_frame)?;
    Ok(SpeechGestureAudioTrajectory {
        original,
        original_frame: frame.to_vec(),
        trajectory,
        admitted_frame,
    })
}

pub struct PreparedAuthoredAcousticGesture {
    eligible: SpeechAuthoredGestureEligibility,
    originals: [Vec<u8>; 2],
    admitted: Vec<u8>,
}
impl PreparedAuthoredAcousticGesture {
    pub fn original_gesture(&self) -> &SpeechAcousticGesture {
        self.eligible.gesture()
    }
    pub fn original_timing(&self) -> &SpeechGestureTiming {
        self.eligible.timing()
    }
    pub fn original_canonical_frames(&self) -> &[Vec<u8>; 2] {
        &self.originals
    }
    pub fn admitted_eligibility_canonical(&self) -> &[u8] {
        &self.admitted
    }
}
/// Explicit manual gesture admission executes the original Source where laws
/// through Native admission. It performs no interpolation or arithmetic.
pub fn prepare_authored_acoustic_gesture(
    gesture_frame: &[u8],
    timing_frame: &[u8],
) -> Result<PreparedAuthoredAcousticGesture, SpeechGestureRefusal> {
    let gesture = SpeechAcousticGesture::decode(gesture_frame)?;
    let timing = SpeechGestureTiming::decode(timing_frame)?;
    let eligible = SpeechAuthoredGestureEligibility::new(gesture, timing)?;
    let admitted = eligible.encode()?;
    let eligible = SpeechAuthoredGestureEligibility::decode(&admitted)?;
    Ok(PreparedAuthoredAcousticGesture {
        eligible,
        originals: [gesture_frame.to_vec(), timing_frame.to_vec()],
        admitted,
    })
}
