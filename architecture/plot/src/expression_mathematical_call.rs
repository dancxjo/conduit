//! Mathematical call signatures; representation selection belongs to planning.
use crate::{CheckedExpressionType, ExpressionSyntax, ExpressionTypeDiagnostic, Span};
use conduit_core::{ANGLE_INFO_ID, MATH_VALUE_INFO_ID};

pub(crate) fn check(
    name: &str,
    arguments: &[ExpressionSyntax],
    span: Span,
    mut argument: impl FnMut(&ExpressionSyntax, Option<&CheckedExpressionType>)
        -> Result<CheckedExpressionType, ExpressionTypeDiagnostic>,
) -> Option<Result<CheckedExpressionType, ExpressionTypeDiagnostic>> {
    if matches!(name, "F64" | "ieee/binary64-bits") {
        return Some((|| {
            if arguments.len() != 1 {
                return Err(ExpressionTypeDiagnostic { span, message: "IEEE constructor requires one operand".into() });
            }
            let expected = CheckedExpressionType::semantic(if name == "F64" {
                MATH_VALUE_INFO_ID
            } else {
                "value/u64"
            });
            if argument(&arguments[0], Some(&expected))? != expected {
                return Err(ExpressionTypeDiagnostic { span, message: "IEEE constructor operand has the wrong explicit type".into() });
            }
            Ok(CheckedExpressionType::semantic(conduit_core::F64_INFO_ID))
        })());
    }
    let arity = match name {
        "math/sin" | "math/cos" | "math/tan" | "math/asin" | "math/acos" |
        "math/atan" | "math/sqrt" | "math/cyclic-to-angular-frequency" | "math/angular-to-cyclic-frequency" => 1,
        "math/pow" | "math/root" | "math/atan2" => 2,
        _ => return None,
    };
    Some((|| {
        let refuse = |message: &str| ExpressionTypeDiagnostic { span, message: message.into() };
        if arguments.len() != arity { return Err(refuse("mathematical operation has the wrong arity")); }
        let scalar = CheckedExpressionType::semantic(MATH_VALUE_INFO_ID);
        let angle = CheckedExpressionType::semantic(ANGLE_INFO_ID);
        match name {
            "math/cyclic-to-angular-frequency" | "math/angular-to-cyclic-frequency" => {
                let (source, target) = if name == "math/cyclic-to-angular-frequency" {
                    (conduit_core::FREQUENCY_INFO_ID, conduit_core::ANGULAR_FREQUENCY_INFO_ID)
                } else { (conduit_core::ANGULAR_FREQUENCY_INFO_ID, conduit_core::FREQUENCY_INFO_ID) };
                let actual = argument(&arguments[0], None)?;
                if actual.value_kind().and_then(|kind| conduit_core::parse_quantity_semantic_role(kind.as_str()))
                    != conduit_core::parse_quantity_semantic_role(source) {
                    return Err(refuse("frequency bridge requires its declared linear frequency family"));
                }
                Ok(CheckedExpressionType::semantic(target))
            }
            "math/sin" | "math/cos" | "math/tan" => {
                // No expected Angle: a bare number must never acquire an angle unit.
                if argument(&arguments[0], None)?.value_kind().and_then(|kind| conduit_core::parse_quantity_semantic_role(kind.as_str())) != conduit_core::parse_quantity_semantic_role(ANGLE_INFO_ID) {
                    return Err(refuse("trigonometry requires the checked Angle family; bare numbers and other quantities are not angles"));
                }
                Ok(scalar)
            }
            "math/atan2" => {
                let y = argument(&arguments[0], None)?;
                let x = argument(&arguments[1], None)?;
                let roles = |value: &CheckedExpressionType| value.value_kind()
                    .and_then(|kind| conduit_core::parse_quantity_semantic_role(kind.as_str()));
                let Some((family, role)) = roles(&y) else { return Err(refuse("atan2 requires unit-bearing quantities")); };
                if roles(&x) != Some((family, role)) {
                    return Err(refuse("atan2 requires the same quantity family and semantic role"));
                }
                // Domain and source-origin laws are established by exact admission.
                Ok(angle)
            }
            _ => {
                for value in arguments {
                    if argument(value, Some(&scalar))? != scalar {
                        return Err(refuse("mathematical operation requires exact mathematical numbers"));
                    }
                }
                if matches!(name, "math/asin" | "math/acos" | "math/atan") { Ok(angle) } else { Ok(scalar) }
            }
        }
    })())
}
