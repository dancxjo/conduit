//! A finite representation of a physical quantity, independent of its Type.
use crate::{Quantity, QuantityRole, Unit, QUANTITY_MAX_EXPONENT};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuantityFormInteger {
    I16Le,
    U16Le,
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
        let bytes: [u8; 2] = bytes
            .try_into()
            .map_err(|_| QuantityFormRefusal::WrongLength)?;
        let coefficient = match self.integer {
            QuantityFormInteger::I16Le => i128::from(i16::from_le_bytes(bytes)),
            QuantityFormInteger::U16Le => i128::from(u16::from_le_bytes(bytes)),
        };
        Quantity::from_decimal_role(coefficient, self.exponent, self.unit, self.role)
            .map_err(|_| QuantityFormRefusal::InvalidDefinition)
    }

    pub fn encode(&self, value: Quantity) -> Result<[u8; 2], QuantityFormRefusal> {
        use QuantityFormRefusal as R;
        self.validate()?;
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
        Ok(match self.integer {
            QuantityFormInteger::I16Le => i16::try_from(coefficient)
                .map_err(|_| R::Overflow)?
                .to_le_bytes(),
            QuantityFormInteger::U16Le => u16::try_from(coefficient)
                .map_err(|_| R::Overflow)?
                .to_le_bytes(),
        })
    }
}
