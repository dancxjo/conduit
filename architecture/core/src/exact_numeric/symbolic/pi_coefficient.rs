//! Bounded recognition of a rational multiple of Pi, not general simplification.
use super::*;
impl MathematicalValueView<'_> {
    /// Recognizes Pi with rational multiplication/division and exact zero.
    /// Unknown shapes return None; no approximate equality is attempted.
    pub fn rational_pi_coefficient(
        &self,
        work: &mut ExactNumericWork,
    ) -> Result<Option<ExactRational>, MathematicalValueRefusal> {
        if self.node_count() == 0 {
            return Ok((self.as_rational(work)? == Some(ExactRational::ZERO))
                .then_some(ExactRational::ZERO));
        }
        coefficient(self, self.node_count() - 1, 0, work)
    }
}
fn coefficient(
    view: &MathematicalValueView<'_>,
    index: usize,
    depth: usize,
    work: &mut ExactNumericWork,
) -> Result<Option<ExactRational>, MathematicalValueRefusal> {
    if depth >= MATHEMATICAL_VALUE_MAX_DEPTH {
        return Err(MathematicalValueRefusal::DepthBound);
    }
    let (next, factor, divide) = match view.node(index, work)? {
        MathematicalNode::Pi => return Ok(Some(ExactRational::ONE)),
        MathematicalNode::Multiply(a, b) => {
            if let MathematicalNode::Rational(q) = view.node(a.index as usize, work)? {
                (b, q, false)
            } else if let MathematicalNode::Rational(q) = view.node(b.index as usize, work)? {
                (a, q, false)
            } else {
                return Ok(None);
            }
        }
        MathematicalNode::Divide(a, b) => {
            let MathematicalNode::Rational(q) = view.node(b.index as usize, work)? else {
                return Ok(None);
            };
            (a, q, true)
        }
        _ => return Ok(None),
    };
    let Some(value) = coefficient(view, next.index as usize, depth + 1, work)? else {
        return Ok(None);
    };
    Ok(Some(if divide {
        value.checked_divide(factor, work)?
    } else {
        value.checked_multiply(factor, work)?
    }))
}
