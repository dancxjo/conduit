//! Compact Gear Face controls derived from authoritative checked contracts.

use conduit_core::ConfigurationValue;
use conduit_form::CheckedGear;
use conduit_std_catalog::StandardConfigurationRule;

use crate::PatchbayGraphError;

/// Finite Gear-face control bound. Sixteen covers the reviewed 14-field
/// Instrument 1 synth surface without making arbitrary catalog growth free.
pub const MAX_FACE_CONTROLS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceControlKind {
    /// A Boolean is both a native toggle and a finite two-choice value.
    BooleanChoice {
        choices: [&'static str; 2],
    },
    TextChoice {
        choices: Vec<String>,
    },
    Number {
        minimum: u64,
        maximum: u64,
        unit: Option<&'static str>,
    },
    ScalarNumber {
        minimum: i64,
        maximum: i64,
        unit: &'static str,
    },
    Range {
        minimum: u64,
        maximum: u64,
        unit: Option<&'static str>,
    },
    ShortText {
        maximum_bytes: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceControl {
    pub key: String,
    pub value: ConfigurationValue,
    pub kind: FaceControlKind,
}

pub(crate) fn project_controls(gear: &CheckedGear) -> Result<Vec<FaceControl>, PatchbayGraphError> {
    let Some(contract) = conduit_std_catalog::supported_nucleus_contracts()
        .into_iter()
        .chain(conduit_std_catalog::standard_contracts())
        .find(|contract| contract.kind_id == gear.kind_id)
    else {
        return Ok(Vec::new());
    };
    if contract.configuration.len() > MAX_FACE_CONTROLS {
        return Err(PatchbayGraphError::TooManyControls);
    }
    contract
        .configuration
        .into_iter()
        .map(|field| {
            let value = gear
                .configuration
                .iter()
                .find(|entry| entry.key == field.key)
                .ok_or(PatchbayGraphError::InvalidConfigurationContract)?
                .value
                .clone();
            let kind = match (&value, field.rule) {
                (ConfigurationValue::Bool(_), StandardConfigurationRule::Any) => {
                    FaceControlKind::BooleanChoice {
                        choices: ["false", "true"],
                    }
                }
                (
                    ConfigurationValue::U64(_),
                    StandardConfigurationRule::U64Range { minimum, maximum },
                ) => {
                    let unit = field.key.ends_with("-ms").then_some("ms");
                    if maximum.saturating_sub(minimum) <= 100 {
                        FaceControlKind::Range {
                            minimum,
                            maximum,
                            unit,
                        }
                    } else {
                        FaceControlKind::Number {
                            minimum,
                            maximum,
                            unit,
                        }
                    }
                }
                (
                    ConfigurationValue::U64(_),
                    StandardConfigurationRule::DurationMillis { minimum, maximum },
                ) => FaceControlKind::Number {
                    minimum,
                    maximum,
                    unit: Some("ms"),
                },
                (
                    ConfigurationValue::I64(_),
                    StandardConfigurationRule::I64Range { minimum, maximum },
                ) => FaceControlKind::ScalarNumber {
                    minimum,
                    maximum,
                    unit: "µ",
                },
                (ConfigurationValue::Text(_), StandardConfigurationRule::TextBytes { maximum }) => {
                    FaceControlKind::ShortText {
                        maximum_bytes: maximum,
                    }
                }
                (ConfigurationValue::Text(_), StandardConfigurationRule::TextOneOf { values }) => {
                    FaceControlKind::TextChoice { choices: values }
                }
                _ => return Err(PatchbayGraphError::InvalidConfigurationContract),
            };
            Ok(FaceControl {
                key: field.key,
                value,
                kind,
            })
        })
        .collect()
}
