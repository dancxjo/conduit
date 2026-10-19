//! Existing exact arithmetic with an optional bounded symbolic-data construction.
//! A canonical denotation graph is not an intermediate-rounded execution witness.
use crate::{BinaryOperator as O, PortableExpressionEvaluationRefusal as R};
use conduit_core::{
    ExactNumericRefusal as N, ExactNumericWork, MathematicalNode as Node, MathematicalValue,
    MathematicalValueRefusal as M, MathematicalValueView as View,
    PreparedMathematicalValueBuilder as Builder, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
    MATHEMATICAL_VALUE_VALIDATION_WORK,
};
pub(crate) struct PreparedArithmetic(Builder);
impl PreparedArithmetic {
    pub(crate) fn new() -> Result<Self, R> {
        Ok(Self(builder()?))
    }
    pub(crate) fn retained_payload_bytes(&self) -> usize {
        self.0.retained_payload_bytes()
    }
    pub(crate) fn encode(
        &mut self,
        operator: O,
        left: &[u8],
        right: &[u8],
        output: &mut [u8],
    ) -> Result<usize, R> {
        evaluate_inner(operator, left, right, output, Some(&mut self.0))
    }
}
fn builder() -> Result<Builder, R> {
    Builder::new(131072, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES).map_err(R::MathematicalArithmetic)
}
/// Allocating reference entrance; rational-only arithmetic needs no graph builder.
pub(crate) fn evaluate(
    operator: O,
    left: &[u8],
    right: &[u8],
    output: &mut [u8],
) -> Result<usize, R> {
    evaluate_inner(operator, left, right, output, None)
}
fn evaluate_inner(
    operator: O,
    left: &[u8],
    right: &[u8],
    output: &mut [u8],
    prepared: Option<&mut Builder>,
) -> Result<usize, R> {
    let mut work = ExactNumericWork::new(3 * MATHEMATICAL_VALUE_VALIDATION_WORK);
    let left = View::decode(left, &mut work).map_err(|_| R::Arithmetic)?;
    let right = View::decode(right, &mut work).map_err(|_| R::Arithmetic)?;
    let a = left.as_rational(&mut work).map_err(|_| R::Arithmetic)?;
    let b = right.as_rational(&mut work).map_err(|_| R::Arithmetic)?;
    if let (Some(a), Some(b)) = (a, b) {
        // Preserve the old exact rational path and its refusal contract.
        let result = match operator {
            O::Add => a.checked_add(b, &mut work),
            O::Subtract => a.checked_subtract(b, &mut work),
            O::Multiply => a.checked_multiply(b, &mut work),
            O::Divide => a.checked_divide(b, &mut work),
            _ => return Err(R::InvalidProgram),
        }
        .map_err(|_| R::Arithmetic)?;
        return MathematicalValue::rational(result)
            .encode(output)
            .map_err(|_| R::Arithmetic);
    }
    match prepared {
        Some(builder) => symbolic(operator, &left, &right, output, builder, &mut work),
        None => symbolic(operator, &left, &right, output, &mut builder()?, &mut work),
    }
}
fn symbolic(
    operator: O,
    left: &View<'_>,
    right: &View<'_>,
    output: &mut [u8],
    builder: &mut Builder,
    work: &mut ExactNumericWork,
) -> Result<usize, R> {
    builder.reset();
    let a = builder
        .append_value(left, work)
        .map_err(R::MathematicalArithmetic)?;
    let b = builder
        .append_value(right, work)
        .map_err(R::MathematicalArithmetic)?;
    let node = match operator {
        O::Add => Node::Add(a, b),
        O::Subtract => Node::Subtract(a, b),
        O::Multiply => Node::Multiply(a, b),
        O::Divide => Node::Divide(a, b),
        _ => return Err(R::InvalidProgram),
    };
    let root = builder
        .insert(node, work)
        .map_err(R::MathematicalArithmetic)?;
    builder
        .encode(root, output, work)
        .map_err(R::MathematicalArithmetic)
}
/// Keep prior pure-expression causes unchanged; these follow root cases 33..=36.
pub fn failure_detail(reason: &M) -> u16 {
    match reason {
        M::Numeric(N::DivisionByZero) => 37,
        M::UnsupportedDomainProof => 38,
        M::Numeric(N::WorkLimit) => 39,
        M::NodeBound
        | M::DepthBound
        | M::Numeric(N::ExtentExceeded | N::EncodingBufferTooSmall) => 40,
        _ => 41,
    }
}
