//! Borrowed evidence paired with a standalone admitted Unit descriptor.
use crate::{Unit, UnitRefusal};
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ResolvedQuantitySuffix<'a> {
    source: &'a str,
    unit: Unit,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantitySuffixRefusal {
    Unit(UnitRefusal),
    EvidenceMismatch,
}
impl<'a> ResolvedQuantitySuffix<'a> {
    pub fn resolve(source: &'a str) -> Result<Self, QuantitySuffixRefusal> {
        Self::from_unit(
            source,
            Unit::resolve(source).map_err(QuantitySuffixRefusal::Unit)?,
        )
    }
    pub fn from_unit(source: &'a str, unit: Unit) -> Result<Self, QuantitySuffixRefusal> {
        if !unit.matches_source_evidence(source) {
            return Err(QuantitySuffixRefusal::EvidenceMismatch);
        }
        Ok(Self { source, unit })
    }
    pub const fn source(self) -> &'a str {
        self.source
    }
    pub const fn unit(self) -> Unit {
        self.unit
    }
}
