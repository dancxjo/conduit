//! Portable exact-width integer values and expression operations.

use core::cmp::Ordering;

use crate::{fixed_integer_bytes, PrimitiveInfoKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedIntegerRefusal {
    NotIntegerKind,
    WrongEncodedLength { expected: usize, actual: usize },
    Signedness,
    OutOfRange,
    Overflow,
    DivisionByZero,
    ShiftOutOfRange { width: u32, count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedInteger {
    kind: PrimitiveInfoKind,
    bits: u128,
}

impl FixedInteger {
    pub fn decode(kind: PrimitiveInfoKind, encoded: &[u8]) -> Result<Self, FixedIntegerRefusal> {
        let width = integer_width(kind).ok_or(FixedIntegerRefusal::NotIntegerKind)?;
        let expected = fixed_integer_bytes(kind);
        if encoded.len() != expected {
            return Err(FixedIntegerRefusal::WrongEncodedLength {
                expected,
                actual: encoded.len(),
            });
        }
        let mut bytes = [0_u8; 16];
        bytes[..encoded.len()].copy_from_slice(encoded);
        Ok(Self {
            kind,
            bits: u128::from_le_bytes(bytes) & width_mask(width),
        })
    }

    pub fn from_unsigned(
        kind: PrimitiveInfoKind,
        value: u128,
    ) -> Result<Self, FixedIntegerRefusal> {
        let width = integer_width(kind).ok_or(FixedIntegerRefusal::NotIntegerKind)?;
        if is_signed(kind) {
            return Err(FixedIntegerRefusal::Signedness);
        }
        if value > width_mask(width) {
            return Err(FixedIntegerRefusal::OutOfRange);
        }
        Ok(Self { kind, bits: value })
    }

    pub fn from_signed(kind: PrimitiveInfoKind, value: i128) -> Result<Self, FixedIntegerRefusal> {
        let width = integer_width(kind).ok_or(FixedIntegerRefusal::NotIntegerKind)?;
        if !is_signed(kind) {
            return Err(FixedIntegerRefusal::Signedness);
        }
        let (minimum, maximum) = signed_bounds(width);
        if !(minimum..=maximum).contains(&value) {
            return Err(FixedIntegerRefusal::OutOfRange);
        }
        Ok(Self {
            kind,
            bits: (value as u128) & width_mask(width),
        })
    }

    pub const fn kind(self) -> PrimitiveInfoKind {
        self.kind
    }

    pub fn encode(self) -> ([u8; 16], usize) {
        (self.bits.to_le_bytes(), fixed_integer_bytes(self.kind))
    }

    pub fn unsigned(self) -> Result<u128, FixedIntegerRefusal> {
        if is_signed(self.kind) {
            Err(FixedIntegerRefusal::Signedness)
        } else {
            Ok(self.bits)
        }
    }

    pub fn signed(self) -> Result<i128, FixedIntegerRefusal> {
        let width = integer_width(self.kind).ok_or(FixedIntegerRefusal::NotIntegerKind)?;
        if !is_signed(self.kind) {
            return Err(FixedIntegerRefusal::Signedness);
        }
        Ok(sign_extend(self.bits, width))
    }

    pub fn checked_add(self, right: Self) -> Result<Self, FixedIntegerRefusal> {
        self.same_kind(right)?;
        if is_signed(self.kind) {
            let value = self
                .signed()?
                .checked_add(right.signed()?)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_signed(self.kind, value).map_err(out_of_range_as_overflow)
        } else {
            let value = self
                .bits
                .checked_add(right.bits)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_unsigned(self.kind, value).map_err(out_of_range_as_overflow)
        }
    }

    pub fn checked_sub(self, right: Self) -> Result<Self, FixedIntegerRefusal> {
        self.same_kind(right)?;
        if is_signed(self.kind) {
            let value = self
                .signed()?
                .checked_sub(right.signed()?)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_signed(self.kind, value).map_err(out_of_range_as_overflow)
        } else {
            let value = self
                .bits
                .checked_sub(right.bits)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_unsigned(self.kind, value)
        }
    }

    pub fn checked_mul(self, right: Self) -> Result<Self, FixedIntegerRefusal> {
        self.same_kind(right)?;
        if is_signed(self.kind) {
            let value = self
                .signed()?
                .checked_mul(right.signed()?)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_signed(self.kind, value).map_err(out_of_range_as_overflow)
        } else {
            let value = self
                .bits
                .checked_mul(right.bits)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_unsigned(self.kind, value).map_err(out_of_range_as_overflow)
        }
    }

    pub fn checked_div(self, right: Self) -> Result<Self, FixedIntegerRefusal> {
        self.same_kind(right)?;
        if right.bits == 0 {
            return Err(FixedIntegerRefusal::DivisionByZero);
        }
        if is_signed(self.kind) {
            let value = self
                .signed()?
                .checked_div(right.signed()?)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_signed(self.kind, value).map_err(out_of_range_as_overflow)
        } else {
            Self::from_unsigned(self.kind, self.bits / right.bits)
        }
    }

    pub fn checked_rem(self, right: Self) -> Result<Self, FixedIntegerRefusal> {
        self.same_kind(right)?;
        if right.bits == 0 {
            return Err(FixedIntegerRefusal::DivisionByZero);
        }
        if is_signed(self.kind) {
            let value = self
                .signed()?
                .checked_rem(right.signed()?)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_signed(self.kind, value).map_err(out_of_range_as_overflow)
        } else {
            Self::from_unsigned(self.kind, self.bits % right.bits)
        }
    }

    pub fn checked_neg(self) -> Result<Self, FixedIntegerRefusal> {
        let value = self
            .signed()?
            .checked_neg()
            .ok_or(FixedIntegerRefusal::Overflow)?;
        Self::from_signed(self.kind, value).map_err(out_of_range_as_overflow)
    }

    pub fn bit_and(self, right: Self) -> Result<Self, FixedIntegerRefusal> {
        self.bitwise(right, |left, right| left & right)
    }

    pub fn bit_or(self, right: Self) -> Result<Self, FixedIntegerRefusal> {
        self.bitwise(right, |left, right| left | right)
    }

    pub fn bit_xor(self, right: Self) -> Result<Self, FixedIntegerRefusal> {
        self.bitwise(right, |left, right| left ^ right)
    }

    pub fn checked_shift_left(self, count: u32) -> Result<Self, FixedIntegerRefusal> {
        let width = integer_width(self.kind).ok_or(FixedIntegerRefusal::NotIntegerKind)?;
        check_shift(width, count)?;
        if is_signed(self.kind) {
            let factor = 1_i128
                .checked_shl(count)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            let value = self
                .signed()?
                .checked_mul(factor)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_signed(self.kind, value).map_err(out_of_range_as_overflow)
        } else {
            let value = self
                .bits
                .checked_shl(count)
                .ok_or(FixedIntegerRefusal::Overflow)?;
            Self::from_unsigned(self.kind, value).map_err(out_of_range_as_overflow)
        }
    }

    pub fn checked_shift_right(self, count: u32) -> Result<Self, FixedIntegerRefusal> {
        let width = integer_width(self.kind).ok_or(FixedIntegerRefusal::NotIntegerKind)?;
        check_shift(width, count)?;
        if is_signed(self.kind) {
            Self::from_signed(self.kind, self.signed()? >> count)
        } else {
            Self::from_unsigned(self.kind, self.bits >> count)
        }
    }

    pub fn compare(self, right: Self) -> Result<Ordering, FixedIntegerRefusal> {
        self.same_kind(right)?;
        if is_signed(self.kind) {
            Ok(self.signed()?.cmp(&right.signed()?))
        } else {
            Ok(self.bits.cmp(&right.bits))
        }
    }

    fn same_kind(self, right: Self) -> Result<(), FixedIntegerRefusal> {
        if self.kind == right.kind {
            Ok(())
        } else {
            Err(FixedIntegerRefusal::Signedness)
        }
    }

    fn bitwise(
        self,
        right: Self,
        operation: impl FnOnce(u128, u128) -> u128,
    ) -> Result<Self, FixedIntegerRefusal> {
        self.same_kind(right)?;
        Ok(Self {
            kind: self.kind,
            bits: operation(self.bits, right.bits),
        })
    }
}

const fn integer_width(kind: PrimitiveInfoKind) -> Option<u32> {
    match kind {
        PrimitiveInfoKind::U8 | PrimitiveInfoKind::I8 => Some(8),
        PrimitiveInfoKind::U16 | PrimitiveInfoKind::I16 => Some(16),
        PrimitiveInfoKind::U32 | PrimitiveInfoKind::I32 => Some(32),
        PrimitiveInfoKind::U64 | PrimitiveInfoKind::I64 => Some(64),
        PrimitiveInfoKind::U128 | PrimitiveInfoKind::I128 => Some(128),
        _ => None,
    }
}

const fn is_signed(kind: PrimitiveInfoKind) -> bool {
    matches!(
        kind,
        PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128
    )
}

const fn width_mask(width: u32) -> u128 {
    if width == 128 {
        u128::MAX
    } else {
        (1_u128 << width) - 1
    }
}

const fn signed_bounds(width: u32) -> (i128, i128) {
    if width == 128 {
        (i128::MIN, i128::MAX)
    } else {
        let magnitude = 1_i128 << (width - 1);
        (-magnitude, magnitude - 1)
    }
}

const fn sign_extend(bits: u128, width: u32) -> i128 {
    if width == 128 || bits & (1_u128 << (width - 1)) == 0 {
        bits as i128
    } else {
        (bits | !width_mask(width)) as i128
    }
}

fn check_shift(width: u32, count: u32) -> Result<(), FixedIntegerRefusal> {
    if count < width {
        Ok(())
    } else {
        Err(FixedIntegerRefusal::ShiftOutOfRange { width, count })
    }
}

fn out_of_range_as_overflow(refusal: FixedIntegerRefusal) -> FixedIntegerRefusal {
    match refusal {
        FixedIntegerRefusal::OutOfRange => FixedIntegerRefusal::Overflow,
        other => other,
    }
}
