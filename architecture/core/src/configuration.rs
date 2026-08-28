use alloc::{string::String, vec::Vec};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::{KindId, StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructuredConfigurationValue {
    profile: KindId,
    canonical_value: Vec<u8>,
}

impl StructuredConfigurationValue {
    pub fn new(profile: KindId, canonical_value: Vec<u8>) -> Option<Self> {
        if profile.as_str().is_empty()
            || canonical_value.is_empty()
            || canonical_value.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return None;
        }
        let value = StructuredInfoValue::from_canonical_bytes(&canonical_value).ok()?;
        let actual_profile = value.value_type().profile().ok()?;
        (actual_profile.value_kind() == &profile).then_some(Self {
            profile,
            canonical_value,
        })
    }

    pub fn profile(&self) -> &KindId {
        &self.profile
    }

    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical_value
    }
}

impl<'de> Deserialize<'de> for StructuredConfigurationValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Encoded {
            profile: KindId,
            canonical_value: Vec<u8>,
        }

        let encoded = Encoded::deserialize(deserializer)?;
        Self::new(encoded.profile, encoded.canonical_value)
            .ok_or_else(|| D::Error::custom("invalid structured configuration value"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationValue {
    Bool(bool),
    U64(u64),
    /// Signed fixed-point scalar microunits, matching `value/scalar@1`.
    I64(i64),
    Text(String),
    /// Exact finite structured semantic value used by an immutable Gear configuration.
    Structured(StructuredConfigurationValue),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationEntry {
    pub key: String,
    pub value: ConfigurationValue,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kind_id, StructuredInfoType, StructuredInfoValue};
    use alloc::vec;

    #[test]
    fn structured_configuration_requires_matching_profile_and_canonical_value() {
        let value_type = StructuredInfoType::leaf(kind_id("value/count@1")).unwrap();
        let value = StructuredInfoValue::leaf(value_type.clone(), b"7".to_vec()).unwrap();
        let canonical = value.canonical_bytes().unwrap();
        let profile = value_type.profile().unwrap().value_kind().clone();

        assert!(StructuredConfigurationValue::new(profile, canonical.clone()).is_some());
        assert!(
            StructuredConfigurationValue::new(kind_id("structured-info/wrong@1"), canonical)
                .is_none()
        );
        assert!(StructuredConfigurationValue::new(
            value_type.profile().unwrap().value_kind().clone(),
            vec![0xff],
        )
        .is_none());
    }
}
