//! Private checked I128 operators. Failure propagates without exporting scratch.
use super::Error;
use core::ops::{Add, Div, Mul, Shl, Shr, Sub};

#[derive(Clone, Copy)]
pub(super) struct Checked(Option<i128>);
impl From<i128> for Checked {
    fn from(value: i128) -> Self {
        Self(Some(value))
    }
}
impl Checked {
    pub(super) fn value(self) -> Result<i128, Error> {
        self.0.ok_or(Error::Arithmetic)
    }
}
macro_rules! binary {
    ($trait:ident, $method:ident, $checked:ident) => {
        impl $trait for Checked {
            type Output = Self;
            fn $method(self, rhs: Self) -> Self {
                Self(self.0.zip(rhs.0).and_then(|(a, b)| a.$checked(b)))
            }
        }
    };
}
binary!(Add, add, checked_add);
binary!(Sub, sub, checked_sub);
binary!(Mul, mul, checked_mul);
binary!(Div, div, checked_div);
impl Shl<u32> for Checked {
    type Output = Self;
    fn shl(self, bits: u32) -> Self {
        // checked_shl checks the shift count, not lost signed bits. Multiplying
        // by a checked positive power of two preserves checked integer shifts.
        Self(self.0.and_then(|v| {
            1_i128
                .checked_shl(bits)
                .filter(|n| *n > 0)
                .and_then(|n| v.checked_mul(n))
        }))
    }
}
impl Shr<u32> for Checked {
    type Output = Self;
    fn shr(self, bits: u32) -> Self {
        Self(self.0.and_then(|v| v.checked_shr(bits)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_rounding_and_checked_failures_preserve_stage_semantics() {
        let c = Checked::from;
        assert_eq!((c(-3) >> 1).value(), Ok(-2));
        assert_eq!((c(-3) / c(2)).value(), Ok(-1));
        for value in [
            c(i128::MAX) + c(1),
            c(i128::MIN) - c(1),
            c(i128::MAX) * c(2),
            c(i128::MIN) / c(-1),
            c(1) / c(0),
            c(i128::MAX) << 1,
            c(1) << 128,
            c(1) >> 128,
        ] {
            assert_eq!(value.value(), Err(Error::Arithmetic));
            assert_eq!((value * c(0)).value(), Err(Error::Arithmetic));
        }
        assert_eq!((c(i128::MIN) << 0).value(), Ok(i128::MIN));
        assert_eq!((c(-1) << 35).value(), Ok(-(1_i128 << 35)));
    }
}
