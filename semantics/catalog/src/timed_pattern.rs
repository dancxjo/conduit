//! Portable bounded values and checked contracts for finite timed patterns.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredFieldType, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_form::{KindDefinition, KindSignature};

pub const TIMED_EVENT_SEQUENCE_TYPE: &str = "TimedEventSequence";
pub const INTERVAL_SEQUENCE_TYPE: &str = "IntervalSequence";
pub const ORDERED_EVENT_INTERVALS_KIND: &str = "time/ordered-event-intervals";
pub const ORDERED_EVENT_INTERVALS_REVISION: &str = "conduit.std/ordered-event-intervals@1";
pub const CLOCK_BASIS_INFO_ID: &str = "time/clock-basis@1";
pub const EVENT_TIMES_INFO_ID: &str = "time/ordered-microsecond-sequence@1";
pub const INTERVALS_INFO_ID: &str = "time/microsecond-interval-sequence@1";
pub const MAXIMUM_TIMED_EVENTS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimedPatternRefusal {
    Malformed,
    TooFewEvents,
    TooManyEvents,
    ReorderedOrDuplicateEvent,
    IntervalOverflow,
}

impl core::fmt::Display for TimedPatternRefusal {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Malformed => "timed sequence is malformed",
            Self::TooFewEvents => "timed sequence requires at least two events",
            Self::TooManyEvents => "timed sequence exceeds its event bound",
            Self::ReorderedOrDuplicateEvent => "timed sequence events are not strictly ordered",
            Self::IntervalOverflow => "timed sequence interval is not representable",
        })
    }
}

pub fn timed_event_sequence_type() -> StructuredInfoType {
    sequence_record_type(
        "time/timed-event-sequence@1",
        "event_times",
        EVENT_TIMES_INFO_ID,
    )
}

pub fn interval_sequence_type() -> StructuredInfoType {
    sequence_record_type("time/interval-sequence@1", "intervals", INTERVALS_INFO_ID)
}

pub fn ordered_event_intervals_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(ORDERED_EVENT_INTERVALS_KIND),
        kind_contract_revision: KindContractRevision::from(ORDERED_EVENT_INTERVALS_REVISION),
        inputs: vec![value_port(
            "events",
            &timed_event_sequence_type(),
            PortDirection::Input,
        )],
        outputs: vec![value_port(
            "intervals",
            &interval_sequence_type(),
            PortDirection::Output,
        )],
        configuration: Vec::new(),
    }
}

pub fn install_timed_pattern_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_structured_type(TIMED_EVENT_SEQUENCE_TYPE, timed_event_sequence_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type(INTERVAL_SEQUENCE_TYPE, interval_sequence_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: ORDERED_EVENT_INTERVALS_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(ordered_event_intervals_definition())
        .map_err(|error| error.to_string())
}

pub fn timed_event_sequence_value(
    clock_basis: &str,
    event_times: &[u64],
) -> Result<StructuredInfoValue, TimedPatternRefusal> {
    validate_event_times(event_times)?;
    sequence_value(
        timed_event_sequence_type(),
        "event_times",
        EVENT_TIMES_INFO_ID,
        clock_basis,
        event_times,
    )
}

pub fn derive_intervals(
    value: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TimedPatternRefusal> {
    if value.value_type() != &timed_event_sequence_type() {
        return Err(TimedPatternRefusal::Malformed);
    }
    let fields = record(value)?;
    let clock_basis = leaf(field(fields, "clock_basis")?)?;
    let times = parse_sequence(leaf(field(fields, "event_times")?)?)?;
    validate_event_times(&times)?;
    let intervals = times
        .windows(2)
        .map(|pair| {
            pair[1]
                .checked_sub(pair[0])
                .ok_or(TimedPatternRefusal::IntervalOverflow)
        })
        .collect::<Result<Vec<_>, _>>()?;
    sequence_value(
        interval_sequence_type(),
        "intervals",
        INTERVALS_INFO_ID,
        core::str::from_utf8(clock_basis).map_err(|_| TimedPatternRefusal::Malformed)?,
        &intervals,
    )
}

pub fn decode_intervals(
    value: &StructuredInfoValue,
) -> Result<(String, Vec<u64>), TimedPatternRefusal> {
    if value.value_type() != &interval_sequence_type() {
        return Err(TimedPatternRefusal::Malformed);
    }
    let fields = record(value)?;
    let basis = core::str::from_utf8(leaf(field(fields, "clock_basis")?)?)
        .map_err(|_| TimedPatternRefusal::Malformed)?
        .into();
    let intervals = parse_sequence(leaf(field(fields, "intervals")?)?)?;
    Ok((basis, intervals))
}

fn sequence_record_type(schema: &str, field_name: &str, sequence_kind: &str) -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id(schema),
        vec![
            StructuredFieldType::new(
                "clock_basis",
                StructuredInfoType::leaf(kind_id(CLOCK_BASIS_INFO_ID)).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                field_name,
                StructuredInfoType::leaf(kind_id(sequence_kind)).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn value_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn sequence_value(
    value_type: StructuredInfoType,
    sequence_field: &str,
    sequence_kind: &str,
    clock_basis: &str,
    values: &[u64],
) -> Result<StructuredInfoValue, TimedPatternRefusal> {
    if clock_basis.is_empty() {
        return Err(TimedPatternRefusal::Malformed);
    }
    StructuredInfoValue::record(
        value_type,
        vec![
            StructuredFieldValue::new(
                "clock_basis",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id(CLOCK_BASIS_INFO_ID)).unwrap(),
                    clock_basis.as_bytes().to_vec(),
                )?,
            )?,
            StructuredFieldValue::new(
                sequence_field,
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id(sequence_kind)).unwrap(),
                    encode_sequence(values).into_bytes(),
                )?,
            )?,
        ],
    )
    .map_err(|_| TimedPatternRefusal::Malformed)
}

fn validate_event_times(values: &[u64]) -> Result<(), TimedPatternRefusal> {
    if values.len() < 2 {
        return Err(TimedPatternRefusal::TooFewEvents);
    }
    if values.len() > MAXIMUM_TIMED_EVENTS {
        return Err(TimedPatternRefusal::TooManyEvents);
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(TimedPatternRefusal::ReorderedOrDuplicateEvent);
    }
    Ok(())
}

fn encode_sequence(values: &[u64]) -> String {
    values
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_sequence(bytes: &[u8]) -> Result<Vec<u64>, TimedPatternRefusal> {
    let text = core::str::from_utf8(bytes).map_err(|_| TimedPatternRefusal::Malformed)?;
    if text.is_empty() {
        return Ok(Vec::new());
    }
    text.split(',')
        .map(|part| {
            let value = part
                .parse::<u64>()
                .map_err(|_| TimedPatternRefusal::Malformed)?;
            if value.to_string() != part {
                return Err(TimedPatternRefusal::Malformed);
            }
            Ok(value)
        })
        .collect()
}

fn record(value: &StructuredInfoValue) -> Result<&[StructuredFieldValue], TimedPatternRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => Ok(fields),
        _ => Err(TimedPatternRefusal::Malformed),
    }
}

fn field<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, TimedPatternRefusal> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(TimedPatternRefusal::Malformed)
}

fn leaf(value: &StructuredInfoValue) -> Result<&[u8], TimedPatternRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => Ok(bytes),
        _ => Err(TimedPatternRefusal::Malformed),
    }
}

impl From<conduit_core::StructuredInfoRefusal> for TimedPatternRefusal {
    fn from(_: conduit_core::StructuredInfoRefusal) -> Self {
        Self::Malformed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_events_derive_exact_intervals_and_preserve_time_basis() {
        let events =
            timed_event_sequence_value("fixture/monotonic-us", &[100, 240, 610, 900]).unwrap();
        let intervals = derive_intervals(&events).unwrap();
        assert_eq!(
            decode_intervals(&intervals).unwrap(),
            ("fixture/monotonic-us".into(), vec![140, 370, 290])
        );
    }

    #[test]
    fn incomplete_reordered_duplicate_and_excess_sequences_remain_distinct() {
        assert_eq!(
            timed_event_sequence_value("fixture", &[10]),
            Err(TimedPatternRefusal::TooFewEvents)
        );
        assert_eq!(
            timed_event_sequence_value("fixture", &[10, 9]),
            Err(TimedPatternRefusal::ReorderedOrDuplicateEvent)
        );
        assert_eq!(
            timed_event_sequence_value("fixture", &[10, 10]),
            Err(TimedPatternRefusal::ReorderedOrDuplicateEvent)
        );
        assert_eq!(
            timed_event_sequence_value("fixture", &[0; MAXIMUM_TIMED_EVENTS + 1]),
            Err(TimedPatternRefusal::TooManyEvents)
        );
    }

    #[test]
    fn checked_contract_has_exact_typed_value_ports_and_no_platform_facts() {
        let definition = ordered_event_intervals_definition();
        assert_eq!(definition.kind_id.as_str(), ORDERED_EVENT_INTERVALS_KIND);
        assert_eq!(definition.inputs[0].port_id.as_str(), "events");
        assert_eq!(definition.outputs[0].port_id.as_str(), "intervals");
        assert!(definition.configuration.is_empty());
        let authored = [TIMED_EVENT_SEQUENCE_TYPE, INTERVAL_SEQUENCE_TYPE].join(" ");
        for forbidden in ["browser", "host", "device", "transport", "dom", "gpio"] {
            assert!(!authored.to_ascii_lowercase().contains(forbidden));
        }
    }
}
