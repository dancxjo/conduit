//! Traverse only the exact original G1 receipts; boundaries stop neighbor links.
use super::*;
use crate::utterance_timing::EventTimingReceipt;
use conduit_audio::AudioExactTimeOffset;
use source::{execute, Program};
fn duration(
    event: &SpeechUtteranceIntentEvent,
    index: usize,
) -> Result<SpeechExactDuration, PhoneGestureRefusal> {
    let specification = match event {
        SpeechUtteranceIntentEvent::Segment(s) => s.prosody().duration(),
        SpeechUtteranceIntentEvent::Boundary(b) => b.duration(),
    };
    let SpeechDurationSpecification::Known(d) = specification else {
        return Err(PhoneGestureRefusal::InvalidDuration { event: index });
    };
    SpeechExactDuration::new(*d.denominator(), *d.numerator_seconds())
        .map_err(|_| PhoneGestureRefusal::InvalidDuration { event: index })
}
pub fn prepare_phone_gestures<'a, 'intent>(
    original: &'a PreparedContextualIntent<'intent>,
    profile: &'a PhoneGestureProfile<'a, 'intent>,
    anchor: &AudioTrajectoryAnchor,
    provenance: &AudioTrajectoryProvenance,
    capacity: PhoneGestureCapacity,
) -> Result<PreparedPhoneGestures<'a, 'intent>, PhoneGestureRefusal> {
    if !core::ptr::eq(original, profile.owner) {
        return Err(PhoneGestureRefusal::ForeignOwner);
    }
    let events = original.source().events().as_slice();
    if events.is_empty()
        || events.len() > 256
        || capacity.events < events.len()
        || capacity.events > 256
        || capacity.trajectories
            < original
                .phones()
                .len()
                .checked_mul(5)
                .ok_or(PhoneGestureRefusal::Capacity)?
        || capacity.trajectories > 1280
        || capacity.source_executions
            < events
                .len()
                .checked_mul(7)
                .ok_or(PhoneGestureRefusal::Capacity)?
        || capacity.source_executions > 1792
    {
        return Err(PhoneGestureRefusal::Capacity);
    }
    if original.timing().receipts().len() != events.len() {
        return Err(PhoneGestureRefusal::ForeignOwner);
    }
    let durations: Vec<_> = events
        .iter()
        .enumerate()
        .map(|(n, e)| duration(e, n))
        .collect::<Result<_, _>>()?;
    let denominator = *durations[0].denominator();
    let mut start =
        AudioExactTimeOffset::new(denominator, 0).map_err(PhoneGestureRefusal::Native)?;
    let mut executions = Vec::with_capacity(capacity.source_executions);
    let mut phones = Vec::with_capacity(original.phones().len());
    let mut segment = 0;
    for (event, (value, receipt)) in events.iter().zip(original.timing().receipts()).enumerate() {
        let d = &durations[event];
        match (value, receipt) {
            (
                SpeechUtteranceIntentEvent::Boundary(b),
                EventTimingReceipt::Boundary { intent, .. },
            ) if core::ptr::eq(b, *intent) => {}
            (
                SpeechUtteranceIntentEvent::Segment(s),
                EventTimingReceipt::Segment { intent, .. },
            ) if core::ptr::eq(s, *intent) => {
                let selected = original
                    .phones()
                    .get(segment)
                    .ok_or(PhoneGestureRefusal::ForeignOwner)?;
                if !core::ptr::eq(selected.choice().occurrence().segment(), s)
                    || selected.choice().occurrence().segment().occurrence() != s.occurrence()
                {
                    return Err(PhoneGestureRefusal::ForeignOwner);
                }
                let previous_segment = event
                    .checked_sub(1)
                    .filter(|n| matches!(events[*n], SpeechUtteranceIntentEvent::Segment(_)))
                    .map(|_| segment - 1);
                let next_segment = (event + 1 < events.len()
                    && matches!(events[event + 1], SpeechUtteranceIntentEvent::Segment(_)))
                .then_some(segment + 1);
                let timing = SpeechPhoneGestureTiming::new(
                    d.clone(),
                    next_segment.map(|_| durations[event + 1].clone()),
                    previous_segment.map(|_| durations[event - 1].clone()),
                    start.clone(),
                )
                .map_err(|_| PhoneGestureRefusal::InvalidDuration { event })?;
                let phases = source::phases(timing.clone(), &mut executions)?;
                let current = profile.control(segment)?;
                let previous = profile.control(previous_segment.unwrap_or(segment))?;
                let next = profile.control(next_segment.unwrap_or(segment))?;
                let curve=|left:AudioTrajectoryQuantity,now:AudioTrajectoryQuantity,right:AudioTrajectoryQuantity,receipts:&mut Vec<GestureSourceExecution>|->Result<PreparedAudioQuantityTrajectory,PhoneGestureRefusal> {
                    let request=SpeechPhoneGestureCurve::new(anchor.clone(),now,right,phases.clone(),left,provenance.clone()).map_err(PhoneGestureRefusal::Native)?;
                    admit_audio(execute(Program::Curve,request,receipts)?)
                };
                let resonance = [
                    curve(
                        previous.resonance[0].clone(),
                        current.resonance[0].clone(),
                        next.resonance[0].clone(),
                        &mut executions,
                    )?,
                    curve(
                        previous.resonance[1].clone(),
                        current.resonance[1].clone(),
                        next.resonance[1].clone(),
                        &mut executions,
                    )?,
                    curve(
                        previous.resonance[2].clone(),
                        current.resonance[2].clone(),
                        next.resonance[2].clone(),
                        &mut executions,
                    )?,
                ];
                let energy = curve(
                    previous.energy.clone(),
                    current.energy.clone(),
                    next.energy.clone(),
                    &mut executions,
                )?;
                let energy_request = SpeechPhoneGestureEnergy::new(
                    anchor.clone(),
                    current.aspiration.clone(),
                    current.closure.clone(),
                    phases.clone(),
                    provenance.clone(),
                    current.release.clone(),
                )
                .map_err(PhoneGestureRefusal::Native)?;
                let phase_source: conduit_audio::AudioQuantityTrajectory =
                    execute(Program::Energy, energy_request, &mut executions)?;
                let phases_energy = admit_audio(phase_source.clone())?;
                phones.push(PhoneGestures {
                    event,
                    segment,
                    previous_segment,
                    next_segment,
                    timing,
                    phases,
                    resonance,
                    energy,
                    phases_energy,
                    phase_source,
                });
                segment += 1;
            }
            _ => return Err(PhoneGestureRefusal::ForeignOwner),
        }
        let advance = SpeechPhoneGestureAdvance::new(d.clone(), start)
            .map_err(|_| PhoneGestureRefusal::InvalidDuration { event })?;
        let raw: SpeechPhoneGestureRawTime = execute(Program::Advance, advance, &mut executions)?;
        start = AudioExactTimeOffset::new(*raw.denominator(), *raw.numerator())
            .map_err(PhoneGestureRefusal::Native)?;
    }
    if segment != original.phones().len() {
        return Err(PhoneGestureRefusal::ForeignOwner);
    }
    Ok(PreparedPhoneGestures {
        original,
        profile,
        anchor: anchor.clone(),
        phones,
        executions,
    })
}
