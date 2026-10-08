//! Allocation-free admission accounting for the existing Type decoder.
use super::*;
use core::mem::size_of;

impl StructuredInfoType {
    /// Bounds requested allocations made while decoding this exact tagged Type.
    /// Names, boxed child Types and collection capacities are counted separately;
    /// record/variant sorting scratch is conservatively counted with their buffers.
    /// The existing decoder remains the authority for the returned Type.
    pub fn canonical_decode_storage_bound(encoded: &[u8]) -> Result<usize, StructuredInfoRefusal> {
        if encoded.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let mut cursor = canonical::Cursor::new(encoded);
        let mut remaining = MAXIMUM_STRUCTURED_INFO_NODES;
        let mut bytes = 0;
        node(&mut cursor, 1, &mut remaining, &mut bytes)?;
        if !cursor.remaining.is_empty() {
            return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
        }
        Ok(bytes)
    }
}
fn charge(bytes: &mut usize, additional: usize) -> Result<(), StructuredInfoRefusal> {
    *bytes = bytes
        .checked_add(additional)
        .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
    Ok(())
}
fn text<'a>(
    cursor: &mut canonical::Cursor<'a>,
    bytes: &mut usize,
) -> Result<&'a str, StructuredInfoRefusal> {
    let text = cursor.text_ref()?;
    charge(bytes, text.len())?;
    Ok(text)
}
fn slots<T>(bytes: &mut usize, count: usize) -> Result<(), StructuredInfoRefusal> {
    charge(
        bytes,
        count
            .checked_mul(size_of::<T>())
            .and_then(|bytes| bytes.checked_mul(3))
            .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?,
    )
}
fn node(
    cursor: &mut canonical::Cursor<'_>,
    depth: usize,
    remaining: &mut usize,
    bytes: &mut usize,
) -> Result<(), StructuredInfoRefusal> {
    if depth > MAXIMUM_STRUCTURED_INFO_DEPTH || *remaining == 0 {
        return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
    }
    *remaining -= 1;
    match cursor.byte()? {
        0 => validate_name(text(cursor, bytes)?),
        1 => {
            let length = cursor.u16()?;
            node(cursor, depth + 1, remaining, bytes)?;
            if length == 0 {
                return Err(StructuredInfoRefusal::EmptyShape);
            }
            if usize::from(length) > MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
                return Err(StructuredInfoRefusal::CollectionTooLarge);
            }
            charge(bytes, size_of::<StructuredInfoType>())
        }
        4 => {
            let minimum = cursor.u16()?;
            let maximum = cursor.u16()?;
            node(cursor, depth + 1, remaining, bytes)?;
            if maximum == 0 {
                return Err(StructuredInfoRefusal::EmptyShape);
            }
            if minimum > maximum {
                return Err(StructuredInfoRefusal::InvalidCollectionBounds);
            }
            if usize::from(maximum) > MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
                return Err(StructuredInfoRefusal::CollectionTooLarge);
            }
            charge(bytes, size_of::<StructuredInfoType>())
        }
        5 => {
            let schema = text(cursor, bytes)?;
            node(cursor, depth + 1, remaining, bytes)?;
            validate_name(schema)?;
            charge(bytes, size_of::<StructuredInfoType>())
        }
        tag @ (2 | 3) => {
            let schema = text(cursor, bytes)?;
            let length = cursor.length()?;
            let maximum = if tag == 2 {
                MAXIMUM_STRUCTURED_RECORD_FIELDS
            } else {
                MAXIMUM_STRUCTURED_VARIANT_CASES
            };
            if length > maximum {
                return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
            }
            if tag == 2 {
                slots::<StructuredFieldType>(bytes, length)?;
            } else {
                slots::<StructuredVariantCase>(bytes, length)?;
            }
            let mut names = [None;
                if MAXIMUM_STRUCTURED_RECORD_FIELDS > MAXIMUM_STRUCTURED_VARIANT_CASES {
                    MAXIMUM_STRUCTURED_RECORD_FIELDS
                } else {
                    MAXIMUM_STRUCTURED_VARIANT_CASES
                }];
            for name in names.iter_mut().take(length) {
                let field = text(cursor, bytes)?;
                node(cursor, depth + 1, remaining, bytes)?;
                validate_name(field)?;
                *name = Some(field);
            }
            validate_name(schema)?;
            if length == 0 {
                return Err(StructuredInfoRefusal::EmptyShape);
            }
            for (index, name) in names[..length].iter().enumerate() {
                if names[..index].contains(name) {
                    return Err(StructuredInfoRefusal::DuplicateName);
                }
            }
            Ok(())
        }
        _ => Err(StructuredInfoRefusal::MalformedCanonicalEncoding),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn same_refusal(encoded: &[u8]) {
        assert_eq!(
            StructuredInfoType::canonical_decode_storage_bound(encoded).unwrap_err(),
            StructuredInfoType::from_canonical_bytes(encoded).unwrap_err(),
        );
    }
    #[test]
    fn malformed_type_preflight_preserves_decoder_first_refusal() {
        same_refusal(&[0, 0, 0, 0, 0]); // empty leaf identity
        same_refusal(&[0, 1, 0, 0, 0, 255]); // malformed identity text
        same_refusal(&[2, 1, 0, 0, 0, b'r', 65, 0, 0, 0]);
        // The child's malformed text refuses before the empty member/schema.
        same_refusal(&[2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 255]);
        let leaf = StructuredInfoType::leaf(KindId::from("value/unit")).unwrap();
        let cases = vec![
            StructuredVariantCase::new("a", leaf.clone()).unwrap(),
            StructuredVariantCase::new("b", leaf).unwrap(),
        ];
        let mut duplicate = StructuredInfoType::variant(KindId::from("fixture/duplicate"), cases)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let index = duplicate
            .windows(5)
            .rposition(|window| window == [1, 0, 0, 0, b'b'])
            .unwrap();
        duplicate[index + 4] = b'a';
        same_refusal(&duplicate);
        let mut too_deep = Vec::new();
        for _ in 0..MAXIMUM_STRUCTURED_INFO_DEPTH {
            too_deep.extend_from_slice(&[5, 1, 0, 0, 0, b'n']);
        }
        too_deep.extend_from_slice(&[0, 1, 0, 0, 0, b'l']);
        same_refusal(&too_deep);
    }
}
