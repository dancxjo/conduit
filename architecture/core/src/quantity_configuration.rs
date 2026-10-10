//! Checked physical startup values with bounded, separately retained source evidence.

use crate::{Quantity, QuantityRefusal, Unit, UnitRefusal};
use alloc::{string::String, vec::Vec};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct QuantityConfigurationValue {
    canonical_value: Vec<u8>,
    source: String,
}

impl QuantityConfigurationValue {
    pub fn from_value(value: Quantity) -> Result<Self, QuantityRefusal> {
        let source = value.canonical_literal()?;
        Ok(Self {
            canonical_value: value.encode().to_vec(),
            source,
        })
    }

    pub fn new(value: Quantity, source: String) -> Option<Self> {
        (Quantity::parse_plot_literal(&source).ok()? == value).then(|| Self {
            canonical_value: value.encode().to_vec(),
            source,
        })
    }
    pub fn parse(source: &str) -> Result<Self, QuantityRefusal> {
        let value = Quantity::parse_plot_literal(source)?;
        Ok(Self {
            canonical_value: value.encode().to_vec(),
            source: source.into(),
        })
    }
    pub fn value(&self) -> Quantity {
        Quantity::decode(&self.canonical_value).expect("checked exact quantity configuration")
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical_value
    }
}

impl<'de> Deserialize<'de> for QuantityConfigurationValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Encoded {
            canonical_value: Vec<u8>,
            source: String,
        }
        let encoded = Encoded::deserialize(deserializer)?;
        let value = Quantity::decode(&encoded.canonical_value)
            .map_err(|_| D::Error::custom("invalid exact quantity configuration encoding"))?;
        Self::new(value, encoded.source)
            .ok_or_else(|| D::Error::custom("exact quantity source evidence mismatch"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct UnitConfigurationValue {
    canonical_value: Vec<u8>,
    source: String,
}

impl UnitConfigurationValue {
    pub fn new(value: Unit, source: String) -> Option<Self> {
        (Unit::resolve(&source).ok()? == value).then(|| Self {
            canonical_value: value.encode().to_vec(),
            source,
        })
    }
    pub fn parse(source: &str) -> Result<Self, UnitRefusal> {
        let value = Unit::resolve(source)?;
        Ok(Self {
            canonical_value: value.encode().to_vec(),
            source: source.into(),
        })
    }
    pub fn value(&self) -> Unit {
        Unit::decode(&self.canonical_value).expect("checked unit configuration")
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical_value
    }
}

impl<'de> Deserialize<'de> for UnitConfigurationValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Encoded {
            canonical_value: Vec<u8>,
            source: String,
        }
        let encoded = Encoded::deserialize(deserializer)?;
        let value = Unit::decode(&encoded.canonical_value)
            .map_err(|_| D::Error::custom("invalid unit configuration encoding"))?;
        Self::new(value, encoded.source)
            .ok_or_else(|| D::Error::custom("unit source evidence mismatch"))
    }
}

/// Difference evidence is checked under the distinct temperature-difference
/// law; its coordinate never becomes an absolute temperature point.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ExactTemperatureDifferenceConfigurationValue {
    canonical_value: Vec<u8>,
    source: String,
}

impl ExactTemperatureDifferenceConfigurationValue {
    pub fn new(value: crate::ExactTemperatureDifference, source: String) -> Option<Self> {
        (crate::ExactTemperatureDifference::parse_plot_literal(&source).ok()? == value).then(|| {
            Self {
                canonical_value: value.storage_coordinate().encode().to_vec(),
                source,
            }
        })
    }
    pub fn parse(source: &str) -> Result<Self, crate::ExactTemperatureDifferenceRefusal> {
        let value = crate::ExactTemperatureDifference::parse_plot_literal(source)?;
        Ok(Self {
            canonical_value: value.storage_coordinate().encode().to_vec(),
            source: source.into(),
        })
    }
    pub fn value(&self) -> crate::ExactTemperatureDifference {
        let coordinate =
            Quantity::decode(&self.canonical_value).expect("checked difference coordinate");
        crate::ExactTemperatureDifference::new(
            coordinate.coefficient(),
            coordinate.exponent(),
            coordinate.unit(),
        )
        .expect("checked temperature difference configuration")
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical_value
    }
}

impl<'de> Deserialize<'de> for ExactTemperatureDifferenceConfigurationValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Encoded {
            canonical_value: Vec<u8>,
            source: String,
        }
        let encoded = Encoded::deserialize(deserializer)?;
        let coordinate = Quantity::decode(&encoded.canonical_value)
            .map_err(|_| D::Error::custom("invalid temperature difference encoding"))?;
        let value = crate::ExactTemperatureDifference::new(
            coordinate.coefficient(),
            coordinate.exponent(),
            coordinate.unit(),
        )
        .map_err(|_| D::Error::custom("invalid temperature difference dimension"))?;
        Self::new(value, encoded.source)
            .ok_or_else(|| D::Error::custom("temperature difference source evidence mismatch"))
    }
}
