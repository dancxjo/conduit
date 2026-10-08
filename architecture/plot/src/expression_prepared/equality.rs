//! Prepared recursive semantic equality; canonical framing is not semantic equality.
use super::PreparationBudget;
use super::{
    primitive, EvaluationInput, PreparedInput, PreparedPortableExpressionEvaluator, ProgramView,
    Refusal,
};
use crate::{BinaryOperator, PortableExpressionNode};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{
    primitive_info_kind, PrimitiveInfoKind, StructuredInfoType, StructuredInfoTypeShape,
    ValidatedCanonicalStructuredValue,
};

pub(super) struct PreparedEquality {
    left: Box<PreparedPortableExpressionEvaluator>,
    right: Box<PreparedPortableExpressionEvaluator>,
    expected: Vec<u8>,
    shape: Shape,
    negate: bool,
}

enum Shape {
    Primitive(PrimitiveInfoKind),
    Record(Vec<(String, Shape)>),
    Collection(Box<Shape>),
    Variant(Vec<(String, Shape)>),
}

impl PreparedEquality {
    pub(super) fn new(
        operator: BinaryOperator,
        left: &PortableExpressionNode,
        right: &PortableExpressionNode,
        input_type: &StructuredInfoType,
        input: &PreparedInput,
        budget: &mut PreparationBudget,
    ) -> Result<Self, Refusal> {
        if left.value_type != right.value_type
            || !matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
        {
            return Err(Refusal::InvalidProgram);
        }
        budget.array::<PreparedPortableExpressionEvaluator>(2)?;
        budget.prefix(&left.value_type)?;
        if budget.is_bounded() {
            budget.reserve(Shape::preparation_bound(&left.value_type)?)?;
        }
        let mut prepare = |node: &PortableExpressionNode| {
            PreparedPortableExpressionEvaluator::prepare(
                ProgramView {
                    input_type,
                    output_type: &node.value_type,
                    root: node,
                },
                input.clone(),
                budget,
            )
        };
        Ok(Self {
            left: Box::new(prepare(left)?),
            right: Box::new(prepare(right)?),
            expected: left
                .value_type
                .canonical_bytes()
                .map_err(|_| Refusal::InvalidProgram)?,
            shape: Shape::new(&left.value_type)?,
            negate: operator == BinaryOperator::NotEqual,
        })
    }
    pub(super) fn evaluate(
        &mut self,
        input: EvaluationInput<'_>,
    ) -> Result<primitive::PrimitiveValue<'static>, Refusal> {
        let left =
            conduit_core::validate_canonical_structured_value(self.left.evaluate_input(input)?)
                .map_err(|_| Refusal::InvalidInput)?;
        let right =
            conduit_core::validate_canonical_structured_value(self.right.evaluate_input(input)?)
                .map_err(|_| Refusal::InvalidInput)?;
        if left.type_bytes() != self.expected || right.type_bytes() != self.expected {
            return Err(Refusal::InvalidProgram);
        }
        let equal = self.shape.compare(left, right)?;
        primitive::PrimitiveValue::new(PrimitiveInfoKind::Bool, &[u8::from(equal != self.negate)])
    }
}

impl Shape {
    fn new(ty: &StructuredInfoType) -> Result<Self, Refusal> {
        Ok(match ty.shape() {
            StructuredInfoTypeShape::Leaf(kind) => {
                let primitive = primitive_info_kind(kind.as_str())
                    .ok_or_else(|| Refusal::UnsupportedType(kind.as_str().into()))?;
                if !matches!(
                    primitive,
                    PrimitiveInfoKind::Unit
                        | PrimitiveInfoKind::Bool
                        | PrimitiveInfoKind::Text
                        | PrimitiveInfoKind::Count
                        | PrimitiveInfoKind::Scalar
                        | PrimitiveInfoKind::F32
                        | PrimitiveInfoKind::F64
                ) && !super::fixed_integer(primitive)
                    && !super::quantity_kind(primitive)
                {
                    return Err(Refusal::UnsupportedType(kind.as_str().into()));
                }
                Self::Primitive(primitive)
            }
            StructuredInfoTypeShape::Nominal { representation, .. }
                if matches!(representation.shape(), StructuredInfoTypeShape::Leaf(_)) =>
            {
                Self::new(representation)?
            }
            StructuredInfoTypeShape::Record { fields, .. } => Self::Record(
                fields
                    .iter()
                    .map(|field| Ok((field.name().into(), Self::new(field.value_type())?)))
                    .collect::<Result<_, Refusal>>()?,
            ),
            StructuredInfoTypeShape::Collection { element, .. }
            | StructuredInfoTypeShape::Sequence { element, .. } => {
                Self::Collection(Box::new(Self::new(element)?))
            }
            StructuredInfoTypeShape::Variant { cases, .. } => Self::Variant(
                cases
                    .iter()
                    .map(|case| Ok((case.tag().into(), Self::new(case.payload_type())?)))
                    .collect::<Result<_, Refusal>>()?,
            ),
            _ => {
                return Err(Refusal::UnsupportedType(
                    "nominal structured equality".into(),
                ))
            }
        })
    }
    fn compare(
        &self,
        left: ValidatedCanonicalStructuredValue<'_>,
        right: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<bool, Refusal> {
        if left.type_bytes() != right.type_bytes() {
            return Ok(false);
        }
        match self {
            Self::Primitive(kind) => {
                let left = leaf_bytes(left)?;
                let right = leaf_bytes(right)?;
                if matches!(
                    kind,
                    PrimitiveInfoKind::F32 | PrimitiveInfoKind::F64 | PrimitiveInfoKind::Unit
                ) {
                    return Ok(left == right);
                }
                let left = primitive::PrimitiveValue::borrowed(*kind, left)?;
                let right = primitive::PrimitiveValue::borrowed(*kind, right)?;
                primitive::decode_bool(&primitive::evaluate_binary(
                    BinaryOperator::Equal,
                    false,
                    PrimitiveInfoKind::Bool,
                    &left,
                    &right,
                )?)
            }
            Self::Record(fields) => {
                for (name, shape) in fields {
                    let left = left
                        .record_field(name)
                        .map_err(|_| Refusal::InvalidInput)?
                        .ok_or(Refusal::InvalidInput)?;
                    let right = right
                        .record_field(name)
                        .map_err(|_| Refusal::InvalidInput)?
                        .ok_or(Refusal::InvalidInput)?;
                    if !shape.compare(left, right)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Self::Collection(shape) => {
                let count = left
                    .collection_length()
                    .map_err(|_| Refusal::InvalidInput)?;
                if count
                    != right
                        .collection_length()
                        .map_err(|_| Refusal::InvalidInput)?
                {
                    return Ok(false);
                }
                for index in 0..count {
                    let index = u16::try_from(index).map_err(|_| Refusal::InvalidInput)?;
                    let left = left
                        .collection_index(index)
                        .map_err(|_| Refusal::InvalidInput)?
                        .ok_or(Refusal::InvalidInput)?;
                    let right = right
                        .collection_index(index)
                        .map_err(|_| Refusal::InvalidInput)?
                        .ok_or(Refusal::InvalidInput)?;
                    if !shape.compare(left, right)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Self::Variant(cases) => {
                let tag = left.variant_tag().map_err(|_| Refusal::InvalidInput)?;
                if tag != right.variant_tag().map_err(|_| Refusal::InvalidInput)? {
                    return Ok(false);
                }
                let (_, shape) = cases
                    .iter()
                    .find(|(name, _)| name == tag)
                    .ok_or(Refusal::InvalidProgram)?;
                let left = left
                    .variant_payload(tag)
                    .map_err(|_| Refusal::InvalidInput)?
                    .ok_or(Refusal::InvalidInput)?;
                let right = right
                    .variant_payload(tag)
                    .map_err(|_| Refusal::InvalidInput)?
                    .ok_or(Refusal::InvalidInput)?;
                shape.compare(left, right)
            }
        }
    }
}
fn leaf_bytes(value: ValidatedCanonicalStructuredValue<'_>) -> Result<&[u8], Refusal> {
    let [0, length @ ..] = value.value_node() else {
        return Err(Refusal::InvalidInput);
    };
    let bytes = length.get(4..).ok_or(Refusal::InvalidInput)?;
    let declared = u32::from_le_bytes(
        length
            .get(..4)
            .ok_or(Refusal::InvalidInput)?
            .try_into()
            .map_err(|_| Refusal::InvalidInput)?,
    );
    if bytes.len() != declared as usize {
        return Err(Refusal::InvalidInput);
    }
    Ok(bytes)
}

impl PreparedEquality {
    pub(super) fn owned_heap_bytes(&self) -> usize {
        super::storage::boxed(self.left.as_ref(), self.left.owned_heap_bytes())
            .saturating_add(super::storage::boxed(
                self.right.as_ref(),
                self.right.owned_heap_bytes(),
            ))
            .saturating_add(self.expected.capacity())
            .saturating_add(self.shape.owned_heap_bytes())
    }
}
impl Shape {
    fn owned_heap_bytes(&self) -> usize {
        match self {
            Self::Primitive(_) => 0,
            Self::Collection(element) => {
                super::storage::boxed(element.as_ref(), element.owned_heap_bytes())
            }
            Self::Record(fields) | Self::Variant(fields) => fields.iter().fold(
                fields
                    .capacity()
                    .saturating_mul(core::mem::size_of::<(String, Shape)>()),
                |total, (name, shape)| {
                    total
                        .saturating_add(name.capacity())
                        .saturating_add(shape.owned_heap_bytes())
                },
            ),
        }
    }
}

impl Shape {
    fn preparation_bound(ty: &StructuredInfoType) -> Result<usize, Refusal> {
        fn add(a: usize, b: usize) -> Result<usize, Refusal> {
            a.checked_add(b).ok_or(Refusal::InvalidProgram)
        }
        fn vector(count: usize) -> Result<usize, Refusal> {
            count
                .max(4)
                .checked_mul(3)
                .and_then(|count| count.checked_mul(core::mem::size_of::<(String, Shape)>()))
                .ok_or(Refusal::InvalidProgram)
        }
        match ty.shape() {
            StructuredInfoTypeShape::Leaf(kind) => Ok(kind.as_str().len()), // possible allocated refusal
            StructuredInfoTypeShape::Nominal { representation, .. }
                if matches!(representation.shape(), StructuredInfoTypeShape::Leaf(_)) =>
            {
                Self::preparation_bound(representation)
            }
            StructuredInfoTypeShape::Nominal { .. } => Ok(64), // static unsupported-shape refusal
            StructuredInfoTypeShape::Collection { element, .. }
            | StructuredInfoTypeShape::Sequence { element, .. } => add(
                core::mem::size_of::<Self>(),
                Self::preparation_bound(element)?,
            ),
            StructuredInfoTypeShape::Record { fields, .. } => {
                fields.iter().try_fold(vector(fields.len())?, |sum, field| {
                    add(
                        add(sum, field.name().len())?,
                        Self::preparation_bound(field.value_type())?,
                    )
                })
            }
            StructuredInfoTypeShape::Variant { cases, .. } => {
                cases.iter().try_fold(vector(cases.len())?, |sum, case| {
                    add(
                        add(sum, case.tag().len())?,
                        Self::preparation_bound(case.payload_type())?,
                    )
                })
            }
        }
    }
}
