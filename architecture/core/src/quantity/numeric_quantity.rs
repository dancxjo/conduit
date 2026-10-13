//! Canonical exact mathematical coordinates with captured immutable unit law.
use crate::{MAX_UNIT_DEFINITION_BYTES, UnitDefinition, UnitDefinitionRef};
use crate::{MathValue, MathValueRef, NumericRefusal, PortableNumericProfile, QuantityRole};
use alloc::{boxed::Box, vec::Vec};
const PROFILE_START: usize = 2;
const COORDINATE_LENGTH_START: usize = PROFILE_START + PortableNumericProfile::ENCODED_LEN;
const UNIT_LENGTH_START: usize = COORDINATE_LENGTH_START + 4;
const VALUE_START: usize = UNIT_LENGTH_START + 4;
pub const MAX_QUANTITY_ENCODED_BYTES: usize =
    VALUE_START + crate::MAX_NUMERIC_ENCODED_BYTES + MAX_UNIT_DEFINITION_BYTES + 32;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityNumericRefusal {
    Numeric(NumericRefusal),
    Unit(crate::PhysicalDefinitionRefusal),
    InvalidRole,
    WrongEncodingLength,
    UnsupportedVersion,
    IdentityMismatch,
    IncompatibleDimension,
    IncompatibleFamily,
    IncompatibleRole,
    UnlinkedOrigin,
    EqualityUnknown,
}
impl From<NumericRefusal> for QuantityNumericRefusal {
    fn from(value: NumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
impl From<crate::PhysicalDefinitionRefusal> for QuantityNumericRefusal {
    fn from(value: crate::PhysicalDefinitionRefusal) -> Self {
        Self::Unit(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Quantity {
    capsule: Box<[u8]>,
}
#[derive(Debug, Copy, Clone)]
pub struct QuantityRef<'a> {
    bytes: &'a [u8],
    coordinate_end: usize,
    unit_end: usize,
    unit: UnitDefinitionRef<'a>,
}
impl<'a> QuantityRef<'a> {
    pub fn validate(bytes: &'a [u8]) -> Result<Self, QuantityNumericRefusal> {
        if bytes.len() < VALUE_START + 10 + 32 || bytes.len() > MAX_QUANTITY_ENCODED_BYTES {
            return Err(QuantityNumericRefusal::WrongEncodingLength);
        }
        if bytes[0] != 2 {
            return Err(QuantityNumericRefusal::UnsupportedVersion);
        }
        let role = match bytes[1] {
            0 => QuantityRole::Linear,
            1 => QuantityRole::Point,
            2 => QuantityRole::Delta,
            _ => return Err(QuantityNumericRefusal::InvalidRole),
        };
        let profile =
            PortableNumericProfile::decode(&bytes[PROFILE_START..COORDINATE_LENGTH_START])?;
        let read = |at: usize| {
            u32::from_le_bytes(bytes[at..at + 4].try_into().expect("checked header")) as usize
        };
        let coordinate_end = VALUE_START
            .checked_add(read(COORDINATE_LENGTH_START))
            .ok_or(QuantityNumericRefusal::WrongEncodingLength)?;
        let unit_end = coordinate_end
            .checked_add(read(UNIT_LENGTH_START))
            .ok_or(QuantityNumericRefusal::WrongEncodingLength)?;
        if unit_end.checked_add(32) != Some(bytes.len()) {
            return Err(QuantityNumericRefusal::WrongEncodingLength);
        }
        profile.validate(
            bytes
                .get(VALUE_START..coordinate_end)
                .ok_or(QuantityNumericRefusal::WrongEncodingLength)?,
        )?;
        let unit = UnitDefinitionRef::validate(
            bytes
                .get(coordinate_end..unit_end)
                .ok_or(QuantityNumericRefusal::WrongEncodingLength)?,
        )?;
        if !unit.admits_role(role) {
            return Err(QuantityNumericRefusal::InvalidRole);
        }
        if crate::semantic_digest_parts(
            "value/quantity@2",
            &[
                &bytes[1..2],
                &bytes[VALUE_START..coordinate_end],
                &unit.identity(),
            ],
        ) != bytes[unit_end..]
        {
            return Err(QuantityNumericRefusal::IdentityMismatch);
        }
        Ok(Self {
            bytes,
            coordinate_end,
            unit_end,
            unit,
        })
    }
    pub fn encoded(self) -> &'a [u8] {
        self.bytes
    }
    pub fn coordinate(self) -> MathValueRef<'a> {
        MathValueRef::validate(&self.bytes[VALUE_START..self.coordinate_end])
            .expect("checked coordinate")
    }
    pub fn coordinate_profile(self) -> PortableNumericProfile {
        PortableNumericProfile::decode(&self.bytes[PROFILE_START..COORDINATE_LENGTH_START])
            .expect("checked coordinate profile")
    }
    pub fn physical_profile(self) -> crate::PhysicalNumericProfile {
        crate::PhysicalNumericProfile {
            coordinate: self.coordinate_profile(),
            unit_law: self.unit.law_profile(),
        }
    }
    /// Both captured admission budgets and actual wire facts must fit the
    /// required port. Reprofiling is an explicit preparation operation.
    pub fn validate_profile(
        self,
        required: crate::PhysicalNumericProfile,
    ) -> Result<(), QuantityNumericRefusal> {
        required.checked()?;
        if !required.admits_profile(self.physical_profile()) {
            return Err(QuantityNumericRefusal::Numeric(
                NumericRefusal::ProfileViolation,
            ));
        }
        required.coordinate.validate(self.coordinate().encoded())?;
        required
            .unit_law
            .validate(self.unit.exact_scale().encoded())?;
        required
            .unit_law
            .validate(self.unit.exact_point_offset().encoded())?;
        Ok(())
    }
    pub const fn unit(self) -> UnitDefinitionRef<'a> {
        self.unit
    }
    pub fn role(self) -> QuantityRole {
        match self.bytes[1] {
            0 => QuantityRole::Linear,
            1 => QuantityRole::Point,
            _ => QuantityRole::Delta,
        }
    }
    pub fn identity(self) -> [u8; 32] {
        self.bytes[self.unit_end..]
            .try_into()
            .expect("checked quantity identity")
    }
    pub fn to_owned(self) -> Quantity {
        Quantity {
            capsule: self.bytes.into(),
        }
    }
    /// Exact preparation operation. Arithmetic may allocate owned symbolic DAGs.
    pub fn reference_coordinate(self) -> Result<MathValue, QuantityNumericRefusal> {
        let value = self.coordinate().to_owned()?;
        let scale = self.unit.exact_scale().to_owned()?;
        let offset = if self.role() == QuantityRole::Point {
            self.unit.exact_point_offset().to_owned()?
        } else {
            MathValue::ZERO
        };
        Ok(value.checked_mul(&scale)?.checked_add(&offset)?)
    }
    fn comparable(self, rhs: Self) -> Result<(), QuantityNumericRefusal> {
        if self.unit.family().dimension() != rhs.unit.family().dimension() {
            return Err(QuantityNumericRefusal::IncompatibleDimension);
        }
        if self.unit.family().identity() != rhs.unit.family().identity() {
            return Err(QuantityNumericRefusal::IncompatibleFamily);
        }
        if self.role() != rhs.role() {
            return Err(QuantityNumericRefusal::IncompatibleRole);
        }
        if self.unit.reference_anchor() != rhs.unit.reference_anchor() {
            return Err(QuantityNumericRefusal::UnlinkedOrigin);
        }
        Ok(())
    }
    pub fn compare(self, rhs: Self) -> Result<Option<core::cmp::Ordering>, QuantityNumericRefusal> {
        self.comparable(rhs)?;
        Ok(self
            .reference_coordinate()?
            .compare(&rhs.reference_coordinate()?)?)
    }
    /// Allocation-free exact rational comparison. Symbolic equality already
    /// established by identical captured laws/coordinates is cheap; otherwise
    /// symbolic ordering requires a separately selected computation strategy.
    pub fn compare_borrowed(
        self,
        rhs: Self,
    ) -> Result<Option<core::cmp::Ordering>, QuantityNumericRefusal> {
        self.comparable(rhs)?;
        if self.coordinate().encoded() == rhs.coordinate().encoded()
            && self.unit.exact_scale().encoded() == rhs.unit.exact_scale().encoded()
            && (self.role() != QuantityRole::Point
                || self.unit.exact_point_offset().encoded()
                    == rhs.unit.exact_point_offset().encoded())
        {
            return Ok(Some(core::cmp::Ordering::Equal));
        }
        fn reference(
            value: QuantityRef<'_>,
        ) -> Result<Option<crate::ExactRational>, QuantityNumericRefusal> {
            let (Some(coordinate), Some(scale)) = (
                value.coordinate().exact_rational()?,
                value.unit.exact_scale().exact_rational()?,
            ) else {
                return Ok(None);
            };
            let scaled = coordinate.checked_mul(&scale)?;
            if value.role() != QuantityRole::Point {
                return Ok(Some(scaled));
            }
            let Some(offset) = value.unit.exact_point_offset().exact_rational()? else {
                return Ok(None);
            };
            Ok(Some(scaled.checked_add(&offset)?))
        }
        match (reference(self)?, reference(rhs)?) {
            (Some(a), Some(b)) => Ok(Some(a.compare(&b)?)),
            _ => Ok(None),
        }
    }
    /// Checked allocation-free projection for explicitly selected finite integer consumers.
    pub fn project_i64_in_unit(
        self,
        target: UnitDefinitionRef<'_>,
    ) -> Result<i64, crate::QuantityConversionRefusal> {
        self.projected_rational_in_unit(target)?
            .checked_i64()
            .map_err(integer_projection_refusal)
    }
    pub fn project_u64_in_unit(
        self,
        target: UnitDefinitionRef<'_>,
    ) -> Result<u64, crate::QuantityConversionRefusal> {
        self.projected_rational_in_unit(target)?
            .checked_u64()
            .map_err(integer_projection_refusal)
    }
    fn projected_rational_in_unit(
        self,
        target: UnitDefinitionRef<'_>,
    ) -> Result<crate::ExactRational, crate::QuantityConversionRefusal> {
        use crate::QuantityConversionRefusal as R;
        if self.unit.family().dimension() != target.family().dimension() {
            return Err(R::IncompatibleDimensions);
        }
        if self.unit.family().identity() != target.family().identity() {
            return Err(R::IncompatibleQuantityFamilies);
        }
        if !target.admits_role(self.role()) {
            return Err(R::IncompatibleQuantityRoles);
        }
        if self.unit.reference_anchor() != target.reference_anchor() {
            return Err(R::UnlinkedOrigin);
        }
        if self.unit.exact_scale().encoded() == target.exact_scale().encoded()
            && (self.role() != QuantityRole::Point
                || self.unit.exact_point_offset().encoded()
                    == target.exact_point_offset().encoded())
        {
            return self
                .coordinate()
                .exact_rational()
                .map_err(R::Numeric)?
                .ok_or(R::Inexact);
        }
        let rational =
            |v: MathValueRef<'_>| v.exact_rational().map_err(R::Numeric)?.ok_or(R::Inexact);
        let coordinate = rational(self.coordinate())?;
        let mut reference = coordinate
            .checked_mul(&rational(self.unit.exact_scale())?)
            .map_err(R::Numeric)?;
        if self.role() == QuantityRole::Point {
            reference = reference
                .checked_add(&rational(self.unit.exact_point_offset())?)
                .map_err(R::Numeric)?
                .checked_add(&rational(target.exact_point_offset())?.negated())
                .map_err(R::Numeric)?;
        }
        reference
            .checked_div(&rational(target.exact_scale())?)
            .map_err(R::Numeric)
    }
    pub fn convert(
        self,
        target: UnitDefinitionRef<'_>,
    ) -> Result<Quantity, QuantityNumericRefusal> {
        if self.unit.family().dimension() != target.family().dimension() {
            return Err(QuantityNumericRefusal::IncompatibleDimension);
        }
        if self.unit.family().identity() != target.family().identity() {
            return Err(QuantityNumericRefusal::IncompatibleFamily);
        }
        if !target.admits_role(self.role()) {
            return Err(QuantityNumericRefusal::IncompatibleRole);
        }
        if self.unit.reference_anchor() != target.reference_anchor() {
            return Err(QuantityNumericRefusal::UnlinkedOrigin);
        }
        let offset = if self.role() == QuantityRole::Point {
            target.exact_point_offset().to_owned()?
        } else {
            MathValue::ZERO
        };
        let coordinate = self
            .reference_coordinate()?
            .checked_sub(&offset)?
            .checked_div(&target.exact_scale().to_owned()?)?;
        Quantity::from_math_role(coordinate, &target.to_owned(), self.role())
    }
}
impl Quantity {
    /// Write an admitted coordinate and captured unit into canonical storage without allocation.
    pub fn encode_coordinate_into(
        coordinate: MathValueRef<'_>,
        unit: UnitDefinitionRef<'_>,
        role: QuantityRole,
        profile: PortableNumericProfile,
        output: &mut [u8],
    ) -> Result<usize, QuantityNumericRefusal> {
        if !unit.admits_role(role) {
            return Err(QuantityNumericRefusal::InvalidRole);
        }
        profile.validate(coordinate.encoded())?;
        let coordinate_end = VALUE_START + coordinate.encoded().len();
        let unit_end = coordinate_end + unit.encoded().len();
        let length = unit_end + 32;
        if output.len() < length {
            return Err(NumericRefusal::OutputTooSmall.into());
        }
        output[0] = 2;
        output[1] = role as u8;
        output[PROFILE_START..COORDINATE_LENGTH_START].copy_from_slice(&profile.encode()?);
        output[COORDINATE_LENGTH_START..UNIT_LENGTH_START]
            .copy_from_slice(&(coordinate.encoded().len() as u32).to_le_bytes());
        output[UNIT_LENGTH_START..VALUE_START]
            .copy_from_slice(&(unit.encoded().len() as u32).to_le_bytes());
        output[VALUE_START..coordinate_end].copy_from_slice(coordinate.encoded());
        output[coordinate_end..unit_end].copy_from_slice(unit.encoded());
        let identity = crate::semantic_digest_parts(
            "value/quantity@2",
            &[&output[1..2], coordinate.encoded(), &unit.identity()],
        );
        output[unit_end..length].copy_from_slice(&identity);
        Ok(length)
    }

    pub fn encode_integer_into(
        coordinate: i64,
        unit: UnitDefinitionRef<'_>,
        role: QuantityRole,
        profile: PortableNumericProfile,
        output: &mut [u8],
    ) -> Result<usize, QuantityNumericRefusal> {
        let mut encoded = [0; 10];
        encoded[..2].copy_from_slice(&[1, 1]);
        encoded[2..].copy_from_slice(&coordinate.to_le_bytes());
        Self::encode_coordinate_into(
            MathValueRef::validate(&encoded)?,
            unit,
            role,
            profile,
            output,
        )
    }

    pub fn from_math_profile(
        value: MathValue,
        unit: UnitDefinitionRef<'_>,
        role: QuantityRole,
        profile: PortableNumericProfile,
    ) -> Result<Self, QuantityNumericRefusal> {
        if !unit.admits_role(role) {
            return Err(QuantityNumericRefusal::InvalidRole);
        }
        let coordinate = value.canonical_bytes()?;
        profile.validate(&coordinate)?;
        let mut bytes =
            Vec::with_capacity(VALUE_START + coordinate.len() + unit.encoded().len() + 32);
        bytes.push(2);
        bytes.push(role as u8);
        bytes.extend_from_slice(&profile.encode()?);
        bytes.extend_from_slice(&(coordinate.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(unit.encoded().len() as u32).to_le_bytes());
        bytes.extend_from_slice(&coordinate);
        bytes.extend_from_slice(unit.encoded());
        let identity = crate::semantic_digest_parts(
            "value/quantity@2",
            &[&bytes[1..2], &coordinate, &unit.identity()],
        );
        bytes.extend_from_slice(&identity);
        QuantityRef::validate(&bytes)?;
        Ok(Self {
            capsule: bytes.into_boxed_slice(),
        })
    }
    pub fn from_math(
        value: MathValue,
        unit: &UnitDefinition,
    ) -> Result<Self, QuantityNumericRefusal> {
        Self::from_math_role(value, unit, unit.declared_role())
    }
    pub fn from_math_role(
        value: MathValue,
        unit: &UnitDefinition,
        role: QuantityRole,
    ) -> Result<Self, QuantityNumericRefusal> {
        {
            let bytes = value.canonical_bytes()?;
            let profile = PortableNumericProfile::for_value(MathValueRef::validate(&bytes)?);
            Self::from_math_profile(value, unit.as_ref(), role, profile)
        }
    }
    pub fn encoded(&self) -> &[u8] {
        &self.capsule
    }
    pub fn as_ref(&self) -> QuantityRef<'_> {
        QuantityRef::validate(self.encoded()).expect("checked quantity ownership")
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, QuantityNumericRefusal> {
        Ok(QuantityRef::validate(bytes)?.to_owned())
    }
    pub fn encode(&self) -> Vec<u8> {
        self.encoded().to_vec()
    }
    pub fn encode_into(&self, out: &mut [u8]) -> Result<usize, QuantityNumericRefusal> {
        if out.len() < self.encoded().len() {
            return Err(QuantityNumericRefusal::WrongEncodingLength);
        }
        out[..self.encoded().len()].copy_from_slice(self.encoded());
        Ok(self.encoded().len())
    }
    pub fn coordinate(&self) -> MathValueRef<'_> {
        self.as_ref().coordinate()
    }
    pub fn unit(&self) -> UnitDefinitionRef<'_> {
        self.as_ref().unit()
    }
    pub fn role(&self) -> QuantityRole {
        self.as_ref().role()
    }
}

fn integer_projection_refusal(
    error: crate::NumericProjectionRefusal,
) -> crate::QuantityConversionRefusal {
    use crate::{NumericProjectionRefusal as P, QuantityConversionRefusal as R};
    match error {
        P::Overflow => R::Overflow,
        P::Budget(n) => R::Numeric(n),
        _ => R::Inexact,
    }
}
