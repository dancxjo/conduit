//! Portable bounded comparison of normalized finite patterns.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const PATTERN_COMPARISON_TYPE: &str = "PatternComparison";
pub const COMPARE_PATTERN_KIND: &str = "sequence/compare-normalized-pattern";
pub const COMPARE_PATTERN_REVISION: &str = "conduit.std/compare-normalized-pattern@1";
pub const MAXIMUM_ABSOLUTE_METRIC: &str = "maximum-absolute-millionths@1";
pub const DEFAULT_PATTERN_TOLERANCE: u64 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternComparisonRefusal {
    Malformed,
    UnsupportedMetric,
    ToleranceOutOfRange,
    AlgorithmMismatch,
    LengthMismatch,
}

pub fn pattern_comparison_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("sequence/pattern-comparison@1"),
        vec![
            field_type("matched", "value/boolean@1"),
            field_type("metric", "sequence/comparison-metric@1"),
            field_type("score_millionths", "value/count@1"),
            field_type("tolerance_millionths", "value/count@1"),
        ],
    )
    .unwrap()
}

pub fn compare_normalized_pattern_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(COMPARE_PATTERN_KIND),
        kind_contract_revision: KindContractRevision::from(COMPARE_PATTERN_REVISION),
        inputs: vec![
            value_port("candidate", PortDirection::Input),
            value_port("template", PortDirection::Input),
        ],
        outputs: vec![PortDescriptor {
            port_id: port_id("comparison"),
            value_kind: pattern_comparison_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: vec![
            ConfigurationField {
                key: "metric".into(),
                default_value: ConfigurationValue::Text(MAXIMUM_ABSOLUTE_METRIC.into()),
                validation: ConfigurationRule::TextOneOf {
                    values: vec![MAXIMUM_ABSOLUTE_METRIC.into()],
                },
            },
            ConfigurationField {
                key: "tolerance-millionths".into(),
                default_value: ConfigurationValue::U64(DEFAULT_PATTERN_TOLERANCE),
                validation: ConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: crate::NORMALIZED_SCALE,
                },
            },
        ],
    }
}

pub fn install_pattern_comparison_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_structured_type(PATTERN_COMPARISON_TYPE, pattern_comparison_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: COMPARE_PATTERN_KIND.into(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "metric".into(),
                    value_type: "Text".into(),
                    default: Some(MAXIMUM_ABSOLUTE_METRIC.into()),
                },
                StartupParameterSignature {
                    name: "tolerance-millionths".into(),
                    value_type: "Count".into(),
                    default: Some(DEFAULT_PATTERN_TOLERANCE.to_string()),
                },
            ],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(compare_normalized_pattern_definition())
        .map_err(|error| error.to_string())
}

pub fn compare_normalized_patterns(
    candidate: &StructuredInfoValue,
    template: &StructuredInfoValue,
    metric: &str,
    tolerance: u64,
) -> Result<StructuredInfoValue, PatternComparisonRefusal> {
    if metric != MAXIMUM_ABSOLUTE_METRIC {
        return Err(PatternComparisonRefusal::UnsupportedMetric);
    }
    if tolerance > crate::NORMALIZED_SCALE {
        return Err(PatternComparisonRefusal::ToleranceOutOfRange);
    }
    let (candidate_algorithm, candidate) = decode(candidate)?;
    let (template_algorithm, template) = decode(template)?;
    if candidate_algorithm != crate::NORMALIZATION_ALGORITHM.as_bytes()
        || template_algorithm != crate::NORMALIZATION_ALGORITHM.as_bytes()
        || candidate_algorithm != template_algorithm
    {
        return Err(PatternComparisonRefusal::AlgorithmMismatch);
    }
    if candidate.len() != template.len() {
        return Err(PatternComparisonRefusal::LengthMismatch);
    }
    let maximum_error = candidate
        .iter()
        .zip(&template)
        .map(|(candidate, template)| candidate.abs_diff(*template))
        .max()
        .unwrap_or(0);
    comparison_value(
        metric,
        tolerance,
        crate::NORMALIZED_SCALE.saturating_sub(maximum_error),
        maximum_error <= tolerance,
    )
}

fn decode(value: &StructuredInfoValue) -> Result<(&[u8], Vec<u64>), PatternComparisonRefusal> {
    if value.value_type() != &crate::normalized_duration_sequence_type() {
        return Err(PatternComparisonRefusal::Malformed);
    }
    let fields = match value.shape() {
        StructuredInfoValueShape::Record(fields) => fields,
        _ => return Err(PatternComparisonRefusal::Malformed),
    };
    let algorithm = leaf(field(fields, "algorithm")?)?;
    let values = core::str::from_utf8(leaf(field(fields, "values")?)?)
        .map_err(|_| PatternComparisonRefusal::Malformed)?
        .split(',')
        .map(|value| {
            value
                .parse()
                .map_err(|_| PatternComparisonRefusal::Malformed)
        })
        .collect::<Result<Vec<u64>, _>>()?;
    if values.is_empty() || values.len() >= crate::MAXIMUM_TIMED_EVENTS {
        return Err(PatternComparisonRefusal::Malformed);
    }
    Ok((algorithm, values))
}

fn comparison_value(
    metric: &str,
    tolerance: u64,
    score: u64,
    matched: bool,
) -> Result<StructuredInfoValue, PatternComparisonRefusal> {
    StructuredInfoValue::record(
        pattern_comparison_type(),
        vec![
            value_field(
                "matched",
                "value/boolean@1",
                if matched { "true" } else { "false" },
            )?,
            value_field("metric", "sequence/comparison-metric@1", metric)?,
            value_field("score_millionths", "value/count@1", &score.to_string())?,
            value_field(
                "tolerance_millionths",
                "value/count@1",
                &tolerance.to_string(),
            )?,
        ],
    )
    .map_err(|_| PatternComparisonRefusal::Malformed)
}

fn field_type(name: &str, kind: &str) -> StructuredFieldType {
    StructuredFieldType::new(name, StructuredInfoType::leaf(kind_id(kind)).unwrap()).unwrap()
}

fn value_port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: crate::normalized_duration_sequence_type()
            .profile()
            .unwrap()
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn value_field(
    name: &str,
    kind: &str,
    value: &str,
) -> Result<StructuredFieldValue, PatternComparisonRefusal> {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(kind)).unwrap(),
            value.as_bytes().to_vec(),
        )
        .map_err(|_| PatternComparisonRefusal::Malformed)?,
    )
    .map_err(|_| PatternComparisonRefusal::Malformed)
}

fn field<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, PatternComparisonRefusal> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(PatternComparisonRefusal::Malformed)
}

fn leaf(value: &StructuredInfoValue) -> Result<&[u8], PatternComparisonRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(value) => Ok(value),
        _ => Err(PatternComparisonRefusal::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_tolerance_controls_match_and_score() {
        let candidate = crate::normalized_value(&[500_000, 1_000_000, 760_000]).unwrap();
        let template = crate::normalized_value(&[500_000, 1_000_000, 700_000]).unwrap();
        assert_eq!(
            compare_normalized_patterns(&candidate, &template, MAXIMUM_ABSOLUTE_METRIC, 59_999)
                .unwrap(),
            comparison_value(MAXIMUM_ABSOLUTE_METRIC, 59_999, 940_000, false).unwrap()
        );
        assert_eq!(
            compare_normalized_patterns(&candidate, &template, MAXIMUM_ABSOLUTE_METRIC, 60_000)
                .unwrap(),
            comparison_value(MAXIMUM_ABSOLUTE_METRIC, 60_000, 940_000, true).unwrap()
        );
    }

    #[test]
    fn unsupported_metric_length_and_tolerance_refuse_distinctly() {
        let one = crate::normalized_value(&[1]).unwrap();
        let two = crate::normalized_value(&[1, 2]).unwrap();
        assert_eq!(
            compare_normalized_patterns(&one, &one, "hidden", 0),
            Err(PatternComparisonRefusal::UnsupportedMetric)
        );
        assert_eq!(
            compare_normalized_patterns(&one, &one, MAXIMUM_ABSOLUTE_METRIC, 1_000_001),
            Err(PatternComparisonRefusal::ToleranceOutOfRange)
        );
        assert_eq!(
            compare_normalized_patterns(&one, &two, MAXIMUM_ABSOLUTE_METRIC, 0),
            Err(PatternComparisonRefusal::LengthMismatch)
        );
    }
}
