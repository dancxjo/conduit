//! Standalone normalized Dimension values, independent of Unit coordinates and
//! nominal Quantity families. Source custody is retained by checked preparation.
//! The content digest detects inconsistent encodings; it is not authorization to
//! introduce a new basis into an existing checked declaration snapshot.
use crate::{
    DimensionDefinition, DimensionTerm, PhysicalDefinitionRefusal, DIMENSION_DEFINITION_ENCODED_LEN,
};

pub const DIMENSION_INFO_ID: &str = "value/dimension@1";
pub const DIMENSION_ENCODED_LEN: usize = 1 + DIMENSION_DEFINITION_ENCODED_LEN + 32;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dimension(DimensionDefinition);

impl Dimension {
    pub const DIMENSIONLESS: Self = Self(DimensionDefinition::DIMENSIONLESS);
    pub const fn from_definition(definition: DimensionDefinition) -> Self {
        Self(definition)
    }
    pub const fn definition(self) -> DimensionDefinition {
        self.0
    }
    pub fn basis(name: &str) -> Result<Self, PhysicalDefinitionRefusal> {
        DimensionDefinition::new(&[DimensionTerm {
            anchor: DimensionDefinition::anchor(name)?,
            power: 1,
        }])
        .map(Self)
    }
    pub fn product(self, other: Self) -> Result<Self, PhysicalDefinitionRefusal> {
        self.0.product(other.0).map(Self)
    }
    pub fn quotient(self, other: Self) -> Result<Self, PhysicalDefinitionRefusal> {
        self.0.quotient(other.0).map(Self)
    }
    pub fn power(self, power: i8) -> Result<Self, PhysicalDefinitionRefusal> {
        self.0.power(power).map(Self)
    }
    pub fn semantic_digest(self) -> [u8; 32] {
        crate::semantic_digest(DIMENSION_INFO_ID, &self.0.encode())
    }
    pub fn encode(self) -> [u8; DIMENSION_ENCODED_LEN] {
        let mut bytes = [0; DIMENSION_ENCODED_LEN];
        bytes[0] = 1;
        bytes[1..1 + DIMENSION_DEFINITION_ENCODED_LEN].copy_from_slice(&self.0.encode());
        bytes[1 + DIMENSION_DEFINITION_ENCODED_LEN..].copy_from_slice(&self.semantic_digest());
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, PhysicalDefinitionRefusal> {
        if bytes.len() != DIMENSION_ENCODED_LEN {
            return Err(PhysicalDefinitionRefusal::WrongEncodingLength);
        }
        if bytes[0] != 1 {
            return Err(PhysicalDefinitionRefusal::UnsupportedVersion);
        }
        let value = Self(DimensionDefinition::decode(
            &bytes[1..1 + DIMENSION_DEFINITION_ENCODED_LEN],
        )?);
        if bytes[1 + DIMENSION_DEFINITION_ENCODED_LEN..] != value.semantic_digest() {
            return Err(PhysicalDefinitionRefusal::IdentityMismatch);
        }
        Ok(value)
    }
}
impl serde::Serialize for Dimension {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(self.encode().as_slice(), serializer)
    }
}
impl<'de> serde::Deserialize<'de> for Dimension {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let bytes =
            crate::physical_definition::deserialize_fixed::<D, DIMENSION_ENCODED_LEN>(decoder)?;
        Self::decode(&bytes).map_err(|_| serde::de::Error::custom("invalid Dimension value"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_algebra_preserves_named_angle_and_cancels_exactly() {
        let length = Dimension::basis("Length").unwrap();
        let time = Dimension::basis("Time").unwrap();
        let speed = length.quotient(time).unwrap();
        assert_eq!(speed.product(time).unwrap(), length);
        assert_eq!(length.product(time).unwrap(), time.product(length).unwrap());
        assert_eq!(length.quotient(length).unwrap(), Dimension::DIMENSIONLESS);
        assert_ne!(Dimension::basis("Angle").unwrap(), Dimension::DIMENSIONLESS);
        assert_eq!(Dimension::decode(&speed.encode()).unwrap(), speed);
        assert_eq!(
            speed.semantic_digest(),
            length
                .product(time.quotient(time.product(time).unwrap()).unwrap())
                .unwrap()
                .semantic_digest()
        );
    }
    #[test]
    fn decoder_rejects_forged_metadata_version_and_padding() {
        let dimension = Dimension::basis("Length").unwrap();
        let mut bytes = dimension.encode();
        bytes[2] ^= 1;
        assert_eq!(
            Dimension::decode(&bytes),
            Err(PhysicalDefinitionRefusal::IdentityMismatch)
        );
        let mut bytes = dimension.encode();
        bytes[0] = 2;
        assert_eq!(
            Dimension::decode(&bytes),
            Err(PhysicalDefinitionRefusal::UnsupportedVersion)
        );
        let mut bytes = dimension.encode();
        bytes[35] = 1;
        assert_eq!(
            Dimension::decode(&bytes),
            Err(PhysicalDefinitionRefusal::NonCanonicalEncoding)
        );
    }
    #[test]
    fn composition_refuses_extent_and_power_without_narrowing() {
        let anchors: alloc::vec::Vec<_> = (0..9)
            .map(|i| DimensionDefinition::anchor(&alloc::format!("basis{i}")).unwrap())
            .collect();
        let terms: alloc::vec::Vec<_> = anchors
            .iter()
            .map(|anchor| DimensionTerm {
                anchor: *anchor,
                power: 1,
            })
            .collect();
        let full = Dimension::from_definition(DimensionDefinition::new(&terms[..8]).unwrap());
        let extra = Dimension::from_definition(DimensionDefinition::new(&terms[8..]).unwrap());
        assert_eq!(
            full.product(extra),
            Err(PhysicalDefinitionRefusal::TooManyDimensionTerms)
        );
        assert_eq!(full.quotient(full).unwrap(), Dimension::DIMENSIONLESS);
        let high = Dimension::from_definition(
            DimensionDefinition::new(&[DimensionTerm {
                anchor: anchors[0],
                power: 16,
            }])
            .unwrap(),
        );
        assert_eq!(
            high.product(high),
            Err(PhysicalDefinitionRefusal::InvalidDimensionPower)
        );
    }
}
