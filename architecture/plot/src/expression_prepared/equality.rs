//! Prepared exact-Type equality follows the ordinary structured equality law.
use super::*;
use conduit_core::{
    validate_canonical_structured_value, StructuredInfoType, StructuredInfoTypeShape,
};

pub(super) struct PreparedEquality {
    left: PreparedPortableExpressionEvaluator,
    right: PreparedPortableExpressionEvaluator,
    value_type: StructuredInfoType,
    type_bytes: Vec<u8>,
    negate: bool,
}

pub(super) fn binary(
    operator: BinaryOperator,
    proven: bool,
    output_kind: PrimitiveInfoKind,
    left: &PortableExpressionNode,
    right: &PortableExpressionNode,
    input_type: &StructuredInfoType,
    input: &PreparedInput,
) -> Result<PreparedOperation, Refusal> {
    if leaf_kind(&left.value_type).is_ok() {
        return Ok(PreparedOperation::Binary {
            operator,
            proven,
            left: Box::new(prepare_node(left, input_type, input)?),
            right: Box::new(prepare_node(right, input_type, input)?),
        });
    }
    if output_kind != PrimitiveInfoKind::Bool
        || left.value_type != right.value_type
        || !matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
    {
        return Err(Refusal::InvalidProgram);
    }
    let child = |node: &PortableExpressionNode| {
        PreparedPortableExpressionEvaluator::prepare(
            ProgramView {
                input_type,
                output_type: &node.value_type,
                root: node,
            },
            input.clone(),
        )
    };
    Ok(PreparedOperation::Equality(Box::new(PreparedEquality {
        left: child(left)?,
        right: child(right)?,
        value_type: left.value_type.clone(),
        type_bytes: left
            .value_type
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?,
        negate: operator == BinaryOperator::NotEqual,
    })))
}

impl PreparedEquality {
    pub(super) fn evaluate(&mut self, input: &[u8]) -> Result<PrimitiveValue<'static>, Refusal> {
        let left = validate_canonical_structured_value(self.left.evaluate(input)?)
            .map_err(|_| Refusal::InvalidProgram)?;
        let right = validate_canonical_structured_value(self.right.evaluate(input)?)
            .map_err(|_| Refusal::InvalidProgram)?;
        if left.type_bytes() != self.type_bytes || right.type_bytes() != self.type_bytes {
            return Err(Refusal::InvalidProgram);
        }
        let equal = equal(
            &self.value_type,
            &mut Cursor(left.value_node()),
            &mut Cursor(right.value_node()),
        )?;
        PrimitiveValue::new(PrimitiveInfoKind::Bool, &[u8::from(equal != self.negate)])
    }
}

// Both frames have passed Core's finite depth, node, shape and primitive checks.
// Traverse their value nodes once, without constructing owned value trees.
struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Refusal> {
        let value = self.0.get(..n).ok_or(Refusal::InvalidProgram)?;
        self.0 = &self.0[n..];
        Ok(value)
    }
    fn tag(&mut self, expected: u8) -> Result<(), Refusal> {
        if self.take(1)? == [expected] {
            Ok(())
        } else {
            Err(Refusal::InvalidProgram)
        }
    }
    fn length(&mut self) -> Result<usize, Refusal> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| Refusal::InvalidProgram)?,
        ) as usize)
    }
    fn bytes(&mut self) -> Result<&'a [u8], Refusal> {
        let length = self.length()?;
        self.take(length)
    }
}
fn equal(ty: &StructuredInfoType, a: &mut Cursor<'_>, b: &mut Cursor<'_>) -> Result<bool, Refusal> {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => equal(representation, a, b),
        StructuredInfoTypeShape::Leaf(_) => {
            a.tag(0)?;
            b.tag(0)?;
            let left = a.bytes()?;
            let right = b.bytes()?;
            let kind = leaf_kind(ty)?;
            if matches!(
                kind,
                PrimitiveInfoKind::F32 | PrimitiveInfoKind::F64 | PrimitiveInfoKind::Unit
            ) {
                return Ok(left == right);
            }
            let result = evaluate_binary(
                BinaryOperator::Equal,
                false,
                PrimitiveInfoKind::Bool,
                &PrimitiveValue::borrowed(kind, left)?,
                &PrimitiveValue::borrowed(kind, right)?,
            )?;
            decode_bool(&result)
        }
        StructuredInfoTypeShape::Record { fields, .. } => {
            a.tag(2)?;
            b.tag(2)?;
            if a.length()? != fields.len() || b.length()? != fields.len() {
                return Err(Refusal::InvalidProgram);
            }
            for field in fields {
                if a.bytes()? != field.name().as_bytes() || b.bytes()? != field.name().as_bytes() {
                    return Err(Refusal::InvalidProgram);
                }
                if !equal(field.value_type(), a, b)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => {
            a.tag(1)?;
            b.tag(1)?;
            let count = a.length()?;
            if count != b.length()? {
                return Ok(false);
            }
            for _ in 0..count {
                if !equal(element, a, b)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            a.tag(3)?;
            b.tag(3)?;
            let tag = a.bytes()?;
            if tag != b.bytes()? {
                return Ok(false);
            }
            let case = cases
                .iter()
                .find(|case| case.tag().as_bytes() == tag)
                .ok_or(Refusal::InvalidProgram)?;
            equal(case.payload_type(), a, b)
        }
    }
}
