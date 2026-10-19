//! Proposed optional Quantity coordinate form. No legacy port admission is added.
//! Unit definitions remain checked data, not authority to define or bind a Unit.
use crate::{
    ExactNumericRefusal as N, ExactNumericWork, MathematicalValueRefusal, MathematicalValueView,
    QuantityRole, QuantityValueContractRefusal, RationalQuantity, RationalQuantityRefusal, Unit,
    UnitRefusal, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES, UNIT_ENCODED_LEN,
};
pub const MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES: usize =
    2 + UNIT_ENCODED_LEN + MATHEMATICAL_VALUE_MAX_ENCODED_BYTES;
const HEADER: usize = 2 + UNIT_ENCODED_LEN;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalQuantityRefusal {
    Truncated,
    ExtentExceeded,
    UnsupportedVersion(u8),
    InvalidRole,
    Unit(UnitRefusal),
    Coordinate(MathematicalValueRefusal),
    Numeric(N),
    NonRationalCoordinate,
    Rational(RationalQuantityRefusal),
}
use MathematicalQuantityRefusal as R;
/// Borrowed, validated mathematical coordinates with the existing physical Unit
/// and role. Source spelling and conversion evidence remain separate custody.
#[derive(Debug, Clone, Copy)]
pub struct MathematicalQuantityView<'a> {
    encoded: &'a [u8],
    unit: Unit,
    role: QuantityRole,
    coordinate: MathematicalValueView<'a>,
}
impl<'a> MathematicalQuantityView<'a> {
    pub fn decode(encoded: &'a [u8], work: &mut ExactNumericWork) -> Result<Self, R> {
        work.charge(1).map_err(R::Numeric)?;
        let Some(version) = encoded.first() else {
            return Err(R::Truncated);
        };
        if *version != 3 {
            return Err(R::UnsupportedVersion(*version));
        }
        if encoded.len() > MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES {
            return Err(R::ExtentExceeded);
        }
        if encoded.len() < HEADER + 4 {
            return Err(R::Truncated);
        }
        let role = match encoded[1] {
            0 => QuantityRole::Linear,
            1 => QuantityRole::Point,
            2 => QuantityRole::Delta,
            _ => return Err(R::InvalidRole),
        };
        let unit = Unit::decode(&encoded[2..HEADER]).map_err(R::Unit)?;
        if !unit.admits_role(role) {
            return Err(R::InvalidRole);
        }
        let coordinate =
            MathematicalValueView::decode(&encoded[HEADER..], work).map_err(R::Coordinate)?;
        Ok(Self {
            encoded,
            unit,
            role,
            coordinate,
        })
    }
    pub const fn unit(&self) -> Unit {
        self.unit
    }
    pub const fn role(&self) -> QuantityRole {
        self.role
    }
    pub const fn dimension(&self) -> crate::DimensionDefinition {
        self.unit.dimension()
    }
    pub const fn family(&self) -> crate::QuantityFamilyDefinition {
        self.unit.family()
    }
    pub const fn encoded(&self) -> &'a [u8] {
        self.encoded
    }
    pub const fn coordinate(&self) -> &MathematicalValueView<'a> {
        &self.coordinate
    }
    pub fn validate_kind(&self, identity: &str) -> Result<(), QuantityValueContractRefusal> {
        super::value_profile::validate_quantity_kind(self.unit, self.role, identity)
    }
    /// Explicit exact materialization only. An unresolved symbolic coordinate
    /// never becomes a rounded rational or a source with its Unit erased.
    pub fn rational(&self, work: &mut ExactNumericWork) -> Result<RationalQuantity, R> {
        let coordinate = self
            .coordinate
            .as_rational(work)
            .map_err(R::Coordinate)?
            .ok_or(R::NonRationalCoordinate)?;
        RationalQuantity::new(coordinate, self.unit, self.role).map_err(R::Rational)
    }
    /// Coordinate representation identity, not physical equality across Units.
    pub fn representation_digest(&self) -> [u8; 32] {
        crate::semantic_digest(crate::QUANTITY_INFO_ID, self.encoded)
    }
}
/// Encode already validated mathematical data into caller-admitted storage.
/// All failures precede output mutation; work includes the finite frame copy.
pub fn encode_mathematical_quantity_value(
    coordinate: &MathematicalValueView<'_>,
    unit: Unit,
    role: QuantityRole,
    output: &mut [u8],
    work: &mut ExactNumericWork,
) -> Result<usize, R> {
    if !unit.admits_role(role) {
        return Err(R::InvalidRole);
    }
    let n = HEADER
        .checked_add(coordinate.encoded().len())
        .ok_or(R::ExtentExceeded)?;
    if n > MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES {
        return Err(R::ExtentExceeded);
    }
    if output.len() < n {
        return Err(R::Numeric(N::EncodingBufferTooSmall));
    }
    work.charge(n as u64).map_err(R::Numeric)?;
    output[0] = 3;
    output[1] = match role {
        QuantityRole::Linear => 0,
        QuantityRole::Point => 1,
        QuantityRole::Delta => 2,
    };
    output[2..HEADER].copy_from_slice(&unit.encode());
    output[HEADER..n].copy_from_slice(coordinate.encoded());
    Ok(n)
}
