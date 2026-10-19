//! Optional exact coordinate transport for the existing physical Quantity law.
//! Decimal v1 remains byte-for-byte unchanged. Rational v2 reuses the existing
//! RationalQuantity codec and grants no Unit-definition or binding authority.
use crate::{
    ExactNumericRefusal, ExactNumericWork, ExactRational, Quantity, QuantityRefusal, QuantityRole,
    RationalQuantity, RationalQuantityRefusal, Unit, UNIT_ENCODED_LEN,
};

pub const QUANTITY_VALUE_MAX_ENCODED_BYTES: usize = crate::RATIONAL_QUANTITY_MAX_ENCODED_BYTES;
const HEADER: usize = 2 + UNIT_ENCODED_LEN;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityCoordinateRepresentation {
    Decimal,
    Rational,
    Mathematical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityValueRefusal {
    Truncated,
    ExtentExceeded,
    UnsupportedVersion(u8),
    Decimal(QuantityRefusal),
    Rational(RationalQuantityRefusal),
    Mathematical(super::MathematicalQuantityRefusal),
}
impl From<RationalQuantityRefusal> for QuantityValueRefusal {
    fn from(value: RationalQuantityRefusal) -> Self {
        Self::Rational(value)
    }
}
impl From<ExactNumericRefusal> for QuantityValueRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Rational(value.into())
    }
}
#[derive(Debug, Clone, Copy)]
enum Coordinate<'a> {
    Decimal { coefficient: i128, exponent: i16 },
    Rational(&'a [u8]),
    Mathematical(&'a [u8]),
}
/// Borrowed transport inspection does not retain an owning bignum in native
/// Quantity state. Numerical materialization is an explicit, metered operation.
#[derive(Debug, Clone, Copy)]
pub struct QuantityValueView<'a> {
    encoded: &'a [u8],
    unit: Unit,
    role: QuantityRole,
    coordinate: Coordinate<'a>,
}
impl<'a> QuantityValueView<'a> {
    pub fn decode(
        encoded: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<Self, QuantityValueRefusal> {
        work.charge(1)?;
        let Some(version) = encoded.first() else {
            return Err(QuantityValueRefusal::Truncated);
        };
        if encoded.len() > QUANTITY_VALUE_MAX_ENCODED_BYTES {
            return Err(QuantityValueRefusal::ExtentExceeded);
        }
        match version {
            1 => {
                let value = Quantity::decode(encoded).map_err(QuantityValueRefusal::Decimal)?;
                Ok(Self {
                    encoded,
                    unit: value.unit(),
                    role: value.role(),
                    coordinate: Coordinate::Decimal {
                        coefficient: value.coefficient(),
                        exponent: value.exponent(),
                    },
                })
            }
            2 => {
                let (unit, role) = validate_rational(encoded, work)?;
                Ok(Self {
                    encoded,
                    unit,
                    role,
                    coordinate: Coordinate::Rational(&encoded[HEADER..]),
                })
            }
            other => Err(QuantityValueRefusal::UnsupportedVersion(*other)),
        }
    }
    // Entered only after an explicitly selected mathematical wire profile.
    pub(super) fn decode_mathematical(
        encoded: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<Self, QuantityValueRefusal> {
        let value = super::MathematicalQuantityView::decode(encoded, work)
            .map_err(QuantityValueRefusal::Mathematical)?;
        Ok(Self {
            encoded,
            unit: value.unit(),
            role: value.role(),
            coordinate: Coordinate::Mathematical(value.coordinate().encoded()),
        })
    }
    pub const fn unit(self) -> Unit {
        self.unit
    }
    pub const fn role(self) -> QuantityRole {
        self.role
    }
    pub const fn dimension(self) -> crate::DimensionDefinition {
        self.unit.dimension()
    }
    pub const fn family(self) -> crate::QuantityFamilyDefinition {
        self.unit.family()
    }
    pub const fn encoded(self) -> &'a [u8] {
        self.encoded
    }
    pub const fn representation(self) -> QuantityCoordinateRepresentation {
        match self.coordinate {
            Coordinate::Decimal { .. } => QuantityCoordinateRepresentation::Decimal,
            Coordinate::Rational(_) => QuantityCoordinateRepresentation::Rational,
            Coordinate::Mathematical(_) => QuantityCoordinateRepresentation::Mathematical,
        }
    }
    /// Inspect the native representation only; this performs no numerical projection.
    pub fn decimal(self) -> Option<Quantity> {
        match self.coordinate {
            Coordinate::Decimal {
                coefficient,
                exponent,
            } => Some(
                Quantity::from_decimal_role(coefficient, exponent, self.unit, self.role)
                    .expect("validated decimal coordinate"),
            ),
            Coordinate::Rational(_) | Coordinate::Mathematical(_) => None,
        }
    }
    pub fn rational(
        self,
        work: &mut ExactNumericWork,
    ) -> Result<RationalQuantity, QuantityValueRefusal> {
        let coordinate = match self.coordinate {
            Coordinate::Decimal {
                coefficient,
                exponent,
            } => ExactRational::from_decimal_parts(coefficient, exponent, work)?,
            Coordinate::Rational(bytes) => ExactRational::decode(bytes, work)?,
            Coordinate::Mathematical(bytes) => crate::MathematicalValueView::decode(bytes, work)
                .map_err(|e| {
                    QuantityValueRefusal::Mathematical(
                        super::MathematicalQuantityRefusal::Coordinate(e),
                    )
                })?
                .as_rational(work)
                .map_err(|e| {
                    QuantityValueRefusal::Mathematical(
                        super::MathematicalQuantityRefusal::Coordinate(e),
                    )
                })?
                .ok_or(QuantityValueRefusal::Mathematical(
                    super::MathematicalQuantityRefusal::NonRationalCoordinate,
                ))?,
        };
        Ok(RationalQuantity::new(coordinate, self.unit, self.role)?)
    }
    /// Exact projection into the old decimal profile; no rounding or Unit change.
    pub fn project_decimal(
        self,
        work: &mut ExactNumericWork,
    ) -> Result<Quantity, QuantityValueRefusal> {
        if let Some(decimal) = self.decimal() {
            return Ok(decimal);
        }
        let (coefficient, exponent) = self
            .rational(work)?
            .coordinate()
            .checked_decimal_parts(work)?;
        Quantity::from_decimal_role(coefficient, exponent, self.unit, self.role)
            .map_err(QuantityValueRefusal::Decimal)
    }
    pub fn compare(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<core::cmp::Ordering, QuantityValueRefusal> {
        Ok(self.rational(work)?.compare(other.rational(work)?, work)?)
    }
    /// Representation identity, not physical equality across coordinate forms.
    pub fn representation_digest(self) -> [u8; 32] {
        crate::semantic_digest(crate::QUANTITY_INFO_ID, self.encoded)
    }
}

// Keep owning rational validation out of the decimal caller's stack frame.
// This helper is entered only for the optional rational representation.
#[inline(never)]
fn validate_rational(
    encoded: &[u8],
    work: &mut ExactNumericWork,
) -> Result<(Unit, QuantityRole), QuantityValueRefusal> {
    if encoded.len() < HEADER + 6 {
        return Err(QuantityValueRefusal::Truncated);
    }
    let role = match encoded[1] {
        0 => QuantityRole::Linear,
        1 => QuantityRole::Point,
        2 => QuantityRole::Delta,
        _ => return Err(RationalQuantityRefusal::InvalidRole.into()),
    };
    let unit = Unit::decode(&encoded[2..HEADER]).map_err(RationalQuantityRefusal::Unit)?;
    let coordinate = ExactRational::decode(&encoded[HEADER..], work)?;
    RationalQuantity::new(coordinate, unit, role)?;
    Ok((unit, role))
}
/// Write the reviewed rational representation with the Quantity v2 discriminator.
/// The caller supplies admitted storage; no allocation, rounding or unit lookup.
pub fn encode_rational_quantity_value(
    value: RationalQuantity,
    output: &mut [u8],
) -> Result<usize, QuantityValueRefusal> {
    let length = value.encode(output)?;
    output[0] = 2;
    Ok(length)
}
