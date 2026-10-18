//! Optional rational coordinate storage, reusing the admitted physical Unit law.
//! The existing decimal Quantity codec and native paths retain their contracts.
use super::{ExactNumericRefusal, ExactNumericWork, ExactRational};
use crate::{
    DefinitionScalar, DimensionDefinition, Quantity, QuantityConversionRefusal,
    QuantityFamilyDefinition, QuantityRole, Unit, UnitRefusal,
};
use core::cmp::Ordering;

mod arithmetic;
mod enclosure;
mod product;
mod projection;
mod receipt;
mod value_profile;
mod value_view;
pub use enclosure::*;
pub use projection::*;
pub use receipt::*;
pub use value_profile::*;
pub use value_view::*;

pub const RATIONAL_QUANTITY_ID: &str = "numeric/rational-quantity@1";
pub const RATIONAL_QUANTITY_MAX_ENCODED_BYTES: usize =
    2 + crate::UNIT_ENCODED_LEN + super::EXACT_RATIONAL_MAX_ENCODED_BYTES;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RationalQuantityRefusal {
    Physical(QuantityConversionRefusal),
    Numeric(ExactNumericRefusal),
    Unit(UnitRefusal),
    InvalidRole,
    NonCanonicalEncoding,
    UnsupportedVersion,
}
impl From<ExactNumericRefusal> for RationalQuantityRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Numeric(value)
    }
}

/// This is a numerical representation of a Quantity, not a new family or a
/// Unit-definition authority. Coordinate equality and physical equality differ.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RationalQuantity {
    coordinate: ExactRational,
    unit: Unit,
    role: QuantityRole,
}
impl RationalQuantity {
    pub fn new(
        coordinate: ExactRational,
        unit: Unit,
        role: QuantityRole,
    ) -> Result<Self, RationalQuantityRefusal> {
        if !unit.admits_role(role) {
            return Err(RationalQuantityRefusal::InvalidRole);
        }
        Ok(Self {
            coordinate,
            unit,
            role,
        })
    }
    pub fn from_quantity(
        source: Quantity,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        Self::new(
            ExactRational::from_decimal_parts(source.coefficient(), source.exponent(), work)?,
            source.unit(),
            source.role(),
        )
    }
    pub const fn coordinate(self) -> ExactRational {
        self.coordinate
    }
    pub const fn unit(self) -> Unit {
        self.unit
    }
    pub const fn role(self) -> QuantityRole {
        self.role
    }
    pub const fn dimension(self) -> DimensionDefinition {
        self.unit.dimension()
    }
    pub const fn family(self) -> QuantityFamilyDefinition {
        self.unit.family()
    }

    fn reference_coordinate(
        self,
        work: &mut ExactNumericWork,
    ) -> Result<ExactRational, RationalQuantityRefusal> {
        let offset = self
            .unit
            .exact_offset(self.role)
            .map_err(|_| RationalQuantityRefusal::InvalidRole)?;
        Ok(self
            .coordinate
            .checked_multiply(scale(self.unit, work)?, work)?
            .checked_add(scalar(offset, work)?, work)?)
    }
    /// Unit coordinates and numerical representation are independent choices.
    /// Nonterminating rational coordinates survive; no float or rounding occurs.
    pub fn convert_to_unit(
        self,
        target: Unit,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        crate::quantity::compatible_units(self.unit, self.role, target)
            .map_err(RationalQuantityRefusal::Physical)?;
        if self.unit == target {
            // Preserve the already admitted coordinate instead of forming
            // unnecessary scale/offset intermediates. Charge the fixed storage
            // of both rational limbs; identity is bounded work, not free fuel.
            work.charge((2 * super::EXACT_NUMERIC_MAX_BITS / 32) as u64)?;
            return Ok(self);
        }
        let offset = target
            .exact_offset(self.role)
            .map_err(|_| RationalQuantityRefusal::InvalidRole)?;
        let coordinate = self
            .reference_coordinate(work)?
            .checked_subtract(scalar(offset, work)?, work)?
            .checked_divide(scale(target, work)?, work)?;
        Self::new(coordinate, target, self.role)
    }
    pub fn compare(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<Ordering, RationalQuantityRefusal> {
        crate::quantity::compatible_units(self.unit, self.role, other.unit)
            .map_err(RationalQuantityRefusal::Physical)?;
        if self.role != other.role {
            return Err(RationalQuantityRefusal::Physical(
                QuantityConversionRefusal::IncompatibleQuantityRoles,
            ));
        }
        Ok(self
            .reference_coordinate(work)?
            .checked_compare(other.reference_coordinate(work)?, work)?)
    }
    /// Version, exact role, complete Unit capsule, then the canonical rational.
    /// Decode validates every capsule; its digest grants no admission authority.
    pub fn encode(self, output: &mut [u8]) -> Result<usize, RationalQuantityRefusal> {
        let header = 2 + crate::UNIT_ENCODED_LEN;
        if output.len() < header {
            return Err(ExactNumericRefusal::EncodingBufferTooSmall.into());
        }
        output[0] = 1;
        output[1] = match self.role {
            QuantityRole::Linear => 0,
            QuantityRole::Point => 1,
            QuantityRole::Delta => 2,
        };
        output[2..header].copy_from_slice(&self.unit.encode());
        Ok(header + self.coordinate.encode(&mut output[header..])?)
    }
    pub fn decode(
        bytes: &[u8],
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        let header = 2 + crate::UNIT_ENCODED_LEN;
        if bytes.len() < header + 6 || bytes.len() > RATIONAL_QUANTITY_MAX_ENCODED_BYTES {
            return Err(RationalQuantityRefusal::NonCanonicalEncoding);
        }
        if bytes[0] != 1 {
            return Err(RationalQuantityRefusal::UnsupportedVersion);
        }
        let role = match bytes[1] {
            0 => QuantityRole::Linear,
            1 => QuantityRole::Point,
            2 => QuantityRole::Delta,
            _ => return Err(RationalQuantityRefusal::InvalidRole),
        };
        let unit = Unit::decode(&bytes[2..header]).map_err(RationalQuantityRefusal::Unit)?;
        Self::new(ExactRational::decode(&bytes[header..], work)?, unit, role)
    }
    /// Structural identity retains the coordinate's Unit declaration. Use
    /// checked physical comparison for equality across different coordinates.
    pub fn semantic_digest(self) -> [u8; 32] {
        let mut bytes = [0; RATIONAL_QUANTITY_MAX_ENCODED_BYTES];
        let length = self
            .encode(&mut bytes)
            .expect("fixed rational Quantity extent");
        crate::semantic_digest(RATIONAL_QUANTITY_ID, &bytes[..length])
    }
}
fn scalar(
    value: DefinitionScalar,
    work: &mut ExactNumericWork,
) -> Result<ExactRational, ExactNumericRefusal> {
    ExactRational::ratio(value.numerator, value.denominator, work)?.checked_multiply(
        ExactRational::from_i128(10).checked_power(value.decimal_exponent, work)?,
        work,
    )
}
fn scale(unit: Unit, work: &mut ExactNumericWork) -> Result<ExactRational, ExactNumericRefusal> {
    // Unit admission already constrains this exponent; avoid a narrowing cast.
    let exponent =
        i16::try_from(unit.binary_exponent()).map_err(|_| ExactNumericRefusal::ExtentExceeded)?;
    scalar(unit.exact_scale(), work)?.checked_multiply(
        ExactRational::from_i128(2).checked_power(exponent, work)?,
        work,
    )
}
