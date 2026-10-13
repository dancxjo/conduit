//! Software binary64 using exact dyadic arithmetic and ordered-bit searches.
//! No host floating point or empirical transcendental accuracy contract is used.
use crate::{ExactRational, MathValue, NumericRefusal};
use core::cmp::Ordering;

const SIGN: u64 = 1 << 63;
const EXPONENT: u64 = 0x7ff0_0000_0000_0000;
const FRACTION: u64 = 0x000f_ffff_ffff_ffff;
const QUIET: u64 = 1 << 51;
const MAX_FINITE: u64 = 0x7fef_ffff_ffff_ffff;
const CANONICAL_NAN: u64 = EXPONENT | QUIET;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary64Rounding {
    NearestEven,
    NearestAway,
    TowardZero,
    TowardPositive,
    TowardNegative,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Binary64Flags {
    pub invalid: bool,
    pub division_by_zero: bool,
    pub overflow: bool,
    pub underflow: bool,
    pub inexact: bool,
}
impl Binary64Flags {
    pub fn union(self, other: Self) -> Self {
        Self {
            invalid: self.invalid || other.invalid,
            division_by_zero: self.division_by_zero || other.division_by_zero,
            overflow: self.overflow || other.overflow,
            underflow: self.underflow || other.underflow,
            inexact: self.inexact || other.inexact,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binary64Value {
    bits: u64,
}
impl Binary64Value {
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }
    pub const fn bits(self) -> u64 {
        self.bits
    }
    pub const fn is_negative(self) -> bool {
        self.bits & SIGN != 0
    }
    pub const fn is_zero(self) -> bool {
        self.bits & !SIGN == 0
    }
    pub const fn is_nan(self) -> bool {
        self.bits & EXPONENT == EXPONENT && self.bits & FRACTION != 0
    }
    pub const fn is_infinite(self) -> bool {
        self.bits & !SIGN == EXPONENT
    }
    pub const fn is_signaling_nan(self) -> bool {
        self.is_nan() && self.bits & QUIET == 0
    }
    pub fn exact(self) -> Result<Option<MathValue>, NumericRefusal> {
        if self.is_nan() || self.is_infinite() {
            return Ok(None);
        }
        let exponent = ((self.bits & EXPONENT) >> 52) as i16;
        let coefficient = (self.bits & FRACTION) | if exponent != 0 { 1 << 52 } else { 0 };
        let exponent = if exponent == 0 {
            -1074
        } else {
            exponent - 1023 - 52
        };
        let rational = ExactRational::dyadic(coefficient, exponent)?;
        Ok(Some(MathValue::from_rational(if self.is_negative() {
            rational.negated()
        } else {
            rational
        })))
    }
    pub fn project(
        value: &MathValue,
        rounding: Binary64Rounding,
        work: &mut NumericRealizationWork,
    ) -> Result<Binary64Result, NumericRefusal> {
        let rational = value
            .as_rational()
            .ok_or(NumericRefusal::IndeterminateDomain)?;
        let negative = rational.is_negative();
        let magnitude = MathValue::from_rational(if negative {
            rational.negated()
        } else {
            rational
        });
        if magnitude == MathValue::ZERO {
            return Ok(result(0, Binary64Flags::default()));
        }
        round_positive(&magnitude, negative, rounding, false, work)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binary64Result {
    pub value: Binary64Value,
    pub flags: Binary64Flags,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary64Operation {
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
    SquareRoot,
}
/// Counts exact comparison/arithmetic primitives, each with Core's finite limb
/// and primitive work limits. The budget is checked before the charged work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericRealizationWork {
    maximum: u64,
    used: u64,
}
impl NumericRealizationWork {
    pub const fn new(maximum: u64) -> Self {
        Self { maximum, used: 0 }
    }
    pub const fn used(self) -> u64 {
        self.used
    }
    pub fn charge(&mut self, count: u64) -> Result<(), NumericRefusal> {
        self.used = self
            .used
            .checked_add(count)
            .filter(|n| *n <= self.maximum)
            .ok_or(NumericRefusal::WorkBudget)?;
        Ok(())
    }
}
fn result(bits: u64, flags: Binary64Flags) -> Binary64Result {
    Binary64Result {
        value: Binary64Value::from_bits(bits),
        flags,
    }
}
fn exact_bits(bits: u64, work: &mut NumericRealizationWork) -> Result<MathValue, NumericRefusal> {
    work.charge(1)?;
    Binary64Value::from_bits(bits)
        .exact()?
        .ok_or(NumericRefusal::Domain)
}
fn compare(
    a: &MathValue,
    b: &MathValue,
    work: &mut NumericRealizationWork,
) -> Result<Ordering, NumericRefusal> {
    work.charge(1)?;
    a.compare(b)?.ok_or(NumericRefusal::IndeterminateDomain)
}
fn round_positive(
    value: &MathValue,
    negative: bool,
    mode: Binary64Rounding,
    square_root: bool,
    work: &mut NumericRealizationWork,
) -> Result<Binary64Result, NumericRefusal> {
    let mut low = 0;
    let mut high = MAX_FINITE;
    while low < high {
        let midpoint = low + (high - low).div_ceil(2);
        let represented = exact_bits(midpoint, work)?;
        let compared = if square_root {
            work.charge(1)?;
            represented.checked_mul(&represented)?
        } else {
            represented
        };
        if compare(&compared, value, work)? != Ordering::Greater {
            low = midpoint;
        } else {
            high = midpoint - 1;
        }
    }
    let lower = exact_bits(low, work)?;
    let lower_compared = if square_root {
        work.charge(1)?;
        lower.checked_mul(&lower)?
    } else {
        lower.clone()
    };
    if compare(&lower_compared, value, work)? == Ordering::Equal {
        return Ok(result(
            low | if negative { SIGN } else { 0 },
            Binary64Flags::default(),
        ));
    }
    let upper_bits = if low == MAX_FINITE { EXPONENT } else { low + 1 };
    let upper = if upper_bits == EXPONENT {
        work.charge(1)?;
        MathValue::from_rational(ExactRational::dyadic(1, 1024)?)
    } else {
        exact_bits(upper_bits, work)?
    };
    let go_up = match mode {
        Binary64Rounding::TowardZero => false,
        Binary64Rounding::TowardPositive => !negative,
        Binary64Rounding::TowardNegative => negative,
        Binary64Rounding::NearestEven | Binary64Rounding::NearestAway => {
            work.charge(2)?;
            let midpoint = lower
                .checked_add(&upper)?
                .checked_div(&MathValue::integer(2))?;
            let midpoint = if square_root {
                work.charge(1)?;
                midpoint.checked_mul(&midpoint)?
            } else {
                midpoint
            };
            match compare(value, &midpoint, work)? {
                Ordering::Greater => true,
                Ordering::Less => false,
                Ordering::Equal => mode == Binary64Rounding::NearestAway || low & 1 != 0,
            }
        }
    };
    let magnitude_bits = if go_up { upper_bits } else { low };
    // Overflow is tested after rounding to unbounded exponent precision.
    let overflow = if low == MAX_FINITE {
        magnitude_bits == EXPONENT || compare(value, &upper, work)? != Ordering::Less
    } else {
        false
    };
    Ok(result(
        magnitude_bits | if negative { SIGN } else { 0 },
        Binary64Flags {
            overflow,
            underflow: magnitude_bits & EXPONENT == 0,
            inexact: true,
            ..Binary64Flags::default()
        },
    ))
}
fn nan(a: Binary64Value, b: Option<Binary64Value>) -> Option<Binary64Result> {
    let chosen = if a.is_nan() {
        Some(a)
    } else {
        b.filter(|b| b.is_nan())
    }?;
    Some(result(
        chosen.bits | QUIET,
        Binary64Flags {
            invalid: a.is_signaling_nan() || b.is_some_and(Binary64Value::is_signaling_nan),
            ..Binary64Flags::default()
        },
    ))
}
fn invalid() -> Binary64Result {
    result(
        CANONICAL_NAN,
        Binary64Flags {
            invalid: true,
            ..Binary64Flags::default()
        },
    )
}
impl Binary64Operation {
    pub fn apply(
        self,
        a: Binary64Value,
        b: Option<Binary64Value>,
        mode: Binary64Rounding,
        work: &mut NumericRealizationWork,
    ) -> Result<Binary64Result, NumericRefusal> {
        if matches!(self, Self::Negate | Self::SquareRoot) != b.is_none() {
            return Err(NumericRefusal::InvalidNumber);
        }
        work.charge(1)?;
        if self == Self::Negate {
            return Ok(result(a.bits ^ SIGN, Binary64Flags::default()));
        }
        if let Some(n) = nan(a, b) {
            return Ok(n);
        }
        if self == Self::SquareRoot {
            if a.is_negative() && !a.is_zero() {
                return Ok(invalid());
            }
            if a.is_zero() || a.is_infinite() {
                return Ok(result(a.bits, Binary64Flags::default()));
            }
            return round_positive(&exact_bits(a.bits, work)?, false, mode, true, work);
        }
        let b = b.ok_or(NumericRefusal::InvalidNumber)?;
        let (b, operation) = if self == Self::Subtract {
            (Binary64Value::from_bits(b.bits ^ SIGN), Self::Add)
        } else {
            (b, self)
        };
        let negative = a.is_negative() != b.is_negative();
        let special = match operation {
            Self::Add if a.is_infinite() && b.is_infinite() && negative => Some(invalid()),
            Self::Add if a.is_infinite() => Some(result(a.bits, Binary64Flags::default())),
            Self::Add if b.is_infinite() => Some(result(b.bits, Binary64Flags::default())),
            Self::Multiply
                if (a.is_zero() && b.is_infinite()) || (b.is_zero() && a.is_infinite()) =>
            {
                Some(invalid())
            }
            Self::Multiply if a.is_infinite() || b.is_infinite() => Some(result(
                EXPONENT | if negative { SIGN } else { 0 },
                Binary64Flags::default(),
            )),
            Self::Divide
                if (a.is_zero() && b.is_zero()) || (a.is_infinite() && b.is_infinite()) =>
            {
                Some(invalid())
            }
            Self::Divide if a.is_infinite() => Some(result(
                EXPONENT | if negative { SIGN } else { 0 },
                Binary64Flags::default(),
            )),
            Self::Divide if b.is_infinite() => Some(result(
                if negative { SIGN } else { 0 },
                Binary64Flags::default(),
            )),
            Self::Divide if b.is_zero() => Some(result(
                EXPONENT | if negative { SIGN } else { 0 },
                Binary64Flags {
                    division_by_zero: true,
                    ..Binary64Flags::default()
                },
            )),
            _ => None,
        };
        if let Some(value) = special {
            return Ok(value);
        }
        let left = exact_bits(a.bits, work)?;
        let right = exact_bits(b.bits, work)?;
        work.charge(1)?;
        let exact = match operation {
            Self::Add => left.checked_add(&right)?,
            Self::Multiply => left.checked_mul(&right)?,
            Self::Divide => left.checked_div(&right)?,
            _ => return Err(NumericRefusal::Domain),
        };
        if exact == MathValue::ZERO {
            let negative = if operation == Self::Add {
                (a.is_negative() && b.is_negative())
                    || (a.is_negative() != b.is_negative()
                        && mode == Binary64Rounding::TowardNegative)
            } else {
                negative
            };
            return Ok(result(
                if negative { SIGN } else { 0 },
                Binary64Flags::default(),
            ));
        }
        Binary64Value::project(&exact, mode, work)
    }
}
