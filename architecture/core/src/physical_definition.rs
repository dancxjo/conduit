//! Bounded self-contained physical definitions admitted from checked language data.
//! Names are evidence; content identities freeze meaning independently of a registry.
use crate::semantic_digest;

#[path = "physical_definition/dimension.rs"]
mod dimension;
#[path = "physical_definition/family.rs"]
mod family;
#[path = "physical_definition/unit.rs"]
mod unit;
pub use dimension::*;
pub use family::*;
#[path = "physical_definition/numeric_profile.rs"]
mod numeric_profile;
pub use numeric_profile::*;
pub use unit::*;
#[path = "physical_definition/generated.rs"]
mod generated;
pub use generated::*;
#[path = "physical_definition/role_kind.rs"]
mod role_kind;
pub use role_kind::*;
#[path = "physical_definition/serde_fixed.rs"]
mod serde_fixed;
pub(crate) use serde_fixed::deserialize_bounded;

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
    Numeric(crate::NumericRefusal),
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

pub(crate) const fn generated_digest(bytes: &[u8], position: usize) -> [u8; 32] {
    let mut result = [0; 32];
    let mut i = 0;
    while i < 32 {
        result[i] = bytes[position + i];
        i += 1;
    }
    result
}
#[cfg(test)]
mod tests {
 use super::*;use crate::MathValue;
 fn family()->QuantityFamilyDefinition {QuantityFamilyDefinition::new("Custom",DimensionDefinition::DIMENSIONLESS,QuantityRoles::Linear).unwrap()}
 #[test]
 fn all_generated_source_capsules_are_independently_checked() {
  for (symbol,definition) in BUILTIN_UNIT_DEFINITIONS {
   assert_eq!(definition.symbol(),*symbol);
   assert_eq!(UnitDefinition::decode(definition.encoded()).unwrap(),*definition);
  }
  assert_eq!(BUILTIN_NANOVOLT.exact_scale().to_owned().unwrap(),MathValue::parse_decimal("1e-9").unwrap());
  assert_eq!(BUILTIN_NANOAMPERE.exact_scale().to_owned().unwrap(),MathValue::parse_decimal("1e-9").unwrap());
 }
 #[test]
 fn source_root_redefinition_cannot_change_captured_values() {
  let first=UnitDefinition::new_math_role(family(),"widget",MathValue::rational("7","3").unwrap(),MathValue::ZERO,PrefixPolicy::NONE,QuantityRole::Linear).unwrap();
  let replacement=UnitDefinition::new_math_role(family(),"widget",MathValue::rational("8","3").unwrap(),MathValue::ZERO,PrefixPolicy::NONE,QuantityRole::Linear).unwrap();
  assert_ne!(first.identity(),replacement.identity());assert_ne!(first.reference_anchor(),replacement.reference_anchor());
  let mut forged=first.encode();let n=forged.len();forged[n-1]^=1;
  assert_eq!(UnitDefinition::decode(&forged),Err(PhysicalDefinitionRefusal::IdentityMismatch));
  let a=crate::Quantity::new(1,first);let b=crate::Quantity::new(1,replacement);
  assert_eq!(a.compare(&b),Err(crate::QuantityConversionRefusal::UnlinkedOrigin));
 }
 #[test]
 fn point_and_delta_use_one_captured_law_with_distinct_roles() {
  let family=QuantityFamilyDefinition::point_delta("Point","Delta",DimensionDefinition::DIMENSIONLESS).unwrap();
  let unit=UnitDefinition::new_math_role(family,"affine",MathValue::ONE,MathValue::parse_decimal("273.15").unwrap(),PrefixPolicy::NONE,QuantityRole::Point).unwrap();
  assert_eq!(unit.exact_offset(QuantityRole::Point).unwrap().to_owned().unwrap(),MathValue::parse_decimal("273.15").unwrap());
  assert!(unit.exact_offset(QuantityRole::Delta).unwrap().is_zero());
  assert!(unit.exact_offset(QuantityRole::Linear).is_err());
 }
 #[test]
 fn prefixes_require_admitted_policy_and_preserve_exact_power() {
  let plain=UnitDefinition::new_math_role(family(),"custom",MathValue::ONE,MathValue::ZERO,PrefixPolicy::NONE,QuantityRole::Linear).unwrap();
  assert_eq!(plain.with_decimal_prefix("kcustom",3),Err(PhysicalDefinitionRefusal::PrefixNotEnabled));
  let cubic=UnitDefinition::new_math_role(family(),"custom³",MathValue::ONE,MathValue::ZERO,PrefixPolicy::new(&[30],&[],3).unwrap(),QuantityRole::Linear).unwrap();
  let q=cubic.with_decimal_prefix("Qcustom³",30).unwrap();
  assert_eq!(q.exact_scale().to_owned().unwrap(),MathValue::parse_decimal("1e90").unwrap());
  assert_eq!(q.with_decimal_prefix("kQcustom³",3),Err(PhysicalDefinitionRefusal::AlreadyPrefixed));
  let binary=UnitDefinition::new_math_role(family(),"block",MathValue::ONE,MathValue::ZERO,PrefixPolicy::new(&[],&[10],2).unwrap(),QuantityRole::Linear).unwrap().with_binary_prefix("Kiblock",10).unwrap();
  assert_eq!(binary.exact_scale().checked_i64().unwrap(),1_048_576);
 }
}
