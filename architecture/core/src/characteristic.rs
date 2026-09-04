use crate::{CharacteristicId, Quantity, QuantityConversionRefusal, QuantityUnit};
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CharacteristicSubject {
    Realization,
    Resource,
    ComputeTopology,
    HostBase,
    Observation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CharacteristicStability {
    Stable,
    CurrentObservation,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CharacteristicUnit {
    Bytes,
    Tokens,
    Hertz,
    Millihertz,
    Microcents,
    Microseconds,
    Frames,
    EventsPerSecond,
    Items,
    Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CharacteristicValueKind {
    Boolean,
    UnsignedQuantity {
        unit: CharacteristicUnit,
        maximum: u64,
    },
    Categorical {
        allowed_labels: Vec<String>,
        ordered: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CharacteristicDefinition {
    pub characteristic_id: CharacteristicId,
    pub subject: CharacteristicSubject,
    pub stability: CharacteristicStability,
    pub value_kind: CharacteristicValueKind,
    pub human_name: String,
    pub help: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CharacteristicValue {
    Boolean(bool),
    UnsignedQuantity {
        value: u64,
        unit: CharacteristicUnit,
    },
    Categorical(String),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CharacteristicQuantity {
    pub value: u64,
    pub unit: CharacteristicUnit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CharacteristicDefinitionError {
    EmptyIdentity,
    EmptyDocumentation,
    EmptyLabelSet,
    DuplicateLabel,
    UnsortedLabels,
    ZeroMaximum,
    WrongSubject,
    ObservedFactAdvertisedAsStable,
    ValueKindMismatch,
    UnitMismatch,
    QuantityOutOfRange,
    UnknownLabel,
    UnsupportedQuantityUnit,
    NegativeQuantity,
    QuantityConversion(QuantityConversionRefusal),
}

impl CharacteristicUnit {
    pub const fn quantity_unit(self) -> Option<QuantityUnit> {
        match self {
            Self::Bytes => Some(QuantityUnit::Byte),
            Self::Hertz => Some(QuantityUnit::Hertz),
            Self::Millihertz => Some(QuantityUnit::Millihertz),
            Self::Microseconds => Some(QuantityUnit::Microsecond),
            Self::Tokens
            | Self::Microcents
            | Self::Frames
            | Self::EventsPerSecond
            | Self::Items
            | Self::Identifier => None,
        }
    }
}

impl CharacteristicQuantity {
    pub fn from_quantity(
        quantity: Quantity,
        unit: CharacteristicUnit,
    ) -> Result<Self, CharacteristicDefinitionError> {
        let target = unit
            .quantity_unit()
            .ok_or(CharacteristicDefinitionError::UnsupportedQuantityUnit)?;
        let converted = quantity
            .convert(target)
            .map_err(CharacteristicDefinitionError::QuantityConversion)?;
        let value = u64::try_from(converted.value())
            .map_err(|_| CharacteristicDefinitionError::NegativeQuantity)?;
        Ok(Self { value, unit })
    }

    pub fn quantity(self) -> Option<Quantity> {
        let unit = self.unit.quantity_unit()?;
        let value = i64::try_from(self.value).ok()?;
        Some(Quantity::new(value, unit))
    }
}

impl CharacteristicDefinition {
    pub fn validate(&self) -> Result<(), CharacteristicDefinitionError> {
        if self.characteristic_id.as_str().is_empty() {
            return Err(CharacteristicDefinitionError::EmptyIdentity);
        }
        if self.human_name.is_empty() || self.help.is_empty() {
            return Err(CharacteristicDefinitionError::EmptyDocumentation);
        }
        match &self.value_kind {
            CharacteristicValueKind::UnsignedQuantity { maximum, .. } if *maximum == 0 => {
                Err(CharacteristicDefinitionError::ZeroMaximum)
            }
            CharacteristicValueKind::Categorical { allowed_labels, .. } => {
                if allowed_labels.is_empty() {
                    return Err(CharacteristicDefinitionError::EmptyLabelSet);
                }
                if allowed_labels.iter().any(String::is_empty) {
                    return Err(CharacteristicDefinitionError::EmptyIdentity);
                }
                if allowed_labels.windows(2).any(|pair| pair[0] > pair[1]) {
                    return Err(CharacteristicDefinitionError::UnsortedLabels);
                }
                if allowed_labels.windows(2).any(|pair| pair[0] == pair[1]) {
                    return Err(CharacteristicDefinitionError::DuplicateLabel);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn validate_realization_value(
        &self,
        value: &CharacteristicValue,
    ) -> Result<(), CharacteristicDefinitionError> {
        self.validate()?;
        if self.subject != CharacteristicSubject::Realization {
            return Err(CharacteristicDefinitionError::WrongSubject);
        }
        if self.stability != CharacteristicStability::Stable {
            return Err(CharacteristicDefinitionError::ObservedFactAdvertisedAsStable);
        }
        match (&self.value_kind, value) {
            (CharacteristicValueKind::Boolean, CharacteristicValue::Boolean(_)) => Ok(()),
            (
                CharacteristicValueKind::UnsignedQuantity { unit, maximum },
                CharacteristicValue::UnsignedQuantity {
                    value,
                    unit: value_unit,
                },
            ) => {
                if unit != value_unit {
                    Err(CharacteristicDefinitionError::UnitMismatch)
                } else if value > maximum {
                    Err(CharacteristicDefinitionError::QuantityOutOfRange)
                } else {
                    Ok(())
                }
            }
            (
                CharacteristicValueKind::Categorical { allowed_labels, .. },
                CharacteristicValue::Categorical(label),
            ) => allowed_labels
                .binary_search(label)
                .map(|_| ())
                .map_err(|_| CharacteristicDefinitionError::UnknownLabel),
            _ => Err(CharacteristicDefinitionError::ValueKindMismatch),
        }
    }

    pub fn categorical_rank(&self, value: &CharacteristicValue) -> Option<usize> {
        match (&self.value_kind, value) {
            (
                CharacteristicValueKind::Categorical {
                    allowed_labels,
                    ordered: true,
                },
                CharacteristicValue::Categorical(label),
            ) => allowed_labels.binary_search(label).ok(),
            _ => None,
        }
    }
}

pub fn stable_realization_boolean(
    id: impl Into<CharacteristicId>,
    human_name: impl Into<String>,
    help: impl Into<String>,
    value: bool,
) -> crate::RealizationCharacteristic {
    crate::RealizationCharacteristic {
        definition: CharacteristicDefinition {
            characteristic_id: id.into(),
            subject: CharacteristicSubject::Realization,
            stability: CharacteristicStability::Stable,
            value_kind: CharacteristicValueKind::Boolean,
            human_name: human_name.into(),
            help: help.into(),
        },
        value: CharacteristicValue::Boolean(value),
    }
}

pub fn stable_realization_quantity(
    id: impl Into<CharacteristicId>,
    human_name: impl Into<String>,
    help: impl Into<String>,
    unit: CharacteristicUnit,
    maximum: u64,
    value: u64,
) -> crate::RealizationCharacteristic {
    crate::RealizationCharacteristic {
        definition: CharacteristicDefinition {
            characteristic_id: id.into(),
            subject: CharacteristicSubject::Realization,
            stability: CharacteristicStability::Stable,
            value_kind: CharacteristicValueKind::UnsignedQuantity { unit, maximum },
            human_name: human_name.into(),
            help: help.into(),
        },
        value: CharacteristicValue::UnsignedQuantity { value, unit },
    }
}

pub fn stable_realization_category(
    id: impl Into<CharacteristicId>,
    human_name: impl Into<String>,
    help: impl Into<String>,
    mut allowed_labels: Vec<String>,
    ordered: bool,
    value: impl Into<String>,
) -> crate::RealizationCharacteristic {
    allowed_labels.sort();
    crate::RealizationCharacteristic {
        definition: CharacteristicDefinition {
            characteristic_id: id.into(),
            subject: CharacteristicSubject::Realization,
            stability: CharacteristicStability::Stable,
            value_kind: CharacteristicValueKind::Categorical {
                allowed_labels,
                ordered,
            },
            human_name: human_name.into(),
            help: help.into(),
        },
        value: CharacteristicValue::Categorical(value.into()),
    }
}

pub(crate) fn push_characteristic_canonical(
    canonical: &mut Vec<u8>,
    characteristic: &crate::RealizationCharacteristic,
) {
    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        push_u32(bytes, value.len() as u32);
        bytes.extend_from_slice(value.as_bytes());
    }

    push_string(
        canonical,
        characteristic.definition.characteristic_id.as_str(),
    );
    canonical.push(characteristic.definition.subject as u8);
    canonical.push(characteristic.definition.stability as u8);
    match &characteristic.definition.value_kind {
        CharacteristicValueKind::Boolean => canonical.push(0),
        CharacteristicValueKind::UnsignedQuantity { unit, maximum } => {
            canonical.push(1);
            canonical.push(*unit as u8);
            push_u64(canonical, *maximum);
        }
        CharacteristicValueKind::Categorical {
            allowed_labels,
            ordered,
        } => {
            canonical.push(2);
            canonical.push(u8::from(*ordered));
            push_u32(canonical, allowed_labels.len() as u32);
            for label in allowed_labels {
                push_string(canonical, label);
            }
        }
    }
    push_string(canonical, &characteristic.definition.human_name);
    push_string(canonical, &characteristic.definition.help);
    match &characteristic.value {
        CharacteristicValue::UnsignedQuantity { value, unit } => {
            canonical.push(0);
            push_u64(canonical, *value);
            canonical.push(*unit as u8);
        }
        CharacteristicValue::Boolean(value) => {
            canonical.push(1);
            canonical.push(u8::from(*value));
        }
        CharacteristicValue::Categorical(value) => {
            canonical.push(2);
            push_string(canonical, value);
        }
    }
}

#[cfg(test)]
mod tests;
