//! Caller-funded canonical composition against a borrowed linked schema.
//! No Type tree, retained span table, nested output or allocation is constructed.
use super::*;

/// Children of records follow the schema's canonical field order. Leaf bytes
/// retain the schema's exact type; variants select an existing exact case.
#[derive(Clone, Copy)]
pub enum BorrowedStructuredValue<'a> {
    Empty,
    Canonical(ValidatedCanonicalStructuredValue<'a>),
    Leaf(&'a [u8]),
    Record(&'a [BorrowedStructuredValue<'a>]),
    Collection(&'a [BorrowedStructuredValue<'a>]),
    Variant {
        tag: &'a str,
        payload: &'a BorrowedStructuredValue<'a>,
    },
}
/// Finite leaf/record/variant/fixed-collection composition, including nominal wrappers.
/// Sequence construction is intentionally unsupported by this small API.
/// Schema storage and all output belong to the caller; no authority is implied.
pub struct PreparedLinkedStructuredComposer<'schema> {
    validator: BorrowedStructuredValueValidator<'schema>,
}
impl<'schema> PreparedLinkedStructuredComposer<'schema> {
    pub fn new(schema: &'schema [u8], maximum_bytes: usize) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: BorrowedStructuredValueValidator::new(schema, maximum_bytes)?,
        })
    }
    /// All shape, primitive, aggregate and extent refusals precede any write.
    /// The same immutable tree/schema is visited twice; transient stack depth
    /// remains bounded by the existing canonical structured depth/node limits.
    pub fn encode_into<'out>(
        &self,
        value: BorrowedStructuredValue<'_>,
        output: &'out mut [u8],
    ) -> Result<&'out [u8], StructuredInfoRefusal> {
        let schema = self.validator.type_bytes();
        let mut count = Sink {
            output: None,
            position: 0,
        };
        let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        visit(schema, value, 1, &mut nodes, &mut count)?;
        let total = schema
            .len()
            .checked_add(count.position)
            .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
        if total > self.validator.maximum_bytes() || total > output.len() {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let mut writer = Sink {
            output: Some(&mut output[..total]),
            position: 0,
        };
        writer.put(schema)?;
        let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        // Preflight and writing deliberately share one checked traversal. Safe
        // Rust cannot mutate these borrowed schema/value bytes between passes.
        visit(schema, value, 1, &mut nodes, &mut writer)?;
        Ok(&output[..total])
    }
}
struct Sink<'a> {
    output: Option<&'a mut [u8]>,
    position: usize,
}
impl Sink<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), StructuredInfoRefusal> {
        let end = self
            .position
            .checked_add(bytes.len())
            .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
        if end > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        if let Some(output) = self.output.as_deref_mut() {
            output
                .get_mut(self.position..end)
                .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?
                .copy_from_slice(bytes);
        }
        self.position = end;
        Ok(())
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), StructuredInfoRefusal> {
        self.put(&(bytes.len() as u32).to_le_bytes())?;
        self.put(bytes)
    }
}
fn child<'a>(cursor: &mut Cursor<'a>) -> Result<&'a [u8], StructuredInfoRefusal> {
    let mut scratch = MAXIMUM_STRUCTURED_INFO_NODES;
    let (child, rest) = split_type(cursor.remaining, 1, &mut scratch)?;
    cursor.remaining = rest;
    Ok(child)
}
fn visit(
    schema: &[u8],
    value: BorrowedStructuredValue<'_>,
    depth: usize,
    nodes: &mut usize,
    output: &mut Sink<'_>,
) -> Result<(), StructuredInfoRefusal> {
    use StructuredInfoRefusal as R;
    if let BorrowedStructuredValue::Canonical(value) = value {
        if value.type_bytes() != schema {
            return Err(R::WrongType);
        }
        // Recheck in the enclosing traversal's depth and node budget. A child
        // validated alone must not bypass aggregate canonical limits.
        let mut cursor = Cursor::new(value.value_node());
        validate_value(schema, &mut cursor, depth, nodes)?;
        if !cursor.remaining.is_empty() {
            return Err(malformed());
        }
        return output.put(value.value_node());
    }

    if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
        return Err(R::TooDeep);
    }
    if *nodes == 0 {
        return Err(R::TooManyNodes);
    }
    *nodes -= 1;
    let mut kind = Cursor::new(schema);
    match kind.byte()? {
        5 => {
            kind.text()?;
            visit(child(&mut kind)?, value, depth + 1, nodes, output)?;
        }
        0 => {
            let identity = kind.text()?;
            let bytes = match value {
                BorrowedStructuredValue::Leaf(bytes) => bytes,
                BorrowedStructuredValue::Empty if identity == "value/empty" => &[],
                _ => return Err(R::WrongType),
            };
            if bytes.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
                return Err(R::LeafTooLarge);
            }
            crate::validate_primitive_info(identity, bytes).map_err(R::InvalidPrimitiveLeaf)?;
            output.put(&[0])?;
            output.bytes(bytes)?;
        }
        1 => {
            let count = usize::from(kind.u16()?);
            let schema = child(&mut kind)?;
            let BorrowedStructuredValue::Collection(values) = value else {
                return Err(R::WrongType);
            };
            if values.len() != count {
                return Err(R::WrongCollectionLength);
            }
            output.put(&[1])?;
            output.put(&(count as u32).to_le_bytes())?;
            for value in values {
                visit(schema, *value, depth + 1, nodes, output)?;
            }
        }
        2 => {
            kind.text()?;
            let count = kind.length()?;
            let BorrowedStructuredValue::Record(values) = value else {
                return Err(R::WrongType);
            };
            if values.len() != count {
                return Err(R::WrongRecordFields);
            }
            output.put(&[2])?;
            output.put(&(count as u32).to_le_bytes())?;
            for value in values {
                let name = kind.text()?;
                let schema = child(&mut kind)?;
                output.bytes(name.as_bytes())?;
                visit(schema, *value, depth + 1, nodes, output)?;
            }
        }
        3 => {
            kind.text()?;
            let count = kind.length()?;
            let BorrowedStructuredValue::Variant { tag, payload } = value else {
                return Err(R::WrongType);
            };
            let mut selected = None;
            for _ in 0..count {
                let name = kind.text()?;
                let schema = child(&mut kind)?;
                if name == tag {
                    selected = Some(schema);
                }
            }
            let selected = selected.ok_or(R::UnknownVariantTag)?;
            output.put(&[3])?;
            output.bytes(tag.as_bytes())?;
            visit(selected, *payload, depth + 1, nodes, output)?;
        }
        _ => return Err(R::WrongType),
    }
    if !kind.remaining.is_empty() {
        return Err(malformed());
    }
    Ok(())
}
#[cfg(test)]
mod tests;
