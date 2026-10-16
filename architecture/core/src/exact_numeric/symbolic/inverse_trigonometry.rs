//! Exact principal inverse-function domains and canonical rational identities.
use super::{
    ExactNumericWork as Work, ExactRational as Q, MathematicalNode as N,
    MathematicalValueRefusal as R,
};
use core::cmp::Ordering;
pub(super) fn cardinal(node: N, input: Q, work: &mut Work) -> Result<Option<Q>, R> {
    let magnitude = if input.is_negative() {
        input.negated()
    } else {
        input
    };
    if !matches!(node, N::ArctangentTurns(_))
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
    Ok(match node {
        N::ArctangentTurns(_) if input == Q::ZERO => Some(Q::ZERO),
        N::ArctangentTurns(_) if magnitude == Q::ONE => Some(signed(Q::ratio(1, 8, work)?)),
        N::ArctangentTurns(_) => None,
        N::ArcsineTurns(_) => asin.map(signed),
        N::ArccosineTurns(_) => match asin {
            Some(q) => Some(Q::ratio(1, 4, work)?.checked_subtract(signed(q), work)?),
            None => None,
        },
        _ => return Err(R::UnsupportedDomainProof),
    })
}
