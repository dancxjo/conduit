//! Catalogue-pinned physical Unit values, distinct from the empty product value.

use crate::{
    CatalogUnit, CatalogueDecodeRefusal, DecimalPrefix, PrefixableUnit, QuantityDimension,
    QuantitySuffixRefusal, ResolvedQuantitySuffix, DECIMAL_PREFIXES,
};
use alloc::{format, string::String};
mod catalogue;

pub const UNIT_INFO_ID: &str = "value/unit@1";
pub const UNIT_ENCODED_LEN: usize = 3;
pub const UNIT_MAX_SOURCE_BYTES: usize = 128;

/// The version pins the reviewed unit table and decimal-prefix catalogue.
/// Scale is symbolic and bounded even for cubic quetta-meter (10^90).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Unit {
    base: CatalogUnit,
    prefix_exponent: i8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UnitRefusal {
    SourceTooLong,
    Suffix(QuantitySuffixRefusal),
    WrongEncodingLength,
    UnsupportedEncodingVersion(u8),
    InvalidBase(CatalogueDecodeRefusal),
    InvalidPrefix,
    NonCanonicalEncoding,
}

impl Unit {
    pub fn resolve(source: &str) -> Result<Self, UnitRefusal> {
        if source.len() > UNIT_MAX_SOURCE_BYTES {
            return Err(UnitRefusal::SourceTooLong);
        }
        let suffix = ResolvedQuantitySuffix::resolve(source).map_err(UnitRefusal::Suffix)?;
        Ok(Self::from_resolved(suffix))
    }

    pub fn from_resolved(suffix: ResolvedQuantitySuffix<'_>) -> Self {
        match suffix.base() {
            Some(base) => Self {
                base: base.catalogue_unit(),
                prefix_exponent: suffix.prefix().map_or(0, DecimalPrefix::exponent),
            },
            None => Self {
                base: suffix.catalogue_unit().unwrap(),
                prefix_exponent: 0,
            },
        }
    }

    pub const fn base_unit(self) -> CatalogUnit {
        self.base
    }
    pub const fn dimension(self) -> QuantityDimension {
        self.base.dimension()
    }
    pub const fn catalogue_id(self) -> &'static str {
        crate::QUANTITY_PREFIX_CATALOG_ID
    }
    pub const fn prefix_exponent(self) -> i8 {
        self.prefix_exponent
    }
    pub const fn decimal_exponent(self) -> i16 {
        self.prefix_exponent as i16
            * match self.base {
                CatalogUnit::SquareMeter => 2,
                CatalogUnit::CubicMeter => 3,
                _ => 1,
            }
    }

    /// `(coordinate * scale * 10^decimal_exponent + offset) / denominator`.
    /// The prefix scales the coordinate; it never scales the affine offset.
    pub fn reference_transform(self) -> (i128, i128, i128, i16) {
        let (scale, offset, denominator) = self.base.canonical_transform();
        (scale, offset, denominator, self.decimal_exponent())
    }

    pub fn canonical_symbol(self) -> String {
        if self.prefix_exponent == 0 {
            return self.base.plot_suffix().into();
        }
        let prefix = DECIMAL_PREFIXES
            .iter()
            .find(|prefix| prefix.exponent() == self.prefix_exponent)
            .expect("admitted prefix has catalogue entry");
        format!("{}{}", prefix.symbol(), self.base.plot_suffix())
    }

    pub fn from_plot_suffix(source: &str) -> Result<Self, UnitRefusal> {
        Self::resolve(source)
    }
    pub fn plot_suffix(self) -> String {
        self.canonical_symbol()
    }
    pub fn semantic_id(self) -> String {
        if self.prefix_exponent == 0 {
            self.base.semantic_id().into()
        } else {
            format!("{}@10^{}", self.base.semantic_id(), self.decimal_exponent())
        }
    }
    pub(crate) const fn canonical_transform(self) -> (i128, i128, i128) {
        self.base.canonical_transform()
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        crate::semantic_digest(UNIT_INFO_ID, &self.encode())
    }
    pub fn encode(self) -> [u8; UNIT_ENCODED_LEN] {
        [1, self.base.encode()[0], self.prefix_exponent as u8]
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, UnitRefusal> {
        if bytes.len() != UNIT_ENCODED_LEN {
            return Err(UnitRefusal::WrongEncodingLength);
        }
        if bytes[0] != 1 {
            return Err(UnitRefusal::UnsupportedEncodingVersion(bytes[0]));
        }
        let base = CatalogUnit::decode(&bytes[1..2]).map_err(UnitRefusal::InvalidBase)?;
        let prefix_exponent = bytes[2] as i8;
        if prefix_exponent != 0
            && (PrefixableUnit::for_catalogue(base).is_none()
                || !DECIMAL_PREFIXES
                    .iter()
                    .any(|prefix| prefix.exponent() == prefix_exponent))
        {
            return Err(UnitRefusal::InvalidPrefix);
        }
        let value = Self {
            base,
            prefix_exponent,
        };
        // An unprefixed catalogue index may itself denote a prefixed symbol.
        // Normalize against static catalogue spelling without allocating during
        // runtime admission. Nonzero prefixes already have checked base slots.
        if prefix_exponent == 0 && Self::from_catalogue(base) != value {
            return Err(UnitRefusal::NonCanonicalEncoding);
        }
        Ok(value)
    }
}

impl serde::Serialize for Unit {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.encode(), serializer)
    }
}
impl<'de> serde::Deserialize<'de> for Unit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = <[u8; UNIT_ENCODED_LEN] as serde::Deserialize>::deserialize(deserializer)?;
        Self::decode(&bytes).map_err(|_| serde::de::Error::custom("invalid canonical Unit"))
    }
}
