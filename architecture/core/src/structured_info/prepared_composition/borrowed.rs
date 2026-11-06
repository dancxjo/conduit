//! Fallible preparation directly from an immutable, fully validated schema.
use super::super::{borrowed::split_type, canonical::Cursor, BorrowedStructuredValueValidator};
use super::*;
use core::mem::size_of;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredCompositionPreparationRefusal {
    Canonical(Refusal),
    Capacity,
    Allocation,
}
impl From<Refusal> for StructuredCompositionPreparationRefusal {
    fn from(value: Refusal) -> Self {
        Self::Canonical(value)
    }
}
impl PreparedStructuredComposer {
    /// Validate exact canonical schema bytes without decoding an owned Type.
    /// Fund only actual field-span tables and the complete mutable output;
    /// inline Self and allocator bookkeeping remain separate Host charges.
    pub fn try_from_static_schema(
        schema: &'static [u8],
        maximum_bytes: usize,
        maximum_storage_bytes: usize,
    ) -> Result<Self, StructuredCompositionPreparationRefusal> {
        use StructuredCompositionPreparationRefusal as E;
        BorrowedStructuredValueValidator::new(schema, maximum_bytes)?;
        let mut cursor = Cursor::new(schema);
        let tag = cursor.byte()?;
        let count = match tag {
            0 => 0,
            1 | 4 => 0,
            2 | 3 => {
                cursor.text_ref()?;
                cursor.length()?
            }
            _ => return Err(Refusal::WrongType.into()),
        };
        let required = count
            .checked_mul(size_of::<Field>())
            .and_then(|n| n.checked_add(maximum_bytes))
            .ok_or(E::Capacity)?;
        if required > maximum_storage_bytes {
            return Err(E::Capacity);
        }
        let mut remaining = maximum_storage_bytes;
        let shape = match tag {
            0 => {
                CompositionShape::Leaf(SchemaSpan::prepare(schema, cursor.text_ref()?.as_bytes())?)
            }
            1 | 4 => {
                let minimum_items = cursor.u16()?;
                let maximum_items = if tag == 1 {
                    minimum_items
                } else {
                    cursor.u16()?
                };
                let element = child(&mut cursor)?;
                CompositionShape::Collection {
                    element_type: SchemaSpan::prepare(schema, element)?,
                    minimum_items,
                    maximum_items,
                }
            }
            2 | 3 => {
                let mut fields = Vec::new();
                fields.try_reserve_exact(count).map_err(|_| E::Allocation)?;
                remaining = remaining
                    .checked_sub(
                        fields
                            .capacity()
                            .checked_mul(size_of::<Field>())
                            .ok_or(E::Capacity)?,
                    )
                    .ok_or(E::Capacity)?;
                for _ in 0..count {
                    let name = SchemaSpan::prepare(schema, cursor.text_ref()?.as_bytes())?;
                    let type_bytes = SchemaSpan::prepare(schema, child(&mut cursor)?)?;
                    fields.push(Field { name, type_bytes });
                }
                if tag == 2 {
                    CompositionShape::Record(fields)
                } else {
                    CompositionShape::Variant(fields)
                }
            }
            _ => unreachable!("validated supported shape"),
        };
        if !cursor.remaining.is_empty() {
            return Err(Refusal::MalformedCanonicalEncoding.into());
        }
        if maximum_bytes > remaining {
            return Err(E::Capacity);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(maximum_bytes)
            .map_err(|_| E::Allocation)?;
        if output.capacity() > remaining {
            return Err(E::Capacity);
        }
        Ok(Self {
            prefix: Cow::Borrowed(schema),
            shape,
            output,
            maximum_bytes,
        })
    }
}
fn child<'a>(cursor: &mut Cursor<'a>) -> Result<&'a [u8], Refusal> {
    // The whole schema already passed the shared depth/aggregate-node gate.
    let mut nodes = super::super::MAXIMUM_STRUCTURED_INFO_NODES;
    let (bytes, rest) = split_type(cursor.remaining, 1, &mut nodes)?;
    cursor.remaining = rest;
    Ok(bytes)
}
