//! Finite host-side structured value transformation for rhythm comparison.

use conduit_audio::{Gate, MusicalNoteEvent};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
#[cfg(test)]
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoValue, StructuredInfoValueShape,
};
use std::collections::VecDeque;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum RhythmCompareRefusal {
    MalformedPerformance = 1,
    MalformedReference = 2,
    CapacityExhausted = 3,
    DeltaOverflow = 4,
    MalformedFeedback = 5,
    WrongOperation = 6,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct BeatReference {
    beat: u64,
    expected_time_micros: u64,
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
            super::rhythm_compare_operation::validate(placement)?;
        let capacity = usize::from(conduit_std_catalog::RHYTHM_MAXIMUM_PENDING_BEATS);
        Ok(Self {
            target_offset_micros,
            tolerance_micros,
            beats: VecDeque::with_capacity(capacity),
            performance: VecDeque::with_capacity(capacity),
            performance_closed: false,
            previous_absolute_delta: None,
            beat_type_prefix: conduit_std_catalog::beat_reference_type()
                .canonical_bytes()
                .map_err(|error| format!("beat type encoding: {error:?}"))?,
            feedback_type_prefix: conduit_std_catalog::timing_feedback_type()
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
            conduit_std_offers::RHYTHM_PERFORMANCE_HOST_OPERATION => {
                let note = MusicalNoteEvent::decode(input)
                    .map_err(|_| RhythmCompareRefusal::MalformedPerformance)?;
                if note.gate == Gate::On {
                    self.push_performance(note.event_time_micros)?;
                }
            }
            conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION => {
                let beat = decode_beat(input, &self.beat_type_prefix)?;
                self.push_beat(beat)?;
            }
            conduit_std_offers::RHYTHM_DRAIN_HOST_OPERATION => {
                self.performance_closed = true;
            }
            _ => return Err(RhythmCompareRefusal::WrongOperation),
        }
        self.next_feedback()
    }

    fn push_performance(&mut self, event_time_micros: u64) -> Result<(), RhythmCompareRefusal> {
        if self.performance_closed
            || self.performance.len()
                == usize::from(conduit_std_catalog::RHYTHM_MAXIMUM_PENDING_BEATS)
        {
            return Err(RhythmCompareRefusal::CapacityExhausted);
        }
        self.performance.push_back(event_time_micros);
        Ok(())
    }

    fn push_beat(&mut self, beat: BeatReference) -> Result<(), RhythmCompareRefusal> {
        if self.beats.len() == usize::from(conduit_std_catalog::RHYTHM_MAXIMUM_PENDING_BEATS) {
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
    let (delta, classification, recovery) = classification(
        beat,
        observed,
        target_offset_micros,
        tolerance_micros,
        previous_absolute_delta,
    )?;
    output.clear();
    output.extend_from_slice(type_prefix);
    output.push(2);
    output.extend_from_slice(&7_u32.to_le_bytes());
    field_u64(output, "beat", beat.beat);
    field_text(output, "classification", classification);
    field_i64(output, "delta_micros", delta);
    field_u64(output, "expected_time_micros", beat.expected_time_micros);
    field_text(
        output,
        "observed",
        if observed.is_some() { "true" } else { "false" },
    );
    field_u64(output, "observed_time_micros", observed.unwrap_or(0));
    field_text(output, "recovery_state", recovery);
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
) -> Result<(i64, &'static str, &'static str), RhythmCompareRefusal> {
    let Some(observed) = observed else {
        return Ok((0, "missed", "interrupted"));
    };
    let delta = i128::from(observed)
        - i128::from(beat.expected_time_micros)
        - i128::from(target_offset_micros);
    let delta = i64::try_from(delta).map_err(|_| RhythmCompareRefusal::DeltaOverflow)?;
    let absolute = delta.unsigned_abs();
    let classification = if absolute <= tolerance_micros {
        "on-time"
    } else if delta < 0 {
        "early"
    } else {
        "late"
    };
    let recovery = if absolute <= tolerance_micros {
        if previous_absolute_delta.is_some_and(|prior| prior > tolerance_micros) {
            "recovered"
        } else {
            "on-beat"
        }
    } else if previous_absolute_delta.is_some_and(|prior| absolute < prior) {
        "recovering"
    } else {
        "displaced"
    };
    *previous_absolute_delta = Some(absolute);
    Ok((delta, classification, recovery))
}

fn field_text(output: &mut Vec<u8>, name: &str, value: &str) {
    bytes(output, name.as_bytes());
    output.push(0);
    bytes(output, value.as_bytes());
}

fn field_u64(output: &mut Vec<u8>, name: &str, value: u64) {
    bytes(output, name.as_bytes());
    output.push(0);
    decimal_u64(output, value);
}

fn field_i64(output: &mut Vec<u8>, name: &str, value: i64) {
    bytes(output, name.as_bytes());
    output.push(0);
    let length_at = output.len();
    output.extend_from_slice(&0_u32.to_le_bytes());
    let start = output.len();
    if value < 0 {
        output.push(b'-');
    }
    append_digits(output, value.unsigned_abs());
    let length = u32::try_from(output.len() - start).expect("signed decimal length is finite");
    output[length_at..length_at + 4].copy_from_slice(&length.to_le_bytes());
}

fn decimal_u64(output: &mut Vec<u8>, value: u64) {
    let length_at = output.len();
    output.extend_from_slice(&0_u32.to_le_bytes());
    let start = output.len();
    append_digits(output, value);
    let length = u32::try_from(output.len() - start).expect("decimal length is finite");
    output[length_at..length_at + 4].copy_from_slice(&length.to_le_bytes());
}

fn append_digits(output: &mut Vec<u8>, mut value: u64) {
    let mut digits = [0_u8; 20];
    let mut cursor = digits.len();
    loop {
        cursor -= 1;
        digits[cursor] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    output.extend_from_slice(&digits[cursor..]);
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
    let (delta, classification, recovery) = classification(
        beat,
        observed,
        target_offset_micros,
        tolerance_micros,
        previous_absolute_delta,
    )?;
    StructuredInfoValue::record(
        conduit_std_catalog::timing_feedback_type(),
        vec![
            value_field("beat", count_leaf(beat.beat)),
            value_field(
                "classification",
                text_leaf("music/timing-classification@1", classification),
            ),
            value_field(
                "delta_micros",
                text_leaf("time/signed-microseconds@1", &delta.to_string()),
            ),
            value_field(
                "expected_time_micros",
                count_leaf(beat.expected_time_micros),
            ),
            value_field(
                "observed",
                text_leaf(
                    "value/boolean@1",
                    if observed.is_some() { "true" } else { "false" },
                ),
            ),
            value_field("observed_time_micros", count_leaf(observed.unwrap_or(0))),
            value_field(
                "recovery_state",
                text_leaf("music/recovery-state@1", recovery),
            ),
        ],
    )
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
        BeatReference {
            beat,
            expected_time_micros,
        },
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
    let beat = take_named_count(&mut bytes, "beat")?;
    let expected_time_micros = take_named_count(&mut bytes, "expected_time_micros")?;
    if !bytes.is_empty() {
        return Err(RhythmCompareRefusal::MalformedReference);
    }
    Ok(BeatReference {
        beat,
        expected_time_micros,
    })
}

fn take_named_count(bytes: &mut &[u8], name: &str) -> Result<u64, RhythmCompareRefusal> {
    if take_bytes(bytes)? != name.as_bytes() || take_byte(bytes)? != 0 {
        return Err(RhythmCompareRefusal::MalformedReference);
    }
    core::str::from_utf8(take_bytes(bytes)?)
        .map_err(|_| RhythmCompareRefusal::MalformedReference)?
        .parse()
        .map_err(|_| RhythmCompareRefusal::MalformedReference)
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
fn field<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, RhythmCompareRefusal> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(RhythmCompareRefusal::MalformedReference)
}

#[cfg(test)]
fn count_leaf(value: u64) -> StructuredInfoValue {
    text_leaf("value/count@1", &value.to_string())
}

#[cfg(test)]
fn text_leaf(kind: &str, value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind)).unwrap(),
        value.as_bytes().to_vec(),
    )
    .unwrap()
}

#[cfg(test)]
fn value_field(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).unwrap()
}

#[cfg(test)]
#[path = "rhythm_compare_host_tests.rs"]
mod tests;
