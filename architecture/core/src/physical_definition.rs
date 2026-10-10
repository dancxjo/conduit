//! Bounded self-contained physical definitions admitted from checked language data.
//! Names are evidence; content identities freeze meaning independently of a registry.
use crate::semantic_digest;

#[path = "physical_definition/dimension.rs"]
mod dimension;
#[path = "physical_definition/family.rs"]
mod family;
#[path = "physical_definition/scalar.rs"]
mod scalar;
#[path = "physical_definition/unit.rs"]
mod unit;
pub use dimension::*;
pub use family::*;
pub use scalar::*;
pub use unit::*;
#[path = "physical_definition/generated.rs"]
mod generated;
pub use generated::*;
#[path = "physical_definition/role_kind.rs"]
mod role_kind;
pub use role_kind::*;
#[path = "physical_definition/serde_fixed.rs"]
mod serde_fixed;
pub(crate) use serde_fixed::deserialize_fixed;

pub const DEFINITION_NAME_BYTES: usize = 128;
pub const DEFINITION_NAME_ENCODED_LEN: usize = 129;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefinitionName {
    bytes: [u8; DEFINITION_NAME_BYTES],
    length: u8,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PhysicalDefinitionRefusal {
    EmptyName,
    NameTooLong,
    InvalidUtf8,
    NonCanonicalEncoding,
    WrongEncodingLength,
    UnsupportedVersion,
    TooManyDimensionTerms,
    InvalidDimensionPower,
    DuplicateDimensionAnchor,
    InvalidRoles,
    InvalidTransform,
    TransformOverflow,
    InvalidExponent,
    PrefixNotEnabled,
    InvalidPrefix,
    AlreadyPrefixed,
    IncompatibleFamily,
    IdentityMismatch,
}
impl DefinitionName {
    pub(crate) const fn from_generated(bytes: &[u8], position: usize) -> Self {
        let mut result = Self {
            bytes: [0; DEFINITION_NAME_BYTES],
            length: bytes[position],
        };
        let mut index = 0;
        while index < DEFINITION_NAME_BYTES {
            result.bytes[index] = bytes[position + 1 + index];
            index += 1;
        }
        result
    }
    pub fn new(source: &str) -> Result<Self, PhysicalDefinitionRefusal> {
        if source.is_empty() {
            return Err(PhysicalDefinitionRefusal::EmptyName);
        }
        if source.len() > DEFINITION_NAME_BYTES {
            return Err(PhysicalDefinitionRefusal::NameTooLong);
        }
        let mut bytes = [0; DEFINITION_NAME_BYTES];
        bytes[..source.len()].copy_from_slice(source.as_bytes());
        Ok(Self {
            bytes,
            length: source.len() as u8,
        })
    }
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.length as usize]).expect("checked definition name")
    }
    pub(crate) fn encode_into(self, output: &mut [u8]) {
        output[0] = self.length;
        output[1..].copy_from_slice(&self.bytes);
    }
    pub(crate) fn decode(input: &[u8]) -> Result<Self, PhysicalDefinitionRefusal> {
        if input.len() != DEFINITION_NAME_ENCODED_LEN {
            return Err(PhysicalDefinitionRefusal::WrongEncodingLength);
        }
        let length = input[0] as usize;
        if length == 0 {
            return Err(PhysicalDefinitionRefusal::EmptyName);
        }
        if length > DEFINITION_NAME_BYTES {
            return Err(PhysicalDefinitionRefusal::NameTooLong);
        }
        if input[1 + length..].iter().any(|b| *b != 0) {
            return Err(PhysicalDefinitionRefusal::NonCanonicalEncoding);
        }
        let source = core::str::from_utf8(&input[1..1 + length])
            .map_err(|_| PhysicalDefinitionRefusal::InvalidUtf8)?;
        Self::new(source)
    }
}
fn identity(domain: &str, bytes: &[u8]) -> [u8; 32] {
    semantic_digest(domain, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn family(name: &str) -> QuantityFamilyDefinition {
        QuantityFamilyDefinition::new(
            name,
            DimensionDefinition::new(&[DimensionTerm {
                anchor: DimensionDefinition::anchor("test-length").unwrap(),
                power: 1,
            }])
            .unwrap(),
            QuantityRoles::Linear,
        )
        .unwrap()
    }
    #[test]
    fn all_generated_source_capsules_are_independently_checked() {
        assert_eq!(BUILTIN_UNIT_DEFINITIONS.len(), 130);
        for &(symbol, definition) in BUILTIN_UNIT_DEFINITIONS {
            assert_eq!(definition.symbol(), symbol);
            assert_eq!(
                UnitDefinition::decode(&definition.encode()),
                Ok(definition),
                "{symbol}"
            );
        }
        assert_eq!(BUILTIN_NANOVOLT.decimal_exponent(), -9);
        assert_eq!(BUILTIN_NANOAMPERE.decimal_exponent(), -9);
    }
    #[test]
    fn arbitrary_definitions_are_self_contained_and_immutable() {
        let family = family("CustomLength");
        let first = UnitDefinition::new(family, "widget", 7, 0, 3, 0, PrefixPolicy::NONE).unwrap();
        let replacement =
            UnitDefinition::new(family, "widget", 8, 0, 3, 0, PrefixPolicy::NONE).unwrap();
        assert_ne!(first.identity(), replacement.identity());
        assert_eq!(UnitDefinition::decode(&first.encode()), Ok(first));
        assert!(first
            .exact_scale()
            .equivalent(DefinitionScalar::new(7, 3, 0).unwrap()));
        let mut forged = first.encode();
        forged[UNIT_DEFINITION_ENCODED_LEN - 1] ^= 1;
        assert_eq!(
            UnitDefinition::decode(&forged),
            Err(PhysicalDefinitionRefusal::IdentityMismatch)
        );
    }
    #[test]
    fn declared_point_delta_names_and_offsets_are_separate() {
        let family = QuantityFamilyDefinition::point_delta(
            "Temperature",
            "TemperatureDelta",
            DimensionDefinition::DIMENSIONLESS,
        )
        .unwrap();
        assert_eq!(family.role_name(QuantityRole::Point), Ok("Temperature"));
        assert_eq!(
            family.role_name(QuantityRole::Delta),
            Ok("TemperatureDelta")
        );
        assert_ne!(
            family.role_identity(QuantityRole::Point),
            family.role_identity(QuantityRole::Delta)
        );
        let celsius =
            UnitDefinition::new(family, "°C", 100, 27315, 100, 0, PrefixPolicy::NONE).unwrap();
        assert!(celsius
            .exact_offset(QuantityRole::Point)
            .unwrap()
            .equivalent(DefinitionScalar::new(5463, 20, 0).unwrap()));
        assert_eq!(
            celsius.exact_offset(QuantityRole::Delta).unwrap().numerator,
            0
        );
        assert!(celsius.exact_offset(QuantityRole::Linear).is_err());
        assert_eq!(
            QuantityFamilyDefinition::decode(&family.encode()),
            Ok(family)
        );
    }
    #[test]
    fn prefix_policies_are_explicit_and_power_is_preserved() {
        let linear = family("Volume");
        let plain = UnitDefinition::new(linear, "custom", 1, 0, 1, 0, PrefixPolicy::NONE).unwrap();
        assert_eq!(
            plain.with_decimal_prefix("kcustom", 3),
            Err(PhysicalDefinitionRefusal::PrefixNotEnabled)
        );
        assert_eq!(
            plain.with_binary_prefix("Kicustom", 10),
            Err(PhysicalDefinitionRefusal::PrefixNotEnabled)
        );
        let decimal = UnitDefinition::new(
            linear,
            "custom³",
            1,
            0,
            1,
            0,
            PrefixPolicy::new(&[30, 3], &[], 3).unwrap(),
        )
        .unwrap();
        let prefixed = decimal.with_decimal_prefix("Qcustom³", 30).unwrap();
        assert_eq!(prefixed.decimal_exponent(), 90);
        assert_eq!(UnitDefinition::decode(&prefixed.encode()), Ok(prefixed));
        assert_eq!(
            prefixed.with_decimal_prefix("kQcustom³", 3),
            Err(PhysicalDefinitionRefusal::AlreadyPrefixed)
        );
        let binary = UnitDefinition::new(
            linear,
            "block",
            1,
            0,
            1,
            0,
            PrefixPolicy::new(&[], &[10], 2).unwrap(),
        )
        .unwrap()
        .with_binary_prefix("Kiblock", 10)
        .unwrap();
        assert_eq!(binary.binary_exponent(), 20);
        assert_eq!(UnitDefinition::decode(&binary.encode()), Ok(binary));
    }
    #[test]
    fn related_affine_laws_reduce_without_losing_offsets() {
        let family = QuantityFamilyDefinition::point_delta(
            "Point",
            "Delta",
            DimensionDefinition::DIMENSIONLESS,
        )
        .unwrap();
        let reference =
            UnitDefinition::new(family, "origin", 1, 27315, 100, 0, PrefixPolicy::NONE).unwrap();
        let relative =
            UnitDefinition::related(reference, "relative", 2, 3, 5, PrefixPolicy::NONE).unwrap();
        assert!(relative
            .exact_scale()
            .equivalent(DefinitionScalar::new(1, 250, 0).unwrap()));
        assert!(relative
            .exact_offset(QuantityRole::Point)
            .unwrap()
            .equivalent(DefinitionScalar::new(68289, 250, 0).unwrap()));
        assert_eq!(UnitDefinition::decode(&relative.encode()), Ok(relative));
    }
}

pub(crate) const fn generated_digest(bytes: &[u8], position: usize) -> [u8; 32] {
    let mut result = [0; 32];
    let mut i = 0;
    while i < 32 {
        result[i] = bytes[position + i];
        i += 1;
    }
    result
}
pub(crate) const fn generated_i128(bytes: &[u8], position: usize) -> i128 {
    let mut result = [0; 16];
    let mut i = 0;
    while i < 16 {
        result[i] = bytes[position + i];
        i += 1;
    }
    i128::from_le_bytes(result)
}
