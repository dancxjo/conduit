//! Exact principal-root construction over the existing bounded rational domain.
//! This creates mathematical data; it neither computes digits nor selects floats.
use crate::PortableExpressionEvaluationRefusal as R;
use conduit_core::{
    ExactNumericWork, MathematicalNode, MathematicalValueRefusal as M, MathematicalValueView,
    PreparedMathematicalValueBuilder, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
    MATHEMATICAL_VALUE_VALIDATION_WORK,
};
pub(crate) const CALL: &str = "math/sqrt";
pub(crate) struct Writer(PreparedMathematicalValueBuilder);
impl Writer {
    pub(crate) fn new() -> Result<Self, R> {
        PreparedMathematicalValueBuilder::new(131072, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)
            .map(Self)
            .map_err(R::SquareRoot)
    }
    pub(crate) fn encode(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, R> {
        let mut work = ExactNumericWork::new(3 * MATHEMATICAL_VALUE_VALIDATION_WORK);
        let value = MathematicalValueView::decode(input, &mut work).map_err(R::SquareRoot)?;
        let rational = value
            .as_rational(&mut work)
            .map_err(R::SquareRoot)?
            .ok_or(R::SquareRoot(M::UnsupportedDomainProof))?;
        self.0.reset();
        let argument = self
            .0
            .insert(MathematicalNode::Rational(rational), &mut work)
            .map_err(R::SquareRoot)?;
        let root = self
            .0
            .insert(MathematicalNode::PrincipalSquareRoot(argument), &mut work)
            .map_err(R::SquareRoot)?;
        self.0
            .encode(root, output, &mut work)
            .map_err(R::SquareRoot)
    }
}
pub(crate) fn check(
    arguments: &[crate::ExpressionSyntax],
    span: crate::Span,
    mut check_argument: impl FnMut(
        &crate::ExpressionSyntax,
        Option<&crate::CheckedExpressionType>,
    ) -> Result<
        crate::CheckedExpressionType,
        crate::ExpressionTypeDiagnostic,
    >,
) -> Result<crate::CheckedExpressionType, crate::ExpressionTypeDiagnostic> {
    let error = || crate::ExpressionTypeDiagnostic {
        span,
        message: alloc::string::String::from(
            "math/sqrt requires one exact dimensionless MathematicalValue",
        ),
    };
    let [argument] = arguments else {
        return Err(error());
    };
    let expected = crate::CheckedExpressionType::semantic(conduit_core::MATHEMATICAL_VALUE_ID);
    let actual = check_argument(argument, Some(&expected))?;
    if actual
        .value_kind()
        .is_none_or(|kind| kind.as_str() != conduit_core::MATHEMATICAL_VALUE_ID)
    {
        return Err(error());
    }
    Ok(expected)
}
pub(crate) fn evaluate(
    arguments: &[crate::expression_evaluate::Value],
    expected: &conduit_core::StructuredInfoType,
) -> Result<crate::expression_evaluate::Value, R> {
    let [argument] = arguments else {
        return Err(R::InvalidProgram);
    };
    let target = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
        conduit_core::MATHEMATICAL_VALUE_ID,
    ))
    .map_err(|_| R::InvalidProgram)?;
    if expected != &target || argument.value_type != target {
        return Err(R::InvalidProgram);
    }
    let mut encoded = alloc::vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = Writer::new()?.encode(&argument.encoded, &mut encoded)?;
    encoded.truncate(n);
    Ok(crate::expression_evaluate::Value {
        value_type: target,
        encoded,
    })
}
/// Stable pure-expression details after inverse-angle cases 20..=32.
pub fn square_root_failure_detail(reason: &M) -> u16 {
    match reason {
        M::NegativeRadicand => 33,
        M::UnsupportedDomainProof => 34,
        M::Numeric(conduit_core::ExactNumericRefusal::WorkLimit) => 35,
        _ => 36,
    }
}
