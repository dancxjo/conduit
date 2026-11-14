//! A finite representation of a physical quantity, independent of its Type.
use crate::{Quantity, QuantityRole, Unit, QUANTITY_MAX_EXPONENT};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantityFormInteger {
    I16Le,
    U16Le,
    I128Le,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecimalQuantityForm {
    pub integer: QuantityFormInteger,
    pub unit: Unit,
    pub exponent: i16,
    pub role: QuantityRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantityFormRefusal {
    InvalidDefinition,
    WrongLength,
    WrongFamily,
    WrongRole,
    Inexact,
    Overflow,
}

impl DecimalQuantityForm {
    /// Exact selected representation width; semantic Quantity bytes are separate.
    pub const fn exact_bytes(&self) -> usize {
        match self.integer {
            QuantityFormInteger::I16Le | QuantityFormInteger::U16Le => 2,
            QuantityFormInteger::I128Le => 16,
        }
    }

    pub fn validate(&self) -> Result<(), QuantityFormRefusal> {
        if !(-QUANTITY_MAX_EXPONENT..=QUANTITY_MAX_EXPONENT).contains(&self.exponent)
            || !self.unit.admits_role(self.role)
        {
            return Err(QuantityFormRefusal::InvalidDefinition);
        }
        Ok(())
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<Quantity, QuantityFormRefusal> {
        self.validate()?;
        let wrong_length = |_| QuantityFormRefusal::WrongLength;
        let coefficient = match self.integer {
            QuantityFormInteger::I16Le => {
                i128::from(i16::from_le_bytes(bytes.try_into().map_err(wrong_length)?))
            }
            QuantityFormInteger::U16Le => {
                i128::from(u16::from_le_bytes(bytes.try_into().map_err(wrong_length)?))
            }
            QuantityFormInteger::I128Le => {
                i128::from_le_bytes(bytes.try_into().map_err(wrong_length)?)
            }
        };
        Quantity::from_decimal_role(coefficient, self.exponent, self.unit, self.role).map_err(
            |error| match error {
                crate::QuantityRefusal::SignificantDigitsExceeded => QuantityFormRefusal::Overflow,
                _ => QuantityFormRefusal::InvalidDefinition,
            },
        )
    }

    /// Fixed two-byte convenience. Other selected widths refuse rather than truncate.
    pub fn encode(&self, value: Quantity) -> Result<[u8; 2], QuantityFormRefusal> {
        let mut bytes = [0; 2];
        self.encode_into(value, &mut bytes)?;
        Ok(bytes)
    }

    /// Encode into exactly the selected width without allocation or truncation.
    pub fn encode_into(
        &self,
        value: Quantity,
        bytes: &mut [u8],
    ) -> Result<usize, QuantityFormRefusal> {
        use QuantityFormRefusal as R;
        self.validate()?;
        if bytes.len() != self.exact_bytes() {
            return Err(R::WrongLength);
        }
        if value.family() != self.unit.family() {
            return Err(R::WrongFamily);
        }
        if value.role() != self.role {
            return Err(R::WrongRole);
        }
        let value = value
            .convert_to_decimal(self.unit)
            .map_err(|why| match why {
                crate::QuantityConversionRefusal::Overflow => R::Overflow,
                _ => R::Inexact,
            })?;
        let difference = i32::from(value.exponent()) - i32::from(self.exponent);
        let coefficient = if value.coefficient() == 0 {
            0
        } else if difference >= 0 {
            let scale = 10_i128.checked_pow(difference as u32).ok_or(R::Overflow)?;
            value.coefficient().checked_mul(scale).ok_or(R::Overflow)?
        } else {
            let scale = 10_i128
                .checked_pow((-difference) as u32)
                .ok_or(R::Inexact)?;
            if value.coefficient() % scale != 0 {
                return Err(R::Inexact);
            }
            value.coefficient() / scale
        };
        match self.integer {
            QuantityFormInteger::I16Le => bytes.copy_from_slice(
                &i16::try_from(coefficient)
                    .map_err(|_| R::Overflow)?
                    .to_le_bytes(),
            ),
            QuantityFormInteger::U16Le => bytes.copy_from_slice(
                &u16::try_from(coefficient)
                    .map_err(|_| R::Overflow)?
                    .to_le_bytes(),
            ),
            QuantityFormInteger::I128Le => bytes.copy_from_slice(&coefficient.to_le_bytes()),
        }
        Ok(self.exact_bytes())
    }
}

#[cfg(test)]
#[path = "quantity_form_tests.rs"]
mod tests;
