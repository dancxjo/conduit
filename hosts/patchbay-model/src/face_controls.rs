//! Compact Gear Face controls derived from authoritative checked contracts.

use conduit_core::{
    BoundKind, ConfigurationValue, InfoBool, InteractionContract, InteractionCurrentState,
    InteractionDomain, InteractionFamily, InteractionOption, InteractionValue, KindId,
    OptionAvailability, Quantity, QuantityUnit, BOOL_INFO_ID, QUANTITY_INFO_ID, TEXT_INFO_ID,
};
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

impl FaceControlKind {
    pub fn text_choices(&self) -> Option<&[String]> {
        match self {
            Self::TextChoice { choices } => Some(choices),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceControl {
    pub key: String,
    pub value: ConfigurationValue,
    pub kind: FaceControlKind,
    pub interaction: Option<FaceInteraction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceInteraction {
    pub contract: InteractionContract,
    pub state: InteractionCurrentState,
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
            let kind = match (&value, &field.rule) {
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
                    if maximum.saturating_sub(*minimum) <= 100 {
                        FaceControlKind::Range {
                            minimum: *minimum,
                            maximum: *maximum,
                            unit,
                        }
                    } else {
                        FaceControlKind::Number {
                            minimum: *minimum,
                            maximum: *maximum,
                            unit,
                        }
                    }
                }
                (
                    ConfigurationValue::U64(_),
                    StandardConfigurationRule::DurationMillis { minimum, maximum },
                ) => FaceControlKind::Number {
                    minimum: *minimum,
                    maximum: *maximum,
                    unit: Some("ms"),
                },
                (
                    ConfigurationValue::I64(_),
                    StandardConfigurationRule::I64Range { minimum, maximum },
                ) => FaceControlKind::ScalarNumber {
                    minimum: *minimum,
                    maximum: *maximum,
                    unit: "µ",
                },
                (ConfigurationValue::Text(_), StandardConfigurationRule::TextBytes { maximum }) => {
                    FaceControlKind::ShortText {
                        maximum_bytes: *maximum,
                    }
                }
                (ConfigurationValue::Text(_), StandardConfigurationRule::TextOneOf { values }) => {
                    FaceControlKind::TextChoice {
                        choices: values.clone(),
                    }
                }
                _ => return Err(PatchbayGraphError::InvalidConfigurationContract),
            };
            let interaction = project_interaction(gear, &field.key, &value, &field.rule)?;
            Ok(FaceControl {
                key: field.key,
                value,
                kind,
                interaction,
            })
        })
        .collect()
}

fn project_interaction(
    gear: &CheckedGear,
    key: &str,
    value: &ConfigurationValue,
    rule: &StandardConfigurationRule,
) -> Result<Option<FaceInteraction>, PatchbayGraphError> {
    let semantic_id = format!("configuration/{}/{}", gear.kind_id.as_str(), key);
    let (family, domain, current) = match (value, rule) {
        (ConfigurationValue::Bool(value), StandardConfigurationRule::Any) => (
            InteractionFamily::Boolean,
            None,
            interaction_value(
                BOOL_INFO_ID,
                &if *value {
                    InfoBool::TRUE
                } else {
                    InfoBool::FALSE
                }
                .encode(),
            )?,
        ),
        (
            ConfigurationValue::U64(value),
            StandardConfigurationRule::U64Range { minimum, maximum }
            | StandardConfigurationRule::DurationMillis { minimum, maximum },
        ) => {
            let Ok(value) = i64::try_from(*value) else {
                return Ok(None);
            };
            let minimum = i64::try_from(*minimum).unwrap_or(i64::MAX);
            let maximum = i64::try_from(*maximum).unwrap_or(i64::MAX);
            if minimum > maximum || value < minimum || value > maximum {
                return Ok(None);
            }
            let unit = if matches!(rule, StandardConfigurationRule::DurationMillis { .. })
                || key.ends_with("-ms")
            {
                QuantityUnit::Millisecond
            } else {
                QuantityUnit::One
            };
            (
                InteractionFamily::Scalar {
                    unit,
                    minimum,
                    minimum_bound: BoundKind::Inclusive,
                    maximum,
                    maximum_bound: BoundKind::Inclusive,
                    granularity: 1,
                },
                None,
                quantity_value(value, unit)?,
            )
        }
        (
            ConfigurationValue::I64(value),
            StandardConfigurationRule::I64Range { minimum, maximum },
        ) => (
            InteractionFamily::Scalar {
                unit: QuantityUnit::Millionth,
                minimum: *minimum,
                minimum_bound: BoundKind::Inclusive,
                maximum: *maximum,
                maximum_bound: BoundKind::Inclusive,
                granularity: 1,
            },
            None,
            quantity_value(*value, QuantityUnit::Millionth)?,
        ),
        (ConfigurationValue::Text(value), StandardConfigurationRule::TextBytes { maximum }) => (
            InteractionFamily::Text {
                maximum_bytes: *maximum,
                allow_empty: true,
            },
            None,
            interaction_value(TEXT_INFO_ID, value.as_bytes())?,
        ),
        (ConfigurationValue::Text(value), StandardConfigurationRule::TextOneOf { values }) => {
            let value_kind = KindId::from("configuration/text-choice@1");
            let options = values
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    Ok(InteractionOption {
                        identity: format!("{semantic_id}/option/{index}"),
                        value: InteractionValue::new(
                            value_kind.clone(),
                            choice.as_bytes().to_vec(),
                        )
                        .map_err(|_| PatchbayGraphError::InvalidConfigurationContract)?,
                        availability: OptionAvailability::Available,
                    })
                })
                .collect::<Result<Vec<_>, PatchbayGraphError>>()?;
            (
                InteractionFamily::ChooseOne {
                    value_kind: value_kind.clone(),
                    maximum_options: u16::try_from(values.len())
                        .map_err(|_| PatchbayGraphError::InvalidConfigurationContract)?,
                },
                Some(InteractionDomain {
                    revision: 0,
                    options,
                }),
                InteractionValue::new(value_kind, value.as_bytes().to_vec())
                    .map_err(|_| PatchbayGraphError::InvalidConfigurationContract)?,
            )
        }
        _ => return Ok(None),
    };
    let contract = InteractionContract::new(semantic_id, family)
        .map_err(|_| PatchbayGraphError::InvalidConfigurationContract)?;
    let state = InteractionCurrentState::new(&contract, 0, domain, vec![current])
        .map_err(|_| PatchbayGraphError::InvalidConfigurationContract)?;
    Ok(Some(FaceInteraction { contract, state }))
}

fn interaction_value(kind: &str, bytes: &[u8]) -> Result<InteractionValue, PatchbayGraphError> {
    InteractionValue::new(KindId::from(kind), bytes.to_vec())
        .map_err(|_| PatchbayGraphError::InvalidConfigurationContract)
}

fn quantity_value(value: i64, unit: QuantityUnit) -> Result<InteractionValue, PatchbayGraphError> {
    interaction_value(QUANTITY_INFO_ID, &Quantity::new(value, unit).encode())
}
