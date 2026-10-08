//! Explicit v2 authored approximation, retaining existing original owners.
use crate::{
    common_acoustic_quantities::{boolean, execute},
    greeting_programs::*,
    semantic::*,
    *,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
pub struct PreparedGreetingPhoneGestures {
    lowered: PreparedDeclaredPhoneGestures,
    policy: SpeechGreetingLossPolicy,
    policy_frame: Vec<u8>,
    symbol_frame: Vec<u8>,
    request: Option<SpeechGreetingPolicyRequest>,
    request_frame: Option<Vec<u8>>,
    admitted_policy_frame: Option<Vec<u8>>,
    effect: SpeechGreetingApproximationEffect,
    effect_frame: Vec<u8>,
    second_start: Option<conduit_audio::AudioTimeFraction>,
}
impl PreparedGreetingPhoneGestures {
    pub fn lowered(&self) -> &PreparedDeclaredPhoneGestures {
        &self.lowered
    }
    pub fn selected_policy_canonical(&self) -> &[u8] {
        &self.policy_frame
    }
    pub fn original_symbol_canonical(&self) -> &[u8] {
        &self.symbol_frame
    }
    pub fn selected_policy(&self) -> &SpeechGreetingLossPolicy {
        &self.policy
    }
    pub fn original_policy_request(&self) -> Option<&SpeechGreetingPolicyRequest> {
        self.request.as_ref()
    }
    pub fn original_policy_request_canonical(&self) -> Option<&[u8]> {
        self.request_frame.as_deref()
    }
    pub fn admitted_policy_canonical(&self) -> Option<&[u8]> {
        self.admitted_policy_frame.as_deref()
    }
    pub fn effect(&self) -> &SpeechGreetingApproximationEffect {
        &self.effect
    }
    pub fn effect_canonical(&self) -> &[u8] {
        &self.effect_frame
    }
    pub fn second_start(&self) -> Option<&conduit_audio::AudioTimeFraction> {
        self.second_start.as_ref()
    }
}
pub fn prepare_greeting_phone_gestures(
    segment_frame: &[u8],
    membership_frame: &[u8],
    phone_frame: &[u8],
    choice_frame: &[u8],
    timing_frame: &[u8],
    policy: SpeechGreetingLossPolicy,
) -> Result<PreparedGreetingPhoneGestures, SpeechGestureRefusal> {
    let policy_frame = policy.encode()?;
    match crate::gesture_lowering::prepare_declared_phone_gestures_with_profile(
        segment_frame,
        membership_frame,
        phone_frame,
        choice_frame,
        timing_frame,
        true,
    ) {
        Ok(mut lowered) => {
            let symbol_frame = lowered
                .executions
                .iter()
                .find(|e| e.source_program_hex() == crate::gesture_programs::SYMBOL)
                .ok_or(SpeechGestureRefusal::UnsupportedSymbol)?
                .input_canonical()
                .to_vec();
            let effect_frame = execute(LEGACY_EFFECT, policy, &mut lowered.executions)?;
            let effect = SpeechGreetingApproximationEffect::decode(&effect_frame)?;
            return Ok(PreparedGreetingPhoneGestures {
                lowered,
                policy,
                policy_frame,
                symbol_frame,
                request: None,
                request_frame: None,
                admitted_policy_frame: None,
                effect,
                effect_frame,
                second_start: None,
            });
        }
        Err(SpeechGestureRefusal::UnsupportedSymbol) => {}
        Err(e) => return Err(e),
    }
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
    admit_reviewed_features(&phone, &mut executions)?;
    if !boolean(
        crate::gesture_programs::CHOICE,
        choice.clone(),
        &mut executions,
    )? {
        return Err(SpeechGestureRefusal::UnselectedChoice);
    }

    let symbol = SpeechGestureSymbol::new(phone.ipa().clone())?;
    let symbol_frame = symbol.clone().encode()?;
    let class =
        match SpeechGreetingSymbolResult::decode(&execute(SYMBOL, symbol, &mut executions)?)? {
            SpeechGreetingSymbolResult::Supported(c) => c,
            SpeechGreetingSymbolResult::Unsupported => {
                return Err(SpeechGestureRefusal::UnsupportedSymbol)
            }
        };
    let request = SpeechGreetingPolicyRequest::new(class, policy)?;
    let request_frame = request.clone().encode()?;
    let admitted = SpeechGreetingPolicyAdmission::new(request.clone())?;
    let admitted_policy_frame = admitted.clone().encode()?;
    let admitted = SpeechGreetingPolicyAdmission::decode(&admitted_policy_frame)?;
    let effect_frame = execute(EFFECT, admitted, &mut executions)?;
    let effect = SpeechGreetingApproximationEffect::decode(&effect_frame)?;
    let eligible = SpeechGestureU32Timing::new(timing.clone())?;
    let mut gestures = Vec::new();
    let mut admitted_frames = Vec::new();
    let mut second_start = None;
    for channel in [
        SpeechGestureChannel::LaryngealVoicing,
        SpeechGestureChannel::Aspiration,
        SpeechGestureChannel::Frication,
        SpeechGestureChannel::FormantCenter,
        SpeechGestureChannel::FormantBandwidth,
    ] {
        if !boolean(
            ROLE,
            SpeechGreetingRoleRequest::new(channel, class)?,
            &mut executions,
        )? {
            continue;
        }
        let phases = if boolean(
            SPLIT,
            SpeechGreetingRoleRequest::new(channel, class)?,
            &mut executions,
        )? {
            2
        } else {
            1
        };
        for phase in 0..phases {
            let second = phase == 1;
            let raw = SpeechGestureRawWindow::decode(&execute(
                WINDOW,
                SpeechGreetingWindowRequest::new(channel, class, second, eligible.clone())?,
                &mut executions,
            )?)?;
            let start = conduit_audio::AudioTimeFraction::new(*raw.denominator(), *raw.start())?;
            let end = conduit_audio::AudioTimeFraction::new(*raw.denominator(), *raw.end())?;
            if second {
                second_start = Some(start.clone());
            }
            let formants = SpeechGestureRawFormants::decode(&execute(
                FORMANTS,
                SpeechGreetingFormantRequest::new(class, second)?,
                &mut executions,
            )?)?;
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
                        conduit_audio::AudioTrajectoryQuantity::frequency(
                            *formants.denominator(),
                            n,
                        )?,
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
                        conduit_audio::AudioTrajectoryQuantity::frequency(
                            *formants.denominator(),
                            n,
                        )?,
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
    }
    let lowered = PreparedDeclaredPhoneGestures {
        profile: "speech/authored-greeting-approximation/2",
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
    };
    Ok(PreparedGreetingPhoneGestures {
        lowered,
        policy,
        policy_frame,
        symbol_frame,
        request: Some(request),
        request_frame: Some(request_frame),
        admitted_policy_frame: Some(admitted_policy_frame),
        effect,
        effect_frame,
        second_start,
    })
}

/// Recognizes exact descriptive metadata of this authored profile. The original
/// phone and every feature remain in the lowered owner; no feature is erased or
/// interpreted as measured anatomy. Nonmatching states/values/IDs refuse.
pub(crate) fn admit_reviewed_features(
    phone: &SpeechPhone,
    executions: &mut Vec<crate::SpeechCommonAcousticExecution>,
) -> Result<(), SpeechGestureRefusal> {
    let request = SpeechGreetingReviewedFeatureRequest::new(
        phone.features().get().clone(),
        phone.ipa().clone(),
    )?;
    if !boolean(FEATURES, request, executions)? {
        return Err(SpeechGestureRefusal::UnsupportedFeatures);
    }
    Ok(())
}
