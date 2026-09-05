//! Portable deterministic normalization for finite duration sequences.

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

pub const NORMALIZED_SEQUENCE_TYPE: &str = "NormalizedDurationSequence";
pub const NORMALIZE_SEQUENCE_KIND: &str = "sequence/normalize-relative-duration";
pub const NORMALIZE_SEQUENCE_REVISION: &str = "conduit.std/normalize-relative-duration@1";
pub const NORMALIZATION_ALGORITHM: &str = "maximum-relative-millionths-half-up@1";
pub const NORMALIZATION_ALGORITHM_INFO_ID: &str = "sequence/normalization-algorithm@1";
pub const NORMALIZED_VALUES_INFO_ID: &str = "sequence/relative-millionth-sequence@1";
pub const NORMALIZED_SCALE: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceNormalizationRefusal {
    Malformed,
    Empty,
    TooManyValues,
    ZeroDuration,
}

pub fn normalized_duration_sequence_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("sequence/normalized-duration-sequence@1"),
        vec![
            StructuredFieldType::new(
                "algorithm",
                StructuredInfoType::leaf(kind_id(NORMALIZATION_ALGORITHM_INFO_ID)).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                "values",
                StructuredInfoType::leaf(kind_id(NORMALIZED_VALUES_INFO_ID)).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

pub fn normalize_relative_duration_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(NORMALIZE_SEQUENCE_KIND),
        kind_contract_revision: KindContractRevision::from(NORMALIZE_SEQUENCE_REVISION),
        inputs: vec![value_port(
            "intervals",
            &crate::interval_sequence_type(),
            PortDirection::Input,
        )],
        outputs: vec![value_port(
            "normalized",
            &normalized_duration_sequence_type(),
            PortDirection::Output,
        )],
        configuration: Vec::new(),
    }
}

pub fn install_sequence_normalization_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_structured_type(
            NORMALIZED_SEQUENCE_TYPE,
            normalized_duration_sequence_type(),
        )
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: NORMALIZE_SEQUENCE_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(normalize_relative_duration_definition())
        .map_err(|error| error.to_string())
}

pub fn normalize_relative_durations(
    intervals: &StructuredInfoValue,
) -> Result<StructuredInfoValue, SequenceNormalizationRefusal> {
    if intervals.value_type() != &crate::interval_sequence_type() {
        return Err(SequenceNormalizationRefusal::Malformed);
    }
    let fields = match intervals.shape() {
        StructuredInfoValueShape::Record(fields) => fields,
        _ => return Err(SequenceNormalizationRefusal::Malformed),
    };
    let encoded = leaf(field(fields, "intervals")?)?;
    let values = parse_values(encoded)?;
    let maximum = values
        .iter()
        .copied()
        .max()
        .ok_or(SequenceNormalizationRefusal::Empty)?;
    let normalized = values
        .iter()
        .map(|value| {
            let numerator = u128::from(*value) * u128::from(NORMALIZED_SCALE);
            ((numerator + u128::from(maximum / 2)) / u128::from(maximum)) as u64
        })
        .collect::<Vec<_>>();
    normalized_value(&normalized)
}

pub fn normalized_value(
    values: &[u64],
) -> Result<StructuredInfoValue, SequenceNormalizationRefusal> {
    if values.is_empty() {
        return Err(SequenceNormalizationRefusal::Empty);
    }
    if values.len() >= crate::MAXIMUM_TIMED_EVENTS {
        return Err(SequenceNormalizationRefusal::TooManyValues);
    }
    let encoded = values
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",")
        .into_bytes();
    StructuredInfoValue::record(
        normalized_duration_sequence_type(),
        vec![
            StructuredFieldValue::new(
                "algorithm",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id(NORMALIZATION_ALGORITHM_INFO_ID)).unwrap(),
                    NORMALIZATION_ALGORITHM.as_bytes().to_vec(),
                )
                .map_err(|_| SequenceNormalizationRefusal::Malformed)?,
            )
            .map_err(|_| SequenceNormalizationRefusal::Malformed)?,
            StructuredFieldValue::new(
                "values",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id(NORMALIZED_VALUES_INFO_ID)).unwrap(),
                    encoded,
                )
                .map_err(|_| SequenceNormalizationRefusal::Malformed)?,
            )
            .map_err(|_| SequenceNormalizationRefusal::Malformed)?,
        ],
    )
    .map_err(|_| SequenceNormalizationRefusal::Malformed)
}

fn parse_values(bytes: &[u8]) -> Result<Vec<u64>, SequenceNormalizationRefusal> {
    let text = core::str::from_utf8(bytes).map_err(|_| SequenceNormalizationRefusal::Malformed)?;
    if text.is_empty() {
        return Err(SequenceNormalizationRefusal::Empty);
    }
    let values = text
        .split(',')
        .map(|value| {
            value
                .parse()
                .map_err(|_| SequenceNormalizationRefusal::Malformed)
        })
        .collect::<Result<Vec<u64>, _>>()?;
    if values.len() >= crate::MAXIMUM_TIMED_EVENTS {
        return Err(SequenceNormalizationRefusal::TooManyValues);
    }
    if values.contains(&0) {
        return Err(SequenceNormalizationRefusal::ZeroDuration);
    }
    Ok(values)
}

fn field<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, SequenceNormalizationRefusal> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(SequenceNormalizationRefusal::Malformed)
}

fn leaf(value: &StructuredInfoValue) -> Result<&[u8], SequenceNormalizationRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => Ok(bytes),
        _ => Err(SequenceNormalizationRefusal::Malformed),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_is_scale_independent_ordered_and_inspectable() {
        let first = crate::timed_event_sequence_value("fixture/us", &[0, 2, 8, 12]).unwrap();
        let second = crate::timed_event_sequence_value("fixture/ms", &[10, 30, 90, 130]).unwrap();
        let first =
            normalize_relative_durations(&crate::derive_intervals(&first).unwrap()).unwrap();
        let second =
            normalize_relative_durations(&crate::derive_intervals(&second).unwrap()).unwrap();
        assert_eq!(first, second);
        let fields = match first.shape() {
            StructuredInfoValueShape::Record(fields) => fields,
            _ => unreachable!(),
        };
        assert_eq!(
            leaf(field(fields, "algorithm").unwrap()).unwrap(),
            NORMALIZATION_ALGORITHM.as_bytes()
        );
        assert_eq!(
            leaf(field(fields, "values").unwrap()).unwrap(),
            b"333333,1000000,666667"
        );
    }

    #[test]
    fn empty_zero_and_excess_sequences_remain_distinct() {
        assert_eq!(
            normalized_value(&[]),
            Err(SequenceNormalizationRefusal::Empty)
        );
        assert_eq!(
            parse_values(b"1,0"),
            Err(SequenceNormalizationRefusal::ZeroDuration)
        );
        let excess = vec![1; crate::MAXIMUM_TIMED_EVENTS];
        assert_eq!(
            normalized_value(&excess),
            Err(SequenceNormalizationRefusal::TooManyValues)
        );
    }
}
