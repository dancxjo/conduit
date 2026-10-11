//! Physical constructors keep large finite Unit intermediates off alias recursion frames.
use super::*;

impl Resolver<'_> {
    // Preserve the separate frame when release LTO considers inlining.
    #[inline(never)]
    pub(super) fn resolve_quantity_constructor(
        &mut self,
        expression: &crate::Expression,
        expected: Option<&conduit_core::StructuredInfoType>,
        kind: &str,
        arguments: &[crate::ExpressionSyntax],
    ) -> Result<CanonicalStartupValue, SyntaxCheckError> {
        let role = self.catalog.physical.quantities[kind].clone();
        if arguments.len() != 2 {
            return Err(SyntaxCheckError::QuantityLiteral(
                "quantity constructor requires coordinate and Unit".into(),
            ));
        }
        let unit_syntax = &arguments[1];
        let start = unit_syntax
            .span()
            .start
            .saturating_sub(expression.span.start);
        let end = unit_syntax.span().end.saturating_sub(expression.span.start);
        let unit_expression = crate::Expression {
            text: expression
                .text
                .get(start..end)
                .ok_or_else(|| {
                    SyntaxCheckError::QuantityLiteral(
                        "constructor operand source span is invalid".into(),
                    )
                })?
                .into(),
            syntax: unit_syntax.clone(),
            span: unit_syntax.span(),
        };
        let unit_type = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
            conduit_core::UNIT_INFO_ID,
        ))
        .expect("Unit leaf");
        let unit_value = self.resolve_expression(&unit_expression, Some(&unit_type))?;
        let CanonicalStartupValue::Unit(unit_value) = unit_value else {
            return Err(SyntaxCheckError::QuantityLiteral(
                "quantity constructor operand is not a concrete Unit".into(),
            ));
        };
        let unit = unit_value.value();
        if unit.family() != role.family {
            return Err(SyntaxCheckError::QuantityLiteral(
                "quantity constructor Unit belongs to another family".into(),
            ));
        }
        let number = &arguments[0];
        let start = number.span().start.saturating_sub(expression.span.start);
        let end = number.span().end.saturating_sub(expression.span.start);
        let coordinate = expression.text.get(start..end).ok_or_else(|| {
            SyntaxCheckError::QuantityLiteral(
                "constructor coordinate source span is invalid".into(),
            )
        })?;
        let resolved_coordinate = if self.locals.contains_key(coordinate) {
            match self.resolve_name(coordinate, None)? {
                CanonicalStartupValue::Literal(value) => value,
                _ => {
                    return Err(SyntaxCheckError::QuantityLiteral(
                        "quantity constructor coordinate is not a concrete decimal".into(),
                    ))
                }
            }
        } else {
            coordinate.to_string()
        };
        let evidence = format!("{}({}, {})", kind, resolved_coordinate, unit.symbol());
        let quantity = conduit_core::Quantity::parse_with_unit_evidence(&evidence, unit, role.role)
            .map_err(|reason| {
                SyntaxCheckError::QuantityLiteral(format!(
                    "invalid constructor coordinate: {reason:?}"
                ))
            })?;
        let value = CanonicalStartupValue::Quantity(
            conduit_core::QuantityConfigurationValue::new(quantity, evidence)
                .expect("checked resolved constructor evidence"),
        );
        if expected.is_some_and(|ty| crate::authored_quantity::bytes(&value, ty).is_none()) {
            return Err(SyntaxCheckError::QuantityLiteral(
                "quantity constructor family or role differs from expected Type".into(),
            ));
        }
        Ok(value)
    }
}
