//! Prepared canonical composition of finite exact structured values.

use alloc::{borrow::Cow, vec::Vec};

use super::{
    validate_canonical_structured_value, StructuredInfoRefusal as Refusal, StructuredInfoType,
    StructuredInfoTypeShape as Shape, ValidatedCanonicalStructuredValue as Value,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

#[derive(Clone, Copy)]
struct SchemaSpan {
    start: u32,
    length: u32,
}
impl SchemaSpan {
    // These bytes are created from a checked Type during preparation. Sharing
    // any equal occurrence is safe: the private prefix never changes in Play.
    fn prepare(prefix: &[u8], bytes: &[u8]) -> Result<Self, Refusal> {
        let start = if bytes.is_empty() {
            0
        } else {
            prefix
                .windows(bytes.len())
                .position(|part| part == bytes)
                .ok_or(Refusal::WrongType)?
        };
        Ok(Self {
            start: start
                .try_into()
                .map_err(|_| Refusal::CanonicalEncodingTooLarge)?,
            length: bytes
                .len()
                .try_into()
                .map_err(|_| Refusal::CanonicalEncodingTooLarge)?,
        })
    }
    fn bytes(self, prefix: &[u8]) -> &[u8] {
        &prefix[self.start as usize..self.start as usize + self.length as usize]
    }
}
struct Field {
    name: SchemaSpan,
    type_bytes: SchemaSpan,
}

/// Canonical composition prepares schema bytes and complete output storage
/// before Play. Runtime calls accept borrowed, already validated child values;
/// they cannot change a field's exact Type or grow the output envelope.
pub struct PreparedStructuredComposer {
    prefix: Cow<'static, [u8]>,
    shape: CompositionShape,
    output: Vec<u8>,
    maximum_bytes: usize,
}

enum CompositionShape {
    Leaf(SchemaSpan),
    Record(Vec<Field>),
    Variant(Vec<Field>),
    Collection {
        element_type: SchemaSpan,
        minimum_items: u16,
        maximum_items: u16,
    },
}

impl PreparedStructuredComposer {
    pub fn new(value_type: &StructuredInfoType, maximum_bytes: usize) -> Result<Self, Refusal> {
        Self::prepare(
            value_type,
            Cow::Owned(value_type.canonical_bytes()?),
            maximum_bytes,
        )
    }

    /// Borrow an immutable linked schema after comparing it with the complete
    /// exact Type. Only changing output and field-range tables occupy the heap.
    /// This grants no authority and does not validate an execution allocation.
    pub fn with_static_schema(
        value_type: &StructuredInfoType,
        canonical_type: &'static [u8],
        maximum_bytes: usize,
    ) -> Result<Self, Refusal> {
        if value_type.canonical_bytes()? != canonical_type {
            return Err(Refusal::WrongType);
        }
        Self::prepare(value_type, Cow::Borrowed(canonical_type), maximum_bytes)
    }

    fn prepare(
        value_type: &StructuredInfoType,
        prefix: Cow<'static, [u8]>,
        maximum_bytes: usize,
    ) -> Result<Self, Refusal> {
        if maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES || maximum_bytes <= prefix.len() {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let shape = match value_type.shape() {
            Shape::Leaf(kind) => {
                CompositionShape::Leaf(SchemaSpan::prepare(&prefix, kind.as_str().as_bytes())?)
            }
            Shape::Record { fields, .. } => CompositionShape::Record(
                fields
                    .iter()
                    .map(|field| {
                        Ok(Field {
                            name: SchemaSpan::prepare(&prefix, field.name().as_bytes())?,
                            type_bytes: SchemaSpan::prepare(
                                &prefix,
                                &field.value_type().canonical_bytes()?,
                            )?,
                        })
                    })
                    .collect::<Result<_, Refusal>>()?,
            ),
            Shape::Variant { cases, .. } => CompositionShape::Variant(
                cases
                    .iter()
                    .map(|case| {
                        Ok(Field {
                            name: SchemaSpan::prepare(&prefix, case.tag().as_bytes())?,
                            type_bytes: SchemaSpan::prepare(
                                &prefix,
                                &case.payload_type().canonical_bytes()?,
                            )?,
                        })
                    })
                    .collect::<Result<_, Refusal>>()?,
            ),
            Shape::Collection { element, length } => CompositionShape::Collection {
                element_type: SchemaSpan::prepare(&prefix, &element.canonical_bytes()?)?,
                minimum_items: length,
                maximum_items: length,
            },
            Shape::Sequence {
                element,
                minimum_items,
                maximum_items,
            } => CompositionShape::Collection {
                element_type: SchemaSpan::prepare(&prefix, &element.canonical_bytes()?)?,
                minimum_items,
                maximum_items,
            },
            _ => return Err(Refusal::WrongType),
        };
        Ok(Self {
            prefix,
            shape,
            output: Vec::with_capacity(maximum_bytes),
            maximum_bytes,
        })
    }

    /// Last successful result, or empty after an aggregate validation refusal.
    /// Preflight count, identity and extent refusals preserve the previous result.
    /// Bytes reserved by owned heap buffers, excluding this object's inline size
    /// and allocator bookkeeping. Available before Play for resource admission.
    pub fn prepared_heap_bytes(&self) -> usize {
        let shape = match &self.shape {
            CompositionShape::Record(fields) | CompositionShape::Variant(fields) => {
                fields.capacity() * core::mem::size_of::<Field>()
            }
            CompositionShape::Leaf(_) | CompositionShape::Collection { .. } => 0,
        };
        let prefix = match &self.prefix {
            Cow::Owned(bytes) => bytes.capacity(),
            Cow::Borrowed(_) => 0,
        };
        prefix + self.output.capacity() + shape
    }

    pub fn encoded(&self) -> &[u8] {
        &self.output
    }

    pub fn leaf(&mut self, bytes: &[u8]) -> Result<&[u8], Refusal> {
        let CompositionShape::Leaf(identity) = &self.shape else {
            return Err(Refusal::WrongType);
        };
        self.admit_size(5 + bytes.len())?;
        let identity = core::str::from_utf8(identity.bytes(&self.prefix))
            .map_err(|_| Refusal::MalformedCanonicalEncoding)?;
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
            if field.type_bytes.bytes(&self.prefix) != value.type_bytes() {
                return Err(Refusal::WrongType);
            }
            size = size
                .checked_add(4 + field.name.length as usize)
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
            push_bytes(field.name.bytes(&self.prefix), &mut self.output);
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
            .find(|case| case.name.bytes(&self.prefix) == tag.as_bytes())
            .ok_or(Refusal::UnknownVariantTag)?;
        if case.type_bytes.bytes(&self.prefix) != payload.type_bytes() {
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

    /// Compose a finite sequence or exact collection from validated children.
    /// Count, child identity and total extent are checked before replacing the
    /// previous output. Preparation reserves all schema and output storage.
    pub fn collection(&mut self, values: &[Value<'_>]) -> Result<&[u8], Refusal> {
        let CompositionShape::Collection {
            element_type,
            minimum_items,
            maximum_items,
        } = &self.shape
        else {
            return Err(Refusal::WrongType);
        };
        if values.len() < usize::from(*minimum_items) || values.len() > usize::from(*maximum_items)
        {
            return Err(Refusal::WrongCollectionLength);
        }
        let mut size = 5_usize;
        for value in values {
            if element_type.bytes(&self.prefix) != value.type_bytes() {
                return Err(Refusal::WrongType);
            }
            size = size
                .checked_add(value.value_node().len())
                .ok_or(Refusal::CanonicalEncodingTooLarge)?;
        }
        self.admit_size(size)?;
        self.output.clear();
        self.output.extend_from_slice(&self.prefix);
        self.output.push(1);
        self.output
            .extend_from_slice(&(values.len() as u32).to_le_bytes());
        for value in values {
            self.output.extend_from_slice(value.value_node());
        }
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

    fn finish(&mut self) -> Result<&[u8], Refusal> {
        // Composition must also obey aggregate depth/node limits, rather than
        // inferring them from independently valid children.
        if let Err(refusal) = validate_canonical_structured_value(&self.output) {
            self.output.clear();
            return Err(refusal);
        }
        Ok(&self.output)
    }
}

fn push_bytes(bytes: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests;
