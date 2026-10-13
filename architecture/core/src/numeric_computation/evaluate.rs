//! Exact preparation-time operation semantics, shared by checking and witnesses.
//! Owned MathValue arithmetic may allocate; this is not an implicit Play algorithm.
use super::*;
use crate::{MathematicalEquality, ANGLE_INFO_ID, MATH_VALUE_INFO_ID};
use alloc::{boxed::Box, string::ToString};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericComputedValue {
    pub value: MathValue,
    pub semantic_kind: String,
    pub unit: Option<Box<Unit>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericComputedResult {
    Number(NumericComputedValue),
    Boolean(bool),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericEvaluationRefusal {
    Computation(NumericComputationRefusal),
    Arithmetic(NumericRefusal),
    MissingParameter,
    ParameterType,
    WrongArity,
    WrongSemanticType,
    MissingUnitEvidence,
    IncompatibleReferenceOrigins,
    UnsupportedSymbolicUnitLaw,
    UnknownComparison,
    UnsupportedOperation,
}
impl From<NumericRefusal> for NumericEvaluationRefusal {
    fn from(value: NumericRefusal) -> Self {
        Self::Arithmetic(value)
    }
}

/// Coordinate transformation uses the complete immutable scale, offset and role.
/// The result is a coordinate in the capsule's captured reference origin.
pub fn numeric_reference_coordinate(
    value: &NumericComputedValue,
) -> Result<MathValue, NumericEvaluationRefusal> {
    let unit = value
        .unit
        .as_ref()
        .ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?;
    let (family, role) = crate::parse_quantity_semantic_role(&value.semantic_kind)
        .ok_or(NumericEvaluationRefusal::WrongSemanticType)?;
    if family != unit.family().identity() {
        return Err(NumericEvaluationRefusal::WrongSemanticType);
    }
    let scale = unit.exact_scale().to_owned()?;
    let offset = unit
        .exact_offset(role)
        .map_err(|_| NumericEvaluationRefusal::WrongSemanticType)?
        .to_owned()?;
    Ok(value.value.checked_mul(&scale)?.checked_add(&offset)?)
}
fn scalar_operand(value: &NumericComputedValue) -> Result<&MathValue, NumericEvaluationRefusal> {
    if !super::is_scalar_kind(&value.semantic_kind) || value.unit.is_some() {
        return Err(NumericEvaluationRefusal::WrongSemanticType);
    }
    Ok(&value.value)
}
fn matching_reference(
    left: &NumericComputedValue,
    right: &NumericComputedValue,
) -> Result<(), NumericEvaluationRefusal> {
    if left.semantic_kind != right.semantic_kind
        && !(super::is_scalar_kind(&left.semantic_kind)
            && super::is_scalar_kind(&right.semantic_kind))
        && !(crate::parse_quantity_semantic_role(&left.semantic_kind).is_some()
            && crate::parse_quantity_semantic_role(&left.semantic_kind) == crate::parse_quantity_semantic_role(&right.semantic_kind))
    {
        return Err(NumericEvaluationRefusal::WrongSemanticType);
    }
    let left = left
        .unit
        .as_ref()
        .ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?;
    let right = right
        .unit
        .as_ref()
        .ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?;
    if left.reference_anchor() != right.reference_anchor() {
        return Err(NumericEvaluationRefusal::IncompatibleReferenceOrigins);
    }
    Ok(())
}

/// Applies one mathematical operation, including physical role/origin admission.
/// Trig coordinates are turns, derived from the checked source-owned Angle root.
pub fn apply_numeric_operation_with_reference(
    operation: NumericOperation,
    operands: &[NumericComputedValue],
    reference_unit: Option<&Unit>,
) -> Result<NumericComputedResult, NumericEvaluationRefusal> {
    if operands.len() != operation.arity() {
        return Err(NumericEvaluationRefusal::WrongArity);
    }
    let first = &operands[0];
    let mut semantic_kind = MATH_VALUE_INFO_ID.to_string();
    let mut unit = None;
    let value = match operation {
        NumericOperation::QuantityConstruction => {
            return Err(NumericEvaluationRefusal::MissingUnitEvidence)
        }
        NumericOperation::Sin | NumericOperation::Cos | NumericOperation::Tan => {
            if crate::parse_quantity_semantic_role(&first.semantic_kind) != crate::parse_quantity_semantic_role(ANGLE_INFO_ID) {
                return Err(NumericEvaluationRefusal::WrongSemanticType);
            }
            let turn = reference_unit.ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?;
            let actual = first
                .unit
                .as_ref()
                .ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?;
            if actual.reference_anchor() != turn.reference_anchor() {
                return Err(NumericEvaluationRefusal::UnsupportedSymbolicUnitLaw);
            }
            let reference = NumericComputedValue {
                value: MathValue::ONE,
                semantic_kind: ANGLE_INFO_ID.to_string(),
                unit: Some(Box::new(turn.clone())),
            };
            let coordinate = numeric_reference_coordinate(first)?
                .checked_div(&numeric_reference_coordinate(&reference)?)?;
            match operation {
                NumericOperation::Sin => coordinate.sin_turns()?,
                NumericOperation::Cos => coordinate.cos_turns()?,
                _ => coordinate.tan_turns()?,
            }
        }
        NumericOperation::Asin | NumericOperation::Acos | NumericOperation::Atan => {
            let coordinate = scalar_operand(first)?;
            semantic_kind = ANGLE_INFO_ID.to_string();
            unit = Some(Box::new(
                reference_unit
                    .ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?
                    .clone(),
            ));
            match operation {
                NumericOperation::Asin => coordinate.asin_turns()?,
                NumericOperation::Acos => coordinate.acos_turns()?,
                _ => coordinate.atan_turns()?,
            }
        }
        NumericOperation::Atan2 => {
            matching_reference(first, &operands[1])?;
            semantic_kind = ANGLE_INFO_ID.to_string();
            unit = Some(Box::new(
                reference_unit
                    .ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?
                    .clone(),
            ));
            MathValue::atan2_turns(
                &numeric_reference_coordinate(first)?,
                &numeric_reference_coordinate(&operands[1])?,
            )?
        }
        NumericOperation::Equal
        | NumericOperation::NotEqual
        | NumericOperation::Less
        | NumericOperation::LessOrEqual
        | NumericOperation::Greater
        | NumericOperation::GreaterOrEqual => {
            let (left, right) = if super::is_scalar_kind(&first.semantic_kind) {
                (
                    scalar_operand(first)?.clone(),
                    scalar_operand(&operands[1])?.clone(),
                )
            } else {
                matching_reference(first, &operands[1])?;
                (
                    numeric_reference_coordinate(first)?,
                    numeric_reference_coordinate(&operands[1])?,
                )
            };
            use core::cmp::Ordering;
            let result = match operation {
                NumericOperation::Equal | NumericOperation::NotEqual => {
                    let equal = match left.mathematical_equality(&right)? {
                        MathematicalEquality::Equal => true,
                        MathematicalEquality::NotEqual => false,
                        MathematicalEquality::Unknown => {
                            return Err(NumericEvaluationRefusal::UnknownComparison)
                        }
                    };
                    if operation == NumericOperation::Equal {
                        equal
                    } else {
                        !equal
                    }
                }
                _ => {
                    let order = left
                        .compare(&right)?
                        .ok_or(NumericEvaluationRefusal::UnknownComparison)?;
                    match operation {
                        NumericOperation::Less => order == Ordering::Less,
                        NumericOperation::LessOrEqual => order != Ordering::Greater,
                        NumericOperation::Greater => order == Ordering::Greater,
                        _ => order != Ordering::Less,
                    }
                }
            };
            return Ok(NumericComputedResult::Boolean(result));
        }
        NumericOperation::Negate => scalar_operand(first)?.checked_neg()?,
        NumericOperation::SquareRoot => scalar_operand(first)?.checked_root(2)?,
        NumericOperation::Add
        | NumericOperation::Subtract
        | NumericOperation::Multiply
        | NumericOperation::Divide
        | NumericOperation::Power
        | NumericOperation::Root => {
            let left = scalar_operand(first)?;
            let right = scalar_operand(&operands[1])?;
            match operation {
                NumericOperation::Add => left.checked_add(right)?,
                NumericOperation::Subtract => left.checked_sub(right)?,
                NumericOperation::Multiply => left.checked_mul(right)?,
                NumericOperation::Divide => left.checked_div(right)?,
                NumericOperation::Power => left.checked_pow(
                    right
                        .exact_i64()
                        .ok_or(NumericEvaluationRefusal::WrongSemanticType)?,
                )?,
                _ => left.checked_root(
                    u8::try_from(
                        right
                            .exact_i64()
                            .ok_or(NumericEvaluationRefusal::WrongSemanticType)?,
                    )
                    .map_err(|_| NumericEvaluationRefusal::WrongSemanticType)?,
                )?,
            }
        }
        _ => return Err(NumericEvaluationRefusal::UnsupportedOperation),
    };
    Ok(NumericComputedResult::Number(NumericComputedValue {
        value,
        semantic_kind,
        unit,
    }))
}

/// Apply a checked operation with its complete immutable output-role contract.
pub fn apply_numeric_operation_in_node(
    node: &NumericComputationNode,
    operands: &[NumericComputedValue],
) -> Result<NumericComputedResult, NumericEvaluationRefusal> {
    let NumericComputationOperation::Apply { operation, .. } = &node.operation else {
        return Err(NumericEvaluationRefusal::UnsupportedOperation);
    };
    if matches!(operation, NumericOperation::CyclicToAngularFrequency | NumericOperation::AngularToCyclicFrequency) {
        if operands.len() != 1 { return Err(NumericEvaluationRefusal::WrongArity); }
        let reference = node.reference_unit.as_ref().ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?;
        let output = node.unit.as_ref().ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?;
        let reference_value = NumericComputedValue { value: MathValue::ONE, semantic_kind: operands[0].semantic_kind.clone(), unit: Some(reference.clone()) };
        matching_reference(&operands[0], &reference_value)?;
        let value = numeric_reference_coordinate(&operands[0])?
            .checked_div(&numeric_reference_coordinate(&reference_value)?)?;
        return Ok(NumericComputedResult::Number(NumericComputedValue { value, semantic_kind: node.semantic_kind.clone(), unit: Some(output.clone()) }));
    }
    if *operation == NumericOperation::QuantityConstruction {
        if operands.len() != 1 {
            return Err(NumericEvaluationRefusal::WrongArity);
        }
        let coordinate = scalar_operand(&operands[0])?.clone();
        let unit = node
            .unit
            .as_ref()
            .ok_or(NumericEvaluationRefusal::MissingUnitEvidence)?;
        let (family, role) = crate::parse_quantity_semantic_role(&node.semantic_kind)
            .ok_or(NumericEvaluationRefusal::WrongSemanticType)?;
        if family != unit.family().identity()
            || !unit.admits_role(role)
            || node.reference_unit.is_some()
        {
            return Err(NumericEvaluationRefusal::WrongSemanticType);
        }
        return Ok(NumericComputedResult::Number(NumericComputedValue {
            value: coordinate,
            semantic_kind: node.semantic_kind.clone(),
            unit: Some(unit.clone()),
        }));
    }
    let mut result = apply_numeric_operation_with_reference(*operation, operands, node.reference_unit.as_deref())?;
    if let NumericComputedResult::Number(value) = &mut result {
        if crate::parse_quantity_semantic_role(&value.semantic_kind).is_some()
            && crate::parse_quantity_semantic_role(&value.semantic_kind) == crate::parse_quantity_semantic_role(&node.semantic_kind)
        {
            value.semantic_kind = node.semantic_kind.clone();
        }
    }
    Ok(result)
}

impl NumericComputation {
    /// Evaluates a closed graph or one with explicitly typed prepared parameters.
    pub fn evaluate_exact(
        &self,
        parameters: &[(u16, NumericComputedValue)],
    ) -> Result<NumericComputedResult, NumericEvaluationRefusal> {
        self.validate()
            .map_err(NumericEvaluationRefusal::Computation)?;
        if self
            .nodes
            .iter()
            .any(|node| node.semantic_kind == crate::F64_INFO_ID)
        {
            return Err(NumericEvaluationRefusal::UnsupportedOperation);
        }
        let mut values = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let mut result = match &node.operation {
                NumericComputationOperation::Literal(bytes) => {
                    NumericComputedResult::Number(NumericComputedValue {
                        value: MathValue::decode(bytes)?,
                        semantic_kind: node.semantic_kind.clone(),
                        unit: node.unit.clone(),
                    })
                }
                NumericComputationOperation::IeeeLiteral { .. }
                | NumericComputationOperation::IeeeRoundedLiteral { .. } => {
                    return Err(NumericEvaluationRefusal::UnsupportedOperation)
                }
                NumericComputationOperation::Parameter(slot) => {
                    let value = parameters
                        .iter()
                        .find(|(actual, _)| actual == slot)
                        .map(|(_, value)| value)
                        .ok_or(NumericEvaluationRefusal::MissingParameter)?;
                    if super::is_scalar_kind(&value.semantic_kind) {
                        if value.unit.is_some() {
                            return Err(NumericEvaluationRefusal::ParameterType);
                        }
                    } else {
                        numeric_reference_coordinate(value)?;
                    }
                    if value.semantic_kind != node.semantic_kind
                        || node
                            .unit
                            .as_ref()
                            .is_some_and(|unit| value.unit.as_ref() != Some(unit))
                    {
                        return Err(NumericEvaluationRefusal::ParameterType);
                    }
                    NumericComputedResult::Number(value.clone())
                }
                NumericComputationOperation::Apply {
                    operation: _,
                    operands,
                } => {
                    let operands = operands
                        .iter()
                        .map(|index| match &values[usize::from(*index)] {
                            NumericComputedResult::Number(value) => Ok(value.clone()),
                            _ => Err(NumericEvaluationRefusal::WrongSemanticType),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    apply_numeric_operation_in_node(node, &operands)?
                }
            };
            if let NumericComputedResult::Number(value) = &mut result {
                if super::is_scalar_kind(&value.semantic_kind)
                    && super::is_scalar_kind(&node.semantic_kind)
                {
                    let bytes = value.value.canonical_bytes()?;
                    crate::validate_primitive_info(&node.semantic_kind, &bytes)
                        .map_err(|_| NumericEvaluationRefusal::WrongSemanticType)?;
                    value.semantic_kind = node.semantic_kind.clone();
                }
            }
            let actual_kind = match &result {
                NumericComputedResult::Number(value) => &value.semantic_kind,
                NumericComputedResult::Boolean(_) => crate::BOOL_INFO_ID,
            };
            if actual_kind != node.semantic_kind {
                return Err(NumericEvaluationRefusal::WrongSemanticType);
            }
            if let NumericComputedResult::Number(value) = &result {
                if node
                    .unit
                    .as_ref()
                    .is_some_and(|expected| value.unit.as_ref() != Some(expected))
                {
                    return Err(NumericEvaluationRefusal::WrongSemanticType);
                }
            }
            values.push(result);
        }
        Ok(values[usize::from(self.root)].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PrefixPolicy, QuantityRole, UnitDefinition};
    use alloc::vec;
    #[test]
    fn angle_reference_snapshot_survives_a_changed_default_origin() {
        let family = crate::builtin_angle_unit().family();
        let one = MathValue::ONE;
        let zero = MathValue::ZERO;
        // Historical authored root is degrees; its source-owned turn is 360 degrees.
        let degree = UnitDefinition::new_math_role(
            family,
            "°",
            one,
            zero.clone(),
            PrefixPolicy::NONE,
            QuantityRole::Linear,
        )
        .unwrap();
        let turn = UnitDefinition::related_math_role(
            &degree,
            "turn",
            MathValue::integer(360),
            zero,
            PrefixPolicy::NONE,
            QuantityRole::Linear,
        )
        .unwrap();
        let literal = NumericComputationNode::literal(
            ANGLE_INFO_ID.to_string(),
            Some(Box::new(Unit::from_definition(degree))),
            &MathValue::integer(90),
        )
        .unwrap();
        let call = NumericComputationNode {
            semantic_kind: MATH_VALUE_INFO_ID.to_string(),
            unit: None,
            reference_unit: Some(Box::new(Unit::from_definition(turn))),
            operation: NumericComputationOperation::Apply {
                operation: NumericOperation::Sin,
                operands: vec![0],
            },
        };
        let graph = NumericComputation::new(vec![literal, call], 1).unwrap();
        let retained =
            NumericComputation::decode_validate(&graph.canonical_bytes().unwrap()).unwrap();
        assert_eq!(
            retained.evaluate_exact(&[]).unwrap(),
            NumericComputedResult::Number(NumericComputedValue {
                value: MathValue::ONE,
                semantic_kind: MATH_VALUE_INFO_ID.to_string(),
                unit: None,
            })
        );
        let mut swapped = retained.nodes().to_vec();
        swapped[1].reference_unit = Some(Box::new(crate::builtin_angle_unit()));
        let swapped = NumericComputation::new(swapped, 1).unwrap();
        assert_eq!(
            swapped.evaluate_exact(&[]),
            Err(NumericEvaluationRefusal::UnsupportedSymbolicUnitLaw)
        );
    }
    #[test]
    fn forged_operation_result_metadata_cannot_become_custody() {
        let literal =
            NumericComputationNode::literal(MATH_VALUE_INFO_ID.to_string(), None, &MathValue::ONE)
                .unwrap();
        let call = NumericComputationNode {
            semantic_kind: ANGLE_INFO_ID.to_string(),
            unit: None,
            reference_unit: None,
            operation: NumericComputationOperation::Apply {
                operation: NumericOperation::Add,
                operands: vec![0, 0],
            },
        };
        assert_eq!(
            NumericComputation::new(vec![literal, call], 1),
            Err(NumericComputationRefusal::InvalidSemanticKind)
        );
    }
}
