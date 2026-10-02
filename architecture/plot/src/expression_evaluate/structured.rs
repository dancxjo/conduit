//! Structured projection and canonical-value boundaries for evaluation.

use super::*;

pub(super) fn projection(
    source: Value,
    member: &PortableExpressionProjection,
    expected: &StructuredInfoType,
) -> Result<Value, PortableExpressionEvaluationRefusal> {
    let value = structured_value(source)?;
    let selected = match (value.shape(), member) {
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
    let encoded = match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => bytes.to_vec(),
        _ => value
            .canonical_bytes()
            .map_err(|_| PortableExpressionEvaluationRefusal::InvalidProgram)?,
    };
    Ok(Value {
        value_type: value.value_type().clone(),
        encoded,
    })
}
