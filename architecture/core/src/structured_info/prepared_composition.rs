//! Prepared canonical composition of exact leaf, record and variant values.

use alloc::{string::String, vec::Vec};

use super::{
    validate_canonical_structured_value, StructuredInfoRefusal as Refusal, StructuredInfoType,
    StructuredInfoTypeShape as Shape, ValidatedCanonicalStructuredValue as Value,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

struct Field {
    name: String,
    type_bytes: Vec<u8>,
}

/// Canonical composition prepares schema bytes and complete output storage
/// before Play. Runtime calls accept borrowed, already validated child values;
/// they cannot change a field's exact Type or grow the output envelope.
pub struct PreparedStructuredComposer {
    prefix: Vec<u8>,
    shape: CompositionShape,
    output: Vec<u8>,
    maximum_bytes: usize,
}

enum CompositionShape {
    Leaf(String),
    Record(Vec<Field>),
    Variant(Vec<Field>),
}

impl PreparedStructuredComposer {
    pub fn new(value_type: &StructuredInfoType, maximum_bytes: usize) -> Result<Self, Refusal> {
        let prefix = value_type.canonical_bytes()?;
        if maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES || maximum_bytes <= prefix.len() {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let shape = match value_type.shape() {
            Shape::Leaf(kind) => CompositionShape::Leaf(kind.as_str().into()),
            Shape::Record { fields, .. } => CompositionShape::Record(
                fields
                    .iter()
                    .map(|field| {
                        Ok(Field {
                            name: field.name().into(),
                            type_bytes: field.value_type().canonical_bytes()?,
                        })
                    })
                    .collect::<Result<_, Refusal>>()?,
            ),
            Shape::Variant { cases, .. } => CompositionShape::Variant(
                cases
                    .iter()
                    .map(|case| {
                        Ok(Field {
                            name: case.tag().into(),
                            type_bytes: case.payload_type().canonical_bytes()?,
                        })
                    })
                    .collect::<Result<_, Refusal>>()?,
            ),
            _ => return Err(Refusal::WrongType),
        };
        Ok(Self {
            prefix,
            shape,
            output: Vec::with_capacity(maximum_bytes),
            maximum_bytes,
        })
    }

    /// Last prepared result, borrowed without changing storage.
    pub fn encoded(&self) -> &[u8] {
        &self.output
    }

    pub fn leaf(&mut self, bytes: &[u8]) -> Result<&[u8], Refusal> {
        let CompositionShape::Leaf(identity) = &self.shape else {
            return Err(Refusal::WrongType);
        };
        self.admit_size(5 + bytes.len())?;
        crate::validate_primitive_info(identity, bytes).map_err(Refusal::InvalidPrimitiveLeaf)?;
        self.output.clear();
        self.output.extend_from_slice(&self.prefix);
        self.output.push(0);
        push_bytes(bytes, &mut self.output);
        self.finish()
    }

    /// Fields are supplied in canonical schema order, retaining exact child
    /// identity. Refusal checks happen before modifying the previous output.
    pub fn record(&mut self, values: &[Value<'_>]) -> Result<&[u8], Refusal> {
        let CompositionShape::Record(fields) = &self.shape else {
            return Err(Refusal::WrongType);
        };
        if fields.len() != values.len() {
            return Err(Refusal::WrongRecordFields);
        }
        let mut size = 5_usize;
        for (field, value) in fields.iter().zip(values) {
            if field.type_bytes != value.type_bytes() {
                return Err(Refusal::WrongType);
            }
            size = size
                .checked_add(4 + field.name.len())
                .and_then(|size| size.checked_add(value.value_node().len()))
                .ok_or(Refusal::CanonicalEncodingTooLarge)?;
        }
        self.admit_size(size)?;
        self.output.clear();
        self.output.extend_from_slice(&self.prefix);
        self.output.push(2);
        self.output
            .extend_from_slice(&(values.len() as u32).to_le_bytes());
        for (field, value) in fields.iter().zip(values) {
            push_bytes(field.name.as_bytes(), &mut self.output);
            self.output.extend_from_slice(value.value_node());
        }
        self.finish()
    }

    pub fn variant(&mut self, tag: &str, payload: Value<'_>) -> Result<&[u8], Refusal> {
        let CompositionShape::Variant(cases) = &self.shape else {
            return Err(Refusal::WrongType);
        };
        let case = cases
            .iter()
            .find(|case| case.name == tag)
            .ok_or(Refusal::UnknownVariantTag)?;
        if case.type_bytes != payload.type_bytes() {
            return Err(Refusal::WrongType);
        }
        let size = 5_usize
            .checked_add(tag.len())
            .and_then(|size| size.checked_add(payload.value_node().len()))
            .ok_or(Refusal::CanonicalEncodingTooLarge)?;
        self.admit_size(size)?;
        self.output.clear();
        self.output.extend_from_slice(&self.prefix);
        self.output.push(3);
        push_bytes(tag.as_bytes(), &mut self.output);
        self.output.extend_from_slice(payload.value_node());
        self.finish()
    }

    fn admit_size(&self, value_bytes: usize) -> Result<(), Refusal> {
        if self
            .prefix
            .len()
            .checked_add(value_bytes)
            .is_none_or(|size| size > self.maximum_bytes)
        {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        Ok(())
    }

    fn finish(&self) -> Result<&[u8], Refusal> {
        // Composition must also obey aggregate depth/node limits, rather than
        // inferring them from independently valid children.
        validate_canonical_structured_value(&self.output)?;
        Ok(&self.output)
    }
}

fn push_bytes(bytes: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests;
