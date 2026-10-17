//! Exact principal inverse-function domains and canonical rational identities.
use super::{
    ExactNumericWork as Work, ExactRational as Q, MathematicalNode as N,
    MathematicalValueRefusal as R,
};
use core::cmp::Ordering;
pub(super) fn cardinal(node: N, input: Q, work: &mut Work) -> Result<Option<Q>, R> {
    let function = match node {
        N::ArctangentTurns(_) => crate::InverseAngleFunction::Arctangent,
        N::ArcsineTurns(_) => crate::InverseAngleFunction::Arcsine,
        N::ArccosineTurns(_) => crate::InverseAngleFunction::Arccosine,
        _ => return Err(R::UnsupportedDomainProof),
    };
    cardinal_function(function, input, work)
}
fn cardinal_function(
    function: crate::InverseAngleFunction,
    input: Q,
    work: &mut Work,
) -> Result<Option<Q>, R> {
    use crate::InverseAngleFunction as F;
    let magnitude = if input.is_negative() {
        input.negated()
    } else {
        input
    };
    if !matches!(function, F::Arctangent)
        && magnitude.checked_compare(Q::ONE, work)? == Ordering::Greater
    {
        return Err(R::InverseTrigonometricDomain);
    }
    let asin = if input == Q::ZERO {
        Some(Q::ZERO)
    } else if magnitude == Q::ONE {
        Some(Q::ratio(1, 4, work)?)
    } else if magnitude == Q::ratio(1, 2, work)? {
        Some(Q::ratio(1, 12, work)?)
    } else {
        None
    };
    let signed = |q: Q| if input.is_negative() { q.negated() } else { q };
    Ok(match function {
        F::Arctangent if input == Q::ZERO => Some(Q::ZERO),
        F::Arctangent if magnitude == Q::ONE => Some(signed(Q::ratio(1, 8, work)?)),
        F::Arctangent => None,
        F::Arcsine => asin.map(signed),
        F::Arccosine => match asin {
            Some(q) => Some(Q::ratio(1, 4, work)?.checked_subtract(signed(q), work)?),
            None => None,
        },
    })
}

pub(in crate::exact_numeric) fn encode_inverse_coordinate(
    function: crate::InverseAngleFunction,
    input: Q,
    output: &mut [u8],
    work: &mut Work,
) -> Result<usize, R> {
    if let Some(q) = cardinal_function(function, input, work)? {
        return super::MathematicalValue::rational(q).encode(output);
    }
    let small = || R::Numeric(super::ExactNumericRefusal::EncodingBufferTooSmall);
    if output.len() < 8 {
        return Err(small());
    }
    let n = input.encode(&mut output[6..])?;
    if output.len() < 8 + n {
        return Err(small());
    }
    output[..4].copy_from_slice(&[1, 1, 2, 0]);
    output[4..6].copy_from_slice(&(n as u16).to_le_bytes());
    let tag = match function {
        crate::InverseAngleFunction::Arctangent => 12,
        crate::InverseAngleFunction::Arcsine => 13,
        crate::InverseAngleFunction::Arccosine => 14,
    };
    output[6 + n..8 + n].copy_from_slice(&[tag, 0]);
    Ok(8 + n)
}

pub(in crate::exact_numeric) fn encode_shifted_arctangent(
    input: Q,
    offset: Q,
    output: &mut [u8],
    work: &mut Work,
) -> Result<usize, R> {
    use crate::InverseAngleFunction::Arctangent;
    if let Some(q) = cardinal_function(Arctangent, input, work)? {
        return super::MathematicalValue::rational(q.checked_add(offset, work)?).encode(output);
    }
    let length = encode_inverse_coordinate(Arctangent, input, output, work)?;
    if offset == Q::ZERO {
        return Ok(length);
    }
    // The atan2 offset has opposite sign to its nonzero ratio, so these two
    // rational nodes cannot alias. Postorder is ratio, atan, offset, addition.
    let small = || R::Numeric(super::ExactNumericRefusal::EncodingBufferTooSmall);
    if output.len() < length + 3 {
        return Err(small());
    }
    let n = offset.encode(&mut output[length + 3..])?;
    if output.len() < length + 3 + n + 3 {
        return Err(small());
    }
    output[length] = 0;
    output[length + 1..length + 3].copy_from_slice(&(n as u16).to_le_bytes());
    output[length + 3 + n..length + 6 + n].copy_from_slice(&[4, 1, 2]);
    output[2] = 4;
    Ok(length + 6 + n)
}
