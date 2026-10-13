//! Exact mathematical Quantity construction and explicit local projections.
pub use super::numeric_quantity::{
    MAX_QUANTITY_ENCODED_BYTES, Quantity, QuantityNumericRefusal, QuantityRef,
};
use crate::{MathValue, NumericRefusal, QuantityRole, Unit, UnitRefusal};
pub const QUANTITY_INFO_ID: &str = "value/quantity@2";
pub const QUANTITY_MAX_NUMBER_BYTES: usize = 96;
pub const QUANTITY_MAX_LITERAL_BYTES: usize = 384;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QuantityRefusal {
    LiteralTooLong,
    NumberTooLong,
    InvalidNumber,
    UnsupportedExponentNotation,
    InvalidRole,
    Unit(UnitRefusal),
    Numeric(NumericRefusal),
    Carrier(super::numeric_quantity::QuantityNumericRefusal),
}
impl From<NumericRefusal> for QuantityRefusal {
    fn from(value: NumericRefusal) -> Self {
        Self::Numeric(value)
    }
}
impl From<super::numeric_quantity::QuantityNumericRefusal> for QuantityRefusal {
    fn from(value: super::numeric_quantity::QuantityNumericRefusal) -> Self {
        Self::Carrier(value)
    }
}
impl Quantity {
    pub fn new(value: i64, unit: Unit) -> Self {
        Self::from_math(MathValue::integer(value), &unit).expect("bounded integer quantity")
    }
    pub fn from_decimal(
        coefficient: i128,
        exponent: i16,
        unit: Unit,
    ) -> Result<Self, QuantityRefusal> {
        Ok(Self::from_math(
            MathValue::from_decimal_parts(coefficient, exponent)?,
            &unit,
        )?)
    }
    pub fn from_decimal_role(
        coefficient: i128,
        exponent: i16,
        unit: Unit,
        role: QuantityRole,
    ) -> Result<Self, QuantityRefusal> {
        Ok(Self::from_math_role(
            MathValue::from_decimal_parts(coefficient, exponent)?,
            &unit,
            role,
        )?)
    }
    pub fn family(&self) -> crate::QuantityFamilyDefinition {
        self.unit().family()
    }
    pub fn dimension(&self) -> crate::DimensionDefinition {
        self.family().dimension()
    }
    pub fn semantic_digest(&self) -> [u8; 32] {
        crate::semantic_digest(QUANTITY_INFO_ID, &self.as_ref().identity())
    }
    pub fn convert_to_unit(
        &self,
        target: &Unit,
    ) -> Result<MathValue, crate::QuantityConversionRefusal> {
        self.as_ref()
            .convert(target.as_ref())
            .map_err(crate::QuantityConversionRefusal::from)?
            .coordinate()
            .to_owned()
            .map_err(crate::QuantityConversionRefusal::Numeric)
    }
    pub fn convert(&self, target: &Unit) -> Result<Self, crate::QuantityConversionRefusal> {
        self.as_ref().convert(target.as_ref()).map_err(Into::into)
    }
    pub fn compare(
        &self,
        other: &Self,
    ) -> Result<Option<core::cmp::Ordering>, crate::QuantityConversionRefusal> {
        self.as_ref().compare(other.as_ref()).map_err(Into::into)
    }
    pub fn to_i64(&self, target: &Unit) -> Result<i64, crate::QuantityConversionRefusal> {
        let coordinate = self.convert_to_unit(target)?;
        let bytes = coordinate
            .canonical_bytes()
            .map_err(crate::QuantityConversionRefusal::Numeric)?;
        crate::MathValueRef::validate(&bytes)
            .map_err(crate::QuantityConversionRefusal::Numeric)?
            .checked_i64()
            .map_err(|error| match error {
                crate::NumericProjectionRefusal::Overflow => {
                    crate::QuantityConversionRefusal::Overflow
                }
                crate::NumericProjectionRefusal::Budget(n) => {
                    crate::QuantityConversionRefusal::Numeric(n)
                }
                _ => crate::QuantityConversionRefusal::Inexact,
            })
    }
    pub fn to_u64(&self, target: &Unit) -> Result<u64, crate::QuantityConversionRefusal> {
        self.as_ref().project_u64_in_unit(target.as_ref())
    }
    pub fn convert_to_u64(&self, target: &Unit) -> Result<u64, crate::QuantityConversionRefusal> {
        self.to_u64(target)
    }
    pub fn matches_literal_evidence(&self, source: &str) -> bool {
        let unit = self.unit().to_owned();
        Self::parse_with_unit_evidence(source, unit, self.role())
            .is_ok_and(|value| value.coordinate().encoded() == self.coordinate().encoded())
    }
    pub fn parse_with_unit_evidence(
        source: &str,
        unit: Unit,
        role: QuantityRole,
    ) -> Result<Self, QuantityRefusal> {
        if source.len() > QUANTITY_MAX_LITERAL_BYTES {
            return Err(QuantityRefusal::LiteralTooLong);
        }
        let source = source.trim();
        let family = unit.family();
        let name = family
            .role_name(role)
            .map_err(|_| QuantityRefusal::InvalidRole)?;
        if let Some(body) = source
            .strip_prefix(name)
            .map(str::trim_start)
            .and_then(|body| body.strip_prefix('('))
            .and_then(|body| body.strip_suffix(')'))
        {
            let (number, symbol) = body.split_once(',').ok_or(QuantityRefusal::InvalidNumber)?;
            if !unit.matches_source_evidence(symbol.trim()) {
                return Err(QuantityRefusal::InvalidNumber);
            }
            return Self::parse_coordinate(number.trim(), unit, role);
        }
        if role != unit.declared_role() {
            return Err(QuantityRefusal::InvalidRole);
        }
        let number = source
            .strip_suffix(unit.symbol())
            .ok_or(QuantityRefusal::InvalidNumber)?;
        Self::parse_coordinate(number, unit, role)
    }
    pub fn parse_plot_literal(source: &str) -> Result<Self, QuantityRefusal> {
        if source.len() > QUANTITY_MAX_LITERAL_BYTES {
            return Err(QuantityRefusal::LiteralTooLong);
        }
        if let Some((name, args)) = source.split_once('(') {
            let (number, symbol) = args
                .strip_suffix(')')
                .and_then(|args| args.split_once(','))
                .ok_or(QuantityRefusal::InvalidNumber)?;
            let unit = Unit::resolve(symbol.trim()).map_err(QuantityRefusal::Unit)?;
            let role = [
                QuantityRole::Linear,
                QuantityRole::Point,
                QuantityRole::Delta,
            ]
            .into_iter()
            .find(|role| unit.family().role_name(*role).ok() == Some(name.trim()))
            .ok_or(QuantityRefusal::InvalidRole)?;
            return Self::parse_coordinate(number.trim(), unit, role);
        }
        let end = source
            .char_indices()
            .find_map(|(index, c)| {
                (!(c.is_ascii_digit() || c == '.' || (index == 0 && c == '-'))).then_some(index)
            })
            .unwrap_or(source.len());
        let (number, symbol) = source.split_at(end);
        let unit = Unit::resolve(symbol).map_err(QuantityRefusal::Unit)?;
        let role = unit.declared_role();
        Self::parse_coordinate(number, unit, role)
    }
    fn parse_coordinate(
        number: &str,
        unit: Unit,
        role: QuantityRole,
    ) -> Result<Self, QuantityRefusal> {
        if number.len() > QUANTITY_MAX_NUMBER_BYTES {
            return Err(QuantityRefusal::NumberTooLong);
        }
        if number.is_empty()
            || !number
                .char_indices()
                .all(|(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && c == '-'))
        {
            return Err(QuantityRefusal::InvalidNumber);
        }
        Ok(Self::from_math_role(
            MathValue::parse_decimal(number)?,
            &unit,
            role,
        )?)
    }
    pub fn canonical_literal(&self) -> Result<alloc::string::String, QuantityRefusal> {
        let number = self
            .coordinate()
            .to_owned()?
            .terminating_decimal_literal(QUANTITY_MAX_NUMBER_BYTES)?
            .ok_or(QuantityRefusal::InvalidNumber)?;
        if number.len() > QUANTITY_MAX_NUMBER_BYTES
            || !number
                .char_indices()
                .all(|(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && c == '-'))
        {
            return Err(QuantityRefusal::InvalidNumber);
        }
        if self.role() == self.unit().declared_role() {
            Ok(alloc::format!("{number}{}", self.unit().symbol()))
        } else {
            Ok(alloc::format!(
                "{}({number}, {})",
                self.family()
                    .role_name(self.role())
                    .map_err(|_| QuantityRefusal::InvalidRole)?,
                self.unit().symbol()
            ))
        }
    }
}
impl serde::Serialize for Quantity {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(self.encoded(), s)
    }
}
impl<'de> serde::Deserialize<'de> for Quantity {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bytes =
            crate::physical_definition::deserialize_bounded::<D, MAX_QUANTITY_ENCODED_BYTES>(d)?;
        Self::decode(&bytes).map_err(|_| serde::de::Error::custom("invalid mathematical Quantity"))
    }
}
