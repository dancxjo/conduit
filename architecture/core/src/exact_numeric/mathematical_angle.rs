//! Exact Angle representation using the existing Unit law and mathematical DAG.
//! This does not alter the decimal Quantity/Angle codec or introduce a Unit law.
use crate::{
    ExactNumericWork, MathematicalValue, MathematicalValueRefusal, MathematicalValueView,
    QuantityRole, Unit, UnitRefusal, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES, UNIT_ENCODED_LEN,
};

pub const MATHEMATICAL_ANGLE_ID: &str = "numeric/mathematical-angle@1";
const PREFIX: usize = 1 + UNIT_ENCODED_LEN;
pub const MATHEMATICAL_ANGLE_MAX_ENCODED_BYTES: usize =
    PREFIX + MATHEMATICAL_VALUE_MAX_ENCODED_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalAngleRefusal {
    Truncated,
    UnsupportedVersion,
    UnsupportedCoordinateUnit,
    Unit(UnitRefusal),
    Coordinate(MathematicalValueRefusal),
}
/// Turn coordinates are explicit and immutable. Exact meaning is retained even
/// when a selected numerical realization cannot satisfy a consumer's profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathematicalAngle {
    coordinate: MathematicalValue,
}
impl MathematicalAngle {
    pub fn from_turn_coordinate(coordinate: MathematicalValue) -> Self {
        Self { coordinate }
    }
    pub fn coordinate(&self) -> &MathematicalValue {
        &self.coordinate
    }
    pub const fn unit(&self) -> Unit {
        Unit::Turn
    }
    pub const fn role(&self) -> QuantityRole {
        QuantityRole::Linear
    }
    /// The caller supplies admitted storage; this writer never grows it.
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, MathematicalAngleRefusal> {
        write_prefix(output)?;
        let length = self
            .coordinate
            .encode(&mut output[PREFIX..])
            .map_err(MathematicalAngleRefusal::Coordinate)?;
        Ok(PREFIX + length)
    }
    /// Owning preparation decode; runtime validation uses the borrowed view.
    pub fn decode(
        encoded: &[u8],
        work: &mut ExactNumericWork,
    ) -> Result<Self, MathematicalAngleRefusal> {
        let bytes = coordinate_bytes(encoded)?;
        Ok(Self::from_turn_coordinate(
            MathematicalValue::decode(bytes, work).map_err(MathematicalAngleRefusal::Coordinate)?,
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MathematicalAngleView<'a> {
    encoded: &'a [u8],
    coordinate: MathematicalValueView<'a>,
}
impl<'a> MathematicalAngleView<'a> {
    pub fn decode(
        encoded: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<Self, MathematicalAngleRefusal> {
        let bytes = coordinate_bytes(encoded)?;
        let coordinate = MathematicalValueView::decode(bytes, work)
            .map_err(MathematicalAngleRefusal::Coordinate)?;
        Ok(Self {
            encoded,
            coordinate,
        })
    }
    pub const fn unit(&self) -> Unit {
        Unit::Turn
    }
    pub const fn role(&self) -> QuantityRole {
        QuantityRole::Linear
    }
    pub const fn coordinate(&self) -> &MathematicalValueView<'a> {
        &self.coordinate
    }
    pub const fn encoded(&self) -> &'a [u8] {
        self.encoded
    }
    pub fn semantic_digest(&self) -> [u8; 32] {
        crate::semantic_digest(MATHEMATICAL_ANGLE_ID, self.encoded)
    }
}
fn coordinate_bytes(encoded: &[u8]) -> Result<&[u8], MathematicalAngleRefusal> {
    use MathematicalAngleRefusal as R;
    if encoded.len() <= PREFIX {
        return Err(R::Truncated);
    }
    if encoded[0] != 1 {
        return Err(R::UnsupportedVersion);
    }
    if Unit::decode(&encoded[1..PREFIX]).map_err(R::Unit)? != Unit::Turn {
        return Err(R::UnsupportedCoordinateUnit);
    }
    Ok(&encoded[PREFIX..])
}

pub(super) fn write_prefix(output: &mut [u8]) -> Result<usize, MathematicalAngleRefusal> {
    if output.len() < PREFIX {
        return Err(MathematicalAngleRefusal::Truncated);
    }
    output[0] = 1;
    output[1..PREFIX].copy_from_slice(&Unit::Turn.encode());
    Ok(PREFIX)
}
