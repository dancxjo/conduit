//! Conservative encoded scratch bounds from the exact checked Type.
use super::Refusal;
use conduit_core::{
    fixed_integer_bytes, primitive_info_kind, PrimitiveInfoKind, StructuredInfoType,
    StructuredInfoTypeShape, MAXIMUM_STRUCTURED_CANONICAL_BYTES, MAXIMUM_STRUCTURED_LEAF_BYTES,
};

pub(super) fn output(ty: &StructuredInfoType) -> Result<usize, Refusal> {
    if let StructuredInfoTypeShape::Leaf(kind) = ty.shape() {
        return Ok(leaf(kind.as_str()));
    }
    canonical(ty)
}
pub(super) fn canonical(ty: &StructuredInfoType) -> Result<usize, Refusal> {
    let prefix = ty
        .canonical_bytes()
        .map_err(|_| Refusal::InvalidProgram)?
        .len();
    Ok(add(prefix, node(ty)).min(MAXIMUM_STRUCTURED_CANONICAL_BYTES))
}
fn node(ty: &StructuredInfoType) -> usize {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind) => add(5, leaf(kind.as_str())),
        StructuredInfoTypeShape::Nominal { representation, .. } => node(representation),
        StructuredInfoTypeShape::Collection { element, length } => {
            add(5, node(element).saturating_mul(usize::from(length)))
        }
        StructuredInfoTypeShape::Sequence {
            element,
            maximum_items,
            ..
        } => add(5, node(element).saturating_mul(usize::from(maximum_items))),
        StructuredInfoTypeShape::Record { fields, .. } => fields.iter().fold(5, |size, field| {
            add(size, add(4 + field.name().len(), node(field.value_type())))
        }),
        StructuredInfoTypeShape::Variant { cases, .. } => add(
            1,
            cases
                .iter()
                .map(|case| add(4 + case.tag().len(), node(case.payload_type())))
                .max()
                .unwrap_or(0),
        ),
    }
    .min(MAXIMUM_STRUCTURED_CANONICAL_BYTES)
}
fn leaf(kind: &str) -> usize {
    if conduit_core::quantity_info_dimension(kind).is_some() {
        return conduit_core::QUANTITY_ENCODED_LEN;
    }
    let Some(kind) = primitive_info_kind(kind) else {
        return MAXIMUM_STRUCTURED_LEAF_BYTES;
    };
    let fixed = fixed_integer_bytes(kind);
    if fixed != 0 {
        return fixed;
    }
    match kind {
        PrimitiveInfoKind::Empty | PrimitiveInfoKind::CancellationRequest => 0,
        PrimitiveInfoKind::Bool => 1,
        PrimitiveInfoKind::Count => 8,
        PrimitiveInfoKind::Scalar => 16,
        PrimitiveInfoKind::Unit => conduit_core::UNIT_ENCODED_LEN,
        PrimitiveInfoKind::Dimension => conduit_core::DIMENSION_ENCODED_LEN,
        PrimitiveInfoKind::MathematicalValue => conduit_core::MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
        PrimitiveInfoKind::Quantity => conduit_core::QUANTITY_ENCODED_LEN,
        PrimitiveInfoKind::F32 => 4,
        PrimitiveInfoKind::F64 => 8,
        _ => MAXIMUM_STRUCTURED_LEAF_BYTES,
    }
}
fn add(a: usize, b: usize) -> usize {
    a.saturating_add(b).min(MAXIMUM_STRUCTURED_CANONICAL_BYTES)
}

/// Scratch follows the operation's actual byte provenance, not only its erased
/// structural Type. Unbounded v2 programs retain their original allocation law.
pub(super) fn expression(
    value: &crate::PortableExpressionNode,
    input: &StructuredInfoType,
    admitted: Option<u32>,
) -> Result<usize, Refusal> {
    use crate::PortableExpressionOperation as Op;
    let Some(maximum) = admitted else {
        return output(&value.value_type);
    };
    let recurse = |child: &crate::PortableExpressionNode| expression(child, input, admitted);
    let prefix = || {
        value
            .value_type
            .canonical_bytes()
            .map(|bytes| bytes.len())
            .map_err(|_| Refusal::InvalidProgram)
    };
    let sum = |left: usize, right: usize| left.checked_add(right).ok_or(Refusal::InvalidProgram);
    let child = |node: &crate::PortableExpressionNode| -> Result<usize, Refusal> {
        let bound = recurse(node)?;
        // Constructors append raw leaves with tag/length framing. Structured
        // children omit their Type prefix; retaining it is conservative.
        if matches!(node.value_type.shape(), StructuredInfoTypeShape::Leaf(_)) {
            sum(5, bound)
        } else {
            Ok(bound)
        }
    };
    let bound = match &value.operation {
        Op::Input => (maximum as usize).min(output(input)?),
        Op::Projection { value: source, .. } => recurse(source)?.min(output(&value.value_type)?),
        Op::Conditional {
            when_true,
            when_false,
            ..
        } => recurse(when_true)?.max(recurse(when_false)?),
        Op::CanonicalLiteral(bytes) => bytes.len(),
        Op::Constant(constant) => constant
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?
            .len(),
        Op::Literal(literal)
            if matches!(value.value_type.shape(), StructuredInfoTypeShape::Leaf(_)) =>
        {
            crate::expression_evaluate::literal_primitive_bytes(&value.value_type, literal)?.len()
        }
        Op::Record(fields) => {
            let mut bound = sum(prefix()?, 5)?;
            for (name, node) in fields {
                bound = sum(bound, sum(4 + name.len(), child(node)?)?)?;
            }
            bound
        }
        Op::Tuple(values) => {
            let mut bound = sum(prefix()?, 5)?;
            for node in values {
                bound = sum(bound, sum(4 + "item-00000".len(), child(node)?)?)?;
            }
            bound
        }
        Op::Collection(values) => {
            let mut bound = sum(prefix()?, 5)?;
            for node in values {
                bound = sum(bound, child(node)?)?;
            }
            bound
        }
        Op::Variant { tag, payload } => sum(sum(prefix()?, 5 + tag.len())?, child(payload)?)?,
        Op::SemanticCall { kind, arguments } if kind == "sequence/at" => {
            let [source, _] = arguments.as_slice() else {
                return Err(Refusal::InvalidProgram);
            };
            recurse(source)?.min(output(&value.value_type)?)
        }
        Op::SemanticCall { kind, arguments } if kind == "bytes/window" => {
            let [source, _, length] = arguments.as_slice() else {
                return Err(Refusal::InvalidProgram);
            };
            let source = recurse(source)?;
            if let Op::Literal(literal) = &length.operation {
                let encoded = crate::expression_evaluate::literal_primitive_bytes(
                    &length.value_type,
                    literal,
                )?;
                let count = u64::from_le_bytes(
                    encoded
                        .as_slice()
                        .try_into()
                        .map_err(|_| Refusal::InvalidProgram)?,
                );
                source.min(usize::try_from(count).map_err(|_| Refusal::InvalidProgram)?)
            } else {
                source
            }
        }
        // Fixed primitive results stay small; other operations retain the
        // existing conservative Type ceiling until their byte law is reviewed.
        _ => output(&value.value_type)?,
    };
    Ok(bound.min(MAXIMUM_STRUCTURED_CANONICAL_BYTES))
}

/// Finite canonical frame ceiling from an exact checked Type, including leaf framing.
pub fn maximum_prepared_canonical_value_bytes(
    ty: &conduit_core::StructuredInfoType,
) -> Result<u32, Refusal> {
    Ok(canonical(ty)? as u32)
}

pub(crate) fn maximum_input_type_bytes(
    ty: &conduit_core::StructuredInfoType,
) -> Result<u32, Refusal> {
    Ok(output(ty)? as u32)
}
