//! Structured projection and canonical-value boundaries for evaluation.

use super::*;

pub(super) fn projection(
    source: Value,
    member: &PortableExpressionProjection,
    expected: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let value = structured_value(source)?;
    let selected = match (value.shape(), member) {
        (
            StructuredInfoValueShape::Variant { tag, payload },
            PortableExpressionProjection::Field(name),
        ) if tag == name => Some(payload),
        (StructuredInfoValueShape::Record(fields), PortableExpressionProjection::Field(name)) => {
            fields
                .iter()
                .find(|field| field.name() == name)
                .map(StructuredFieldValue::value)
        }
        (
            StructuredInfoValueShape::Record(fields),
            PortableExpressionProjection::TupleIndex(index),
        ) => fields
            .get(usize::from(*index))
            .map(StructuredFieldValue::value),
        (
            StructuredInfoValueShape::Collection(values),
            PortableExpressionProjection::TupleIndex(index),
        ) => values.get(usize::from(*index)),
        _ => None,
    }
    .ok_or(PortableExpressionEvaluationRefusal::InvalidProgram)?;
    if selected.value_type() != expected {
        return Err(PortableExpressionEvaluationRefusal::InvalidProgram);
    }
    encoded_structured(selected.clone())
}

pub(super) fn structured_record(
    value_type: StructuredInfoType,
    fields: Vec<StructuredFieldValue>,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    encoded_structured(
        StructuredInfoValue::record(value_type, fields)
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
    )
}

pub(super) fn structured_value(
    value: Value,
) -> Result<StructuredInfoValue, PortableExpressionEvaluationRefusal> {
    match value.value_type.shape() {
        StructuredInfoTypeShape::Leaf(_) => {
            StructuredInfoValue::leaf(value.value_type, value.encoded)
        }
        _ => {
            let structured = StructuredInfoValue::from_canonical_bytes(&value.encoded);
            match structured {
                Ok(structured) if structured.value_type() == &value.value_type => Ok(structured),
                Ok(_) => Err(conduit_core::StructuredInfoRefusal::WrongType),
                Err(refusal) => Err(refusal),
            }
        }
    }
    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
}

pub(super) fn encoded_structured(
    value: StructuredInfoValue,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let encoded = match (value.value_type().shape(), value.shape()) {
        (StructuredInfoTypeShape::Leaf(_), StructuredInfoValueShape::Leaf(bytes)) => bytes.to_vec(),
        _ => value
            .canonical_bytes()
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
    };
    Ok(Value {
        value_type: value.value_type().clone(),
        encoded,
    })
}

pub(super) fn tuple(
    values: &[PortableExpressionNode],
    node: &PortableExpressionNode,
    input: &[u8],
    input_type: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let fields = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            StructuredFieldValue::new(
                alloc::format!("item-{index:05}"),
                structured_value(evaluate_node(value, input, input_type)?)?,
            )
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
        })
        .collect::<Result<Vec<_>, _>>()?;
    structured_record(node.value_type.clone(), fields)
}
pub(super) fn record(
    fields: &[(String, PortableExpressionNode)],
    node: &PortableExpressionNode,
    input: &[u8],
    input_type: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let fields = fields
        .iter()
        .map(|(name, value)| {
            StructuredFieldValue::new(
                name.clone(),
                structured_value(evaluate_node(value, input, input_type)?)?,
            )
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
        })
        .collect::<Result<Vec<_>, _>>()?;
    structured_record(node.value_type.clone(), fields)
}
pub(super) fn collection(
    values: &[PortableExpressionNode],
    node: &PortableExpressionNode,
    input: &[u8],
    input_type: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let values = values
        .iter()
        .map(|value| structured_value(evaluate_node(value, input, input_type)?))
        .collect::<Result<Vec<_>, _>>()?;
    let value = collection_value(&node.value_type, values)?;
    encoded_structured(value)
}
fn collection_value(
    ty: &StructuredInfoType,
    values: Vec<StructuredInfoValue>,
) -> Result<StructuredInfoValue, PortableExpressionEvaluationRefusal> {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            let inner = collection_value(representation, values)?;
            StructuredInfoValue::nominal(ty.clone(), inner)
        }
        StructuredInfoTypeShape::Sequence { .. } => {
            StructuredInfoValue::sequence(ty.clone(), values)
        }
        _ => StructuredInfoValue::collection(ty.clone(), values),
    }
    .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)
}
pub(super) fn variant(
    tag: &str,
    payload: &PortableExpressionNode,
    node: &PortableExpressionNode,
    input: &[u8],
    input_type: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let payload = structured_value(evaluate_node(payload, input, input_type)?)?;
    let value = StructuredInfoValue::variant(node.value_type.clone(), tag, payload)
        .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?;
    encoded_structured(value)
}
