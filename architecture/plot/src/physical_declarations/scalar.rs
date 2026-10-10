//! Exact evaluation of declaration scalar expressions. This walks the same AST
//! as ordinary expressions; quoted values and physical quantities are refused.
use super::{refusal, PhysicalDeclarationDiagnostic};
use crate::{BinaryOperator, ExpressionSyntax, UnaryOperator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExactScalar {
    pub numerator: i128,
    pub denominator: i128,
    pub decimal_exponent: i16,
}

impl ExactScalar {
    pub(crate) fn integer(self) -> Option<i128> {
        let power = 10i128.checked_pow(u32::from(self.decimal_exponent.unsigned_abs()))?;
        let (numerator, denominator) = if self.decimal_exponent >= 0 {
            (self.numerator.checked_mul(power)?, self.denominator)
        } else {
            (self.numerator, self.denominator.checked_mul(power)?)
        };
        (numerator % denominator == 0).then_some(numerator / denominator)
    }
}

type Result<T> = core::result::Result<T, PhysicalDeclarationDiagnostic>;

pub(crate) fn check_exact_scalar(value: &ExpressionSyntax) -> Result<ExactScalar> {
    let fail = || {
        refusal(
            value.span(),
            "expected a bounded exact decimal or rational scalar",
        )
    };
    let result = match value {
        ExpressionSyntax::Atomic(atom) => decimal(&atom.text).ok_or_else(fail)?,
        ExpressionSyntax::Unary {
            operator: UnaryOperator::Negate,
            operand,
            ..
        } => {
            let mut result = check_exact_scalar(operand)?;
            result.numerator = result.numerator.checked_neg().ok_or_else(fail)?;
            result
        }
        ExpressionSyntax::Binary {
            operator: BinaryOperator::Divide,
            left,
            right,
            ..
        } => {
            let left = check_exact_scalar(left)?;
            let right = check_exact_scalar(right)?;
            if right.numerator == 0 {
                return Err(refusal(
                    right_span(right, value),
                    "exact rational denominator cannot be zero",
                ));
            }
            // Cross-cancel before multiplication to avoid refusing an exact
            // representable ratio merely because its unreduced product is large.
            let a = gcd(
                left.numerator.unsigned_abs(),
                right.numerator.unsigned_abs(),
            );
            let b = gcd(right.denominator as u128, left.denominator as u128);
            ExactScalar {
                numerator: (left.numerator / i128::try_from(a).map_err(|_| fail())?)
                    .checked_mul(right.denominator / b as i128)
                    .ok_or_else(fail)?,
                denominator: (left.denominator / b as i128)
                    .checked_mul(right.numerator / i128::try_from(a).map_err(|_| fail())?)
                    .ok_or_else(fail)?,
                decimal_exponent: left
                    .decimal_exponent
                    .checked_sub(right.decimal_exponent)
                    .ok_or_else(fail)?,
            }
        }
        ExpressionSyntax::Record { fields, .. } if fields.len() == 2 => {
            let numerator = fields
                .iter()
                .find(|field| field.name.text == "numerator" && !field.punned)
                .ok_or_else(fail)?;
            let denominator = fields
                .iter()
                .find(|field| field.name.text == "denominator" && !field.punned)
                .ok_or_else(fail)?;
            return check_exact_scalar(&ExpressionSyntax::Binary {
                operator: BinaryOperator::Divide,
                left: alloc::boxed::Box::new(numerator.value.clone()),
                right: alloc::boxed::Box::new(denominator.value.clone()),
                span: value.span(),
            });
        }
        _ => return Err(fail()),
    };
    normalize(result).ok_or_else(fail)
}

fn right_span(_: ExactScalar, value: &ExpressionSyntax) -> crate::Span {
    match value {
        ExpressionSyntax::Binary { right, .. } => right.span(),
        _ => value.span(),
    }
}

fn decimal(text: &str) -> Option<ExactScalar> {
    let (digits, exponent) = if let Some(index) = text.find(['e', 'E']) {
        (&text[..index], text[index + 1..].parse::<i16>().ok()?)
    } else {
        (text, 0)
    };
    let mut coefficient = 0i128;
    let mut fraction = 0i16;
    let mut dot = false;
    let mut count = 0usize;
    for c in digits.chars() {
        if c == '.' && !dot {
            dot = true;
            continue;
        }
        let digit = c.to_digit(10)?;
        coefficient = coefficient
            .checked_mul(10)?
            .checked_add(i128::from(digit))?;
        if dot {
            fraction = fraction.checked_add(1)?;
        }
        count += 1;
    }
    if count == 0 || count > 38 {
        return None;
    }
    normalize(ExactScalar {
        numerator: coefficient,
        denominator: 1,
        decimal_exponent: exponent.checked_sub(fraction)?,
    })
}

fn normalize(mut value: ExactScalar) -> Option<ExactScalar> {
    if value.denominator == 0 {
        return None;
    }
    if value.denominator < 0 {
        value.denominator = value.denominator.checked_neg()?;
        value.numerator = value.numerator.checked_neg()?;
    }
    if value.numerator == 0 {
        return Some(ExactScalar {
            numerator: 0,
            denominator: 1,
            decimal_exponent: 0,
        });
    }
    let divisor = i128::try_from(gcd(
        value.numerator.unsigned_abs(),
        value.denominator as u128,
    ))
    .ok()?;
    value.numerator /= divisor;
    value.denominator /= divisor;
    while value.numerator % 10 == 0 {
        value.numerator /= 10;
        value.decimal_exponent = value.decimal_exponent.checked_add(1)?;
    }
    while value.denominator % 10 == 0 {
        value.denominator /= 10;
        value.decimal_exponent = value.decimal_exponent.checked_sub(1)?;
    }
    if !(-128..=128).contains(&value.decimal_exponent) {
        return None;
    }
    Some(value)
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}
