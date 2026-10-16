//! A finite reference constructor over concrete immutable Angle Source facts.
//! This grants no general startup evaluation of runtime expressions.
use super::*;
impl Resolver<'_> {
    pub(super) fn resolve_angle_reference(
        &mut self,
        container: &crate::Expression,
        syntax: &crate::ExpressionSyntax,
        expected: Option<&conduit_core::StructuredInfoType>,
    ) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        let refusal =
            |message: String| SyntaxCheckError::StructuredExpression(message, Some(syntax.span()));
        let target = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
            conduit_core::MATHEMATICAL_VALUE_ID,
        ))
        .expect("reviewed mathematical Type");
        if expected.is_some_and(|expected| expected != &target) {
            return Err(refusal(
                "Angle reference result requires MathematicalValue".into(),
            ));
        }
        let crate::ExpressionSyntax::SemanticCall {
            kind, arguments, ..
        } = syntax
        else {
            return Err(refusal("Angle reference requires exact call syntax".into()));
        };
        if kind.text != crate::expression_angle_reference::CALL || arguments.len() != 1 {
            return Err(refusal(
                "Angle reference requires exactly one concrete Angle".into(),
            ));
        }
        let argument = &arguments[0];
        let start = argument.span().start.checked_sub(container.span.start);
        let end = argument.span().end.checked_sub(container.span.start);
        let text = start
            .zip(end)
            .and_then(|(start, end)| container.text.get(start..end))
            .ok_or_else(|| refusal("Angle operand Source span is invalid".into()))?;
        let expression = crate::Expression {
            text: text.into(),
            syntax: argument.clone(),
            span: argument.span(),
        };
        // No expected Unit is supplied: a bare scalar must never acquire Angle.
        let CanonicalStartupValue::Quantity(quantity) =
            self.resolve_expression(&expression, None)?
        else {
            return Err(refusal(
                "Angle reference requires one concrete immutable Angle".into(),
            ));
        };
        let mut bytes = alloc::vec![0;crate::expression_angle_reference::MAXIMUM_ENCODED_BYTES];
        let length =
            crate::expression_angle_reference::encode(&quantity.value().encode(), &mut bytes)
                .map_err(|why| refusal(format!("exact Angle reference refused: {why:?}")))?;
        bytes.truncate(length);
        let value = conduit_core::StructuredInfoValue::leaf(target, bytes)
            .map_err(|why| refusal(format!("Angle reference result: {why:?}")))?;
        Ok(CanonicalStartupValue::Structured(
            crate::CanonicalStructuredStartupValue::from_owned_checked_value(value),
        ))
    }
}
