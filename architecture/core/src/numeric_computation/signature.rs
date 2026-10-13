//! Recheck typed operation metadata before a graph becomes retained custody.
use super::*;

pub(super) fn validate(
    node: &NumericComputationNode,
    operation: NumericOperation,
    operands: &[&NumericComputationNode],
) -> Result<(), NumericComputationRefusal> {
    use NumericOperation::*;
    let wrong = NumericComputationRefusal::InvalidSemanticKind;
    let scalar = |value: &NumericComputationNode| {
        is_scalar_kind(&value.semantic_kind) && value.unit.is_none()
    };
    let angle = |value: &NumericComputationNode| crate::parse_quantity_semantic_role(&value.semantic_kind) == crate::parse_quantity_semantic_role(crate::ANGLE_INFO_ID);
    let quantity =
        |value: &NumericComputationNode| crate::parse_quantity_semantic_role(&value.semantic_kind);
    let ieee = |value: &NumericComputationNode| {
        value.semantic_kind == crate::F64_INFO_ID
            && value.unit.is_none()
            && value.reference_unit.is_none()
    };
    if ieee(node) || operands.iter().any(|value| ieee(value)) {
        let accepted = match operation {
            Negate | Add | Subtract | Multiply | Divide => {
                ieee(node) && operands.iter().all(|value| ieee(value))
            }
            Equal | NotEqual | Less | LessOrEqual | Greater | GreaterOrEqual => {
                node.semantic_kind == crate::BOOL_INFO_ID
                    && node.unit.is_none()
                    && node.reference_unit.is_none()
                    && operands.iter().all(|value| ieee(value))
            }
            _ => false,
        };
        return if accepted { Ok(()) } else { Err(wrong) };
    }
    match operation {
        QuantityConstruction => {
            if !scalar(operands[0])
                || quantity(node).is_none()
                || node.unit.is_none()
                || node.reference_unit.is_some()
            {
                return Err(wrong);
            }
        }
        Negate | Add | Subtract | Multiply | Divide | Power | Root | SquareRoot => {
            if !scalar(node)
                || !operands.iter().all(|value| scalar(value))
                || node.reference_unit.is_some()
            {
                return Err(wrong);
            }
        }
        Sin | Cos | Tan => {
            if !scalar(node) || !angle(operands[0]) || node.reference_unit.is_none() {
                return Err(wrong);
            }
        }
        Asin | Acos | Atan => {
            if !angle(node)
                || !scalar(operands[0])
                || node.unit.is_none()
                || node.reference_unit.as_ref() != node.unit.as_ref()
            {
                return Err(wrong);
            }
        }
        Atan2 => {
            if !angle(node)
                || node.unit.is_none()
                || node.reference_unit.as_ref() != node.unit.as_ref()
                || quantity(operands[0]).is_none()
                || quantity(operands[0]) != quantity(operands[1])
            {
                return Err(wrong);
            }
            if let (Some(left), Some(right)) = (&operands[0].unit, &operands[1].unit) {
                if left.reference_anchor() != right.reference_anchor() {
                    return Err(NumericComputationRefusal::InvalidUnit);
                }
            }
        }
        Equal | NotEqual | Less | LessOrEqual | Greater | GreaterOrEqual => {
            if node.semantic_kind != crate::BOOL_INFO_ID
                || node.unit.is_some()
                || node.reference_unit.is_some()
                || (operands[0].semantic_kind != operands[1].semantic_kind
                    && !(scalar(operands[0]) && scalar(operands[1])))
                || (!scalar(operands[0]) && quantity(operands[0]).is_none())
            {
                return Err(wrong);
            }
        }
        CyclicToAngularFrequency | AngularToCyclicFrequency => {
            let (source, target) = if operation == CyclicToAngularFrequency {
                (crate::FREQUENCY_INFO_ID, crate::ANGULAR_FREQUENCY_INFO_ID)
            } else { (crate::ANGULAR_FREQUENCY_INFO_ID, crate::FREQUENCY_INFO_ID) };
            if quantity(operands[0]) != crate::parse_quantity_semantic_role(source)
                || quantity(node) != crate::parse_quantity_semantic_role(target)
                || node.unit.is_none() || node.reference_unit.is_none() {
                return Err(wrong);
            }
            if operands[0].unit.as_ref().map(|unit| unit.reference_anchor())
                != node.reference_unit.as_ref().map(|unit| unit.reference_anchor()) {
                return Err(NumericComputationRefusal::InvalidUnit);
            }
        }
    }
    Ok(())
}
