//! Finite host-side structured value transformation for rhythm comparison.

#[cfg(test)]
use conduit_audio::TimingFeedback;
use conduit_audio::{
    BeatReference, Gate, MusicalNoteEvent, RhythmRecoveryState, TimingClassification,
};
#[cfg(test)]
use conduit_core::StructuredInfoValue;
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
#[cfg(test)]
use conduit_plot::rust_binding::NativeRustBinding;
use std::collections::VecDeque;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum RhythmCompareRefusal {
    MalformedPerformance = 1,
    MalformedReference = 2,
    CapacityExhausted = 3,
    DeltaOverflow = 4,
    MalformedFeedback = 5,
    WrongBack = 6,
}

pub(super) struct RhythmCompareHost {
    target_offset_micros: i64,
    tolerance_micros: u64,
    beats: VecDeque<BeatReference>,
    performance: VecDeque<u64>,
    performance_closed: bool,
    previous_absolute_delta: Option<u64>,
    beat_type_prefix: Vec<u8>,
    feedback_type_prefix: Vec<u8>,
    output: Vec<u8>,
}

impl RhythmCompareHost {
    pub(super) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let (target_offset_micros, tolerance_micros) =
            super::rhythm_compare_back::validate(placement)?;
        let capacity = usize::from(conduit_semantic_catalog::RHYTHM_MAXIMUM_PENDING_BEATS);
        Ok(Self {
            target_offset_micros,
            tolerance_micros,
            beats: VecDeque::with_capacity(capacity),
            performance: VecDeque::with_capacity(capacity),
            performance_closed: false,
            previous_absolute_delta: None,
            beat_type_prefix: conduit_semantic_catalog::beat_reference_type()
                .canonical_bytes()
                .map_err(|error| format!("beat type encoding: {error:?}"))?,
            feedback_type_prefix: conduit_semantic_catalog::timing_feedback_type()
                .canonical_bytes()
                .map_err(|error| format!("feedback type encoding: {error:?}"))?,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<&[u8]>, RhythmCompareRefusal> {
        match contract {
            conduit_std_offers::RHYTHM_PERFORMANCE_HOST_CALL => {
                let note = MusicalNoteEvent::decode(input)
                    .map_err(|_| RhythmCompareRefusal::MalformedPerformance)?;
                if note.gate() == Gate::On {
                    self.push_performance(note.event_time_micros())?;
                }
            }
            conduit_std_offers::RHYTHM_REFERENCE_HOST_CALL => {
                let beat = decode_beat(input, &self.beat_type_prefix)?;
                self.push_beat(beat)?;
            }
            conduit_std_offers::RHYTHM_DRAIN_HOST_CALL => {
                self.performance_closed = true;
            }
            _ => return Err(RhythmCompareRefusal::WrongBack),
        }
        self.next_feedback()
    }

    fn push_performance(&mut self, event_time_micros: u64) -> Result<(), RhythmCompareRefusal> {
        if self.performance_closed
            || self.performance.len()
                == usize::from(conduit_semantic_catalog::RHYTHM_MAXIMUM_PENDING_BEATS)
        {
            return Err(RhythmCompareRefusal::CapacityExhausted);
        }
        self.performance.push_back(event_time_micros);
        Ok(())
    }

    fn push_beat(&mut self, beat: BeatReference) -> Result<(), RhythmCompareRefusal> {
        if self.beats.len() == usize::from(conduit_semantic_catalog::RHYTHM_MAXIMUM_PENDING_BEATS) {
            return Err(RhythmCompareRefusal::CapacityExhausted);
        }
        self.beats.push_back(beat);
        Ok(())
    }

    fn next_feedback(&mut self) -> Result<Option<&[u8]>, RhythmCompareRefusal> {
        let pair = if !self.beats.is_empty() && !self.performance.is_empty() {
            Some((
                self.beats.pop_front().expect("checked beat queue"),
                self.performance.pop_front(),
            ))
        } else if self.performance_closed {
            self.beats.pop_front().map(|beat| (beat, None))
        } else {
            None
        };
        let Some((beat, observed)) = pair else {
            return Ok(None);
        };
        encode_feedback_into(
            &mut self.output,
            &self.feedback_type_prefix,
            beat,
            observed,
            self.target_offset_micros,
            self.tolerance_micros,
            &mut self.previous_absolute_delta,
        )?;
        Ok(Some(&self.output))
    }
}

fn encode_feedback_into(
    output: &mut Vec<u8>,
    type_prefix: &[u8],
    beat: BeatReference,
    observed: Option<u64>,
    target_offset_micros: i64,
    tolerance_micros: u64,
    previous_absolute_delta: &mut Option<u64>,
) -> Result<(), RhythmCompareRefusal> {
    let (delta, classification, recovery_state) = classification(
        beat,
        observed,
        target_offset_micros,
        tolerance_micros,
        previous_absolute_delta,
    )?;
    // The generated binding remains the semantic owner. This host adapter writes
    // its canonical value directly into admitted storage so play does not allocate;
    // the host tests decode every emitted case through `TimingFeedback`.
    output.clear();
    output.extend_from_slice(type_prefix);
    output.push(2);
    output.extend_from_slice(&7_u32.to_le_bytes());
    field_u64(output, "beat", beat.beat());
    field_unit_variant(
        output,
        "classification",
        timing_classification_name(classification),
    );
    field_i64(output, "delta_micros", delta);
    field_u64(output, "expected_time_micros", beat.expected_time_micros());
    field_bool(output, "observed", observed.is_some());
    field_u64(output, "observed_time_micros", observed.unwrap_or(0));
    field_unit_variant(
        output,
        "recovery_state",
        rhythm_recovery_state_name(recovery_state),
    );
    (output.len() <= MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .then_some(())
        .ok_or(RhythmCompareRefusal::MalformedFeedback)
}

fn classification(
    beat: BeatReference,
    observed: Option<u64>,
    target_offset_micros: i64,
    tolerance_micros: u64,
    previous_absolute_delta: &mut Option<u64>,
) -> Result<(i64, TimingClassification, RhythmRecoveryState), RhythmCompareRefusal> {
    let Some(observed) = observed else {
        return Ok((
            0,
            TimingClassification::Missed,
            RhythmRecoveryState::Interrupted,
        ));
    };
    let delta = i128::from(observed)
        - i128::from(beat.expected_time_micros())
        - i128::from(target_offset_micros);
    let delta = i64::try_from(delta).map_err(|_| RhythmCompareRefusal::DeltaOverflow)?;
    let absolute = delta.unsigned_abs();
    let classification = if absolute <= tolerance_micros {
        TimingClassification::OnTime
    } else if delta < 0 {
        TimingClassification::Early
    } else {
        TimingClassification::Late
    };
    let recovery = if absolute <= tolerance_micros {
        if previous_absolute_delta.is_some_and(|prior| prior > tolerance_micros) {
            RhythmRecoveryState::Recovered
        } else {
            RhythmRecoveryState::OnBeat
        }
    } else if previous_absolute_delta.is_some_and(|prior| absolute < prior) {
        RhythmRecoveryState::Recovering
    } else {
        RhythmRecoveryState::Displaced
    };
    *previous_absolute_delta = Some(absolute);
    Ok((delta, classification, recovery))
}

const fn timing_classification_name(value: TimingClassification) -> &'static str {
    match value {
        TimingClassification::OnTime => "on-time",
        TimingClassification::Early => "early",
        TimingClassification::Late => "late",
        TimingClassification::Missed => "missed",
    }
}

const fn rhythm_recovery_state_name(value: RhythmRecoveryState) -> &'static str {
    match value {
        RhythmRecoveryState::Interrupted => "interrupted",
        RhythmRecoveryState::Recovered => "recovered",
        RhythmRecoveryState::OnBeat => "on-beat",
        RhythmRecoveryState::Recovering => "recovering",
        RhythmRecoveryState::Displaced => "displaced",
    }
}

fn field_unit_variant(output: &mut Vec<u8>, name: &str, tag: &str) {
    bytes(output, name.as_bytes());
    output.push(3);
    bytes(output, tag.as_bytes());
    output.push(0);
    bytes(output, &[]);
}

fn field_u64(output: &mut Vec<u8>, name: &str, value: u64) {
    bytes(output, name.as_bytes());
    output.push(0);
    bytes(output, &value.to_le_bytes());
}

fn field_bool(output: &mut Vec<u8>, name: &str, value: bool) {
    bytes(output, name.as_bytes());
    output.push(0);
    bytes(output, &conduit_core::InfoBool::new(value).encode());
}

fn field_i64(output: &mut Vec<u8>, name: &str, value: i64) {
    bytes(output, name.as_bytes());
    output.push(0);
    bytes(output, &value.to_le_bytes());
}

fn bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

#[cfg(test)]
fn feedback(
    beat: BeatReference,
    observed: Option<u64>,
    target_offset_micros: i64,
    tolerance_micros: u64,
    previous_absolute_delta: &mut Option<u64>,
) -> Result<StructuredInfoValue, RhythmCompareRefusal> {
    let (delta, classification, recovery_state) = classification(
        beat,
        observed,
        target_offset_micros,
        tolerance_micros,
        previous_absolute_delta,
    )?;
    TimingFeedback::new(
        beat.beat(),
        classification,
        delta,
        beat.expected_time_micros(),
        observed.is_some(),
        observed.unwrap_or(0),
        recovery_state,
    )
    .and_then(NativeRustBinding::into_structured)
    .map_err(|_| RhythmCompareRefusal::MalformedFeedback)
}

#[cfg(test)]
pub(crate) fn expected_feedback(
    beat: u64,
    expected_time_micros: u64,
    observed: Option<u64>,
    target_offset_micros: i64,
    tolerance_micros: u64,
) -> StructuredInfoValue {
    feedback(
        BeatReference::new(beat, expected_time_micros).unwrap(),
        observed,
        target_offset_micros,
        tolerance_micros,
        &mut None,
    )
    .unwrap()
}

fn decode_beat(bytes: &[u8], type_prefix: &[u8]) -> Result<BeatReference, RhythmCompareRefusal> {
    let mut bytes = bytes
        .strip_prefix(type_prefix)
        .ok_or(RhythmCompareRefusal::MalformedReference)?;
    if take_byte(&mut bytes)? != 2 || take_u32(&mut bytes)? != 2 {
        return Err(RhythmCompareRefusal::MalformedReference);
    }
    let beat = take_named_u64(&mut bytes, "beat")?;
    let expected_time_micros = take_named_u64(&mut bytes, "expected_time_micros")?;
    if !bytes.is_empty() {
        return Err(RhythmCompareRefusal::MalformedReference);
    }
    BeatReference::new(beat, expected_time_micros)
        .map_err(|_| RhythmCompareRefusal::MalformedReference)
}

fn take_named_u64(bytes: &mut &[u8], name: &str) -> Result<u64, RhythmCompareRefusal> {
    if take_bytes(bytes)? != name.as_bytes() || take_byte(bytes)? != 0 {
        return Err(RhythmCompareRefusal::MalformedReference);
    }
    let raw: [u8; 8] = take_bytes(bytes)?
        .try_into()
        .map_err(|_| RhythmCompareRefusal::MalformedReference)?;
    Ok(u64::from_le_bytes(raw))
}

fn take_byte(bytes: &mut &[u8]) -> Result<u8, RhythmCompareRefusal> {
    let (&value, rest) = bytes
        .split_first()
        .ok_or(RhythmCompareRefusal::MalformedReference)?;
    *bytes = rest;
    Ok(value)
}

fn take_u32(bytes: &mut &[u8]) -> Result<u32, RhythmCompareRefusal> {
    let raw: [u8; 4] = bytes
        .get(..4)
        .ok_or(RhythmCompareRefusal::MalformedReference)?
        .try_into()
        .map_err(|_| RhythmCompareRefusal::MalformedReference)?;
    *bytes = &bytes[4..];
    Ok(u32::from_le_bytes(raw))
}

fn take_bytes<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], RhythmCompareRefusal> {
    let length =
        usize::try_from(take_u32(bytes)?).map_err(|_| RhythmCompareRefusal::MalformedReference)?;
    let value = bytes
        .get(..length)
        .ok_or(RhythmCompareRefusal::MalformedReference)?;
    *bytes = &bytes[length..];
    Ok(value)
}

#[cfg(test)]
#[path = "rhythm_compare_host_tests.rs"]
mod tests;
