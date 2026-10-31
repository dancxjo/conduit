//! Pre-Play preparation followed by allocation-free canonical shape validation.
use super::{
    canonical::Cursor, StructuredInfoRefusal as Refusal, StructuredInfoType,
    StructuredInfoTypeShape as Shape, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    MAXIMUM_STRUCTURED_INFO_NODES, MAXIMUM_STRUCTURED_LEAF_BYTES,
};
use alloc::borrow::Cow;

/// Retains one checked finite schema and its exact canonical prefix.
/// Leaf payload meaning remains owned by its kind; this validates the canonical
/// structured envelope, shape and bounds, not a second leaf-language checker.
pub struct PreparedStructuredValueValidator {
    prefix: Cow<'static, [u8]>,
    maximum_bytes: usize,
}

impl PreparedStructuredValueValidator {
    /// Allocates only during preparation, before Play start.
    pub fn new(value_type: &StructuredInfoType, maximum_bytes: usize) -> Result<Self, Refusal> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let prefix = value_type.canonical_bytes()?;
        if prefix.len() >= maximum_bytes {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            prefix: Cow::Owned(prefix),
            maximum_bytes,
        })
    }

    /// Borrow immutable schema bytes after exact checked-Type comparison.
    /// Static preparation retains no decoded Type tree or owned schema copy.
    pub fn with_static_schema(
        value_type: &StructuredInfoType,
        schema: &'static [u8],
        maximum_bytes: usize,
    ) -> Result<Self, Refusal> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let canonical = value_type.canonical_bytes()?;
        if canonical.len() >= maximum_bytes {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        if canonical != schema {
            return Err(Refusal::WrongType);
        }
        Ok(Self {
            prefix: Cow::Borrowed(schema),
            maximum_bytes,
        })
    }

    /// Borrows input and traverses the already checked schema without allocation.
    pub fn validate(&self, input: &[u8]) -> Result<(), Refusal> {
        if input.len() > self.maximum_bytes {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let node = input
            .strip_prefix(self.prefix.as_ref())
            .ok_or(Refusal::WrongType)?;
        let mut cursor = Cursor::new(node);
        let mut remaining = MAXIMUM_STRUCTURED_INFO_NODES;
        validate_schema_node(&self.prefix, &mut cursor, &mut remaining)?;
        if !cursor.remaining.is_empty() {
            return Err(Refusal::MalformedCanonicalEncoding);
        }
        Ok(())
    }
}

fn expect(actual: bool) -> Result<(), Refusal> {
    actual
        .then_some(())
        .ok_or(Refusal::MalformedCanonicalEncoding)
}

fn validate_node(
    ty: &StructuredInfoType,
    cursor: &mut Cursor<'_>,
    remaining: &mut usize,
    count_nominal: bool,
) -> Result<(), Refusal> {
    if !count_nominal {
        if let Shape::Nominal { representation, .. } = ty.shape() {
            return validate_node(representation, cursor, remaining, false);
        }
    }
    *remaining = remaining.checked_sub(1).ok_or(Refusal::TooManyNodes)?;
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            validate_node(representation, cursor, remaining, count_nominal)?;
        }
        Shape::Leaf(kind) => {
            expect(cursor.byte()? == 0)?;
            let encoded = cursor.bytes()?;
            if encoded.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
                return Err(Refusal::LeafTooLarge);
            }
            crate::validate_primitive_info(kind.as_str(), encoded)
                .map_err(Refusal::InvalidPrimitiveLeaf)?;
        }
        Shape::Collection { element, length } => {
            expect(cursor.byte()? == 1)?;
            expect(cursor.length()? == usize::from(length))?;
            for _ in 0..length {
                validate_node(element, cursor, remaining, count_nominal)?;
            }
        }
        Shape::Sequence {
            element,
            minimum_items,
            maximum_items,
        } => {
            expect(cursor.byte()? == 1)?;
            let length = cursor.length()?;
            expect(length >= usize::from(minimum_items) && length <= usize::from(maximum_items))?;
            for _ in 0..length {
                validate_node(element, cursor, remaining, count_nominal)?;
            }
        }
        Shape::Record { fields, .. } => {
            expect(cursor.byte()? == 2)?;
            expect(cursor.length()? == fields.len())?;
            for field in fields {
                expect(cursor.bytes()? == field.name().as_bytes())?;
                validate_node(field.value_type(), cursor, remaining, count_nominal)?;
            }
        }
        Shape::Variant { cases, .. } => {
            expect(cursor.byte()? == 3)?;
            let tag = cursor.bytes()?;
            let case = cases
                .iter()
                .find(|case| case.tag().as_bytes() == tag)
                .ok_or(Refusal::UnknownVariantTag)?;
            validate_node(case.payload_type(), cursor, remaining, count_nominal)?;
        }
    }
    Ok(())
}

/// Validate configuration custody without constructing a tree of value/type clones.
/// Decode the schema with the established normalizing decoder; configuration
/// acceptance and profile identity therefore retain their existing semantics.
pub(crate) fn configuration_profile(input: &[u8]) -> Result<super::StructuredInfoProfile, Refusal> {
    if input.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(Refusal::CanonicalEncodingTooLarge);
    }
    let (value_type, encoded) = super::decode_type(input)?;
    let mut cursor = Cursor::new(encoded);
    let mut remaining = MAXIMUM_STRUCTURED_INFO_NODES;
    // Owned nominal values retag their representation and add no value node.
    validate_node(&value_type, &mut cursor, &mut remaining, false)?;
    if !cursor.remaining.is_empty() {
        return Err(Refusal::MalformedCanonicalEncoding);
    }
    value_type.profile()
}

// Schemas reaching these walkers were compared byte-for-byte to a checked Type
// during preparation. Cursor traversal retains the owned validator's refusal
// order and node accounting; no schema is decoded or allocated during Play.
fn schema_child<'a>(schema: &mut Cursor<'a>) -> Result<&'a [u8], Refusal> {
    let start = schema.remaining;
    match schema.byte()? {
        0 => {
            schema.bytes()?;
        }
        1 => {
            schema.u16()?;
            schema_child(schema)?;
        }
        4 => {
            schema.u16()?;
            schema.u16()?;
            schema_child(schema)?;
        }
        5 => {
            schema.bytes()?;
            schema_child(schema)?;
        }
        2 | 3 => {
            schema.bytes()?;
            for _ in 0..schema.length()? {
                schema.bytes()?;
                schema_child(schema)?;
            }
        }
        _ => return Err(Refusal::MalformedCanonicalEncoding),
    }
    Ok(&start[..start.len() - schema.remaining.len()])
}
fn validate_schema_node(
    schema: &[u8],
    value: &mut Cursor<'_>,
    remaining: &mut usize,
) -> Result<(), Refusal> {
    *remaining = remaining.checked_sub(1).ok_or(Refusal::TooManyNodes)?;
    let mut ty = Cursor::new(schema);
    match ty.byte()? {
        5 => {
            ty.bytes()?;
            validate_schema_node(ty.remaining, value, remaining)?;
        }
        0 => {
            let kind = ty.text_ref()?;
            expect(value.byte()? == 0)?;
            let encoded = value.bytes()?;
            if encoded.len() > MAXIMUM_STRUCTURED_LEAF_BYTES {
                return Err(Refusal::LeafTooLarge);
            }
            crate::validate_primitive_info(kind, encoded).map_err(Refusal::InvalidPrimitiveLeaf)?;
        }
        tag @ (1 | 4) => {
            let minimum = usize::from(ty.u16()?);
            let maximum = if tag == 1 {
                minimum
            } else {
                usize::from(ty.u16()?)
            };
            expect(value.byte()? == 1)?;
            let count = value.length()?;
            expect(count >= minimum && count <= maximum)?;
            let child = schema_child(&mut ty)?;
            for _ in 0..count {
                validate_schema_node(child, value, remaining)?;
            }
        }
        2 => {
            ty.bytes()?;
            let count = ty.length()?;
            expect(value.byte()? == 2)?;
            expect(value.length()? == count)?;
            for _ in 0..count {
                expect(value.bytes()? == ty.bytes()?)?;
                validate_schema_node(schema_child(&mut ty)?, value, remaining)?;
            }
        }
        3 => {
            ty.bytes()?;
            let count = ty.length()?;
            expect(value.byte()? == 3)?;
            let selected = value.bytes()?;
            let mut child = None;
            for _ in 0..count {
                let tag = ty.bytes()?;
                let candidate = schema_child(&mut ty)?;
                if tag == selected {
                    child = Some(candidate);
                    break;
                }
            }
            validate_schema_node(child.ok_or(Refusal::UnknownVariantTag)?, value, remaining)?;
        }
        _ => return Err(Refusal::MalformedCanonicalEncoding),
    }
    Ok(())
}

#[cfg(test)]
mod schema_parity_tests {
    use super::*;
    use crate::{
        kind_id, InfoBool, StructuredFieldType, StructuredFieldValue, StructuredInfoValue,
        StructuredVariantCase,
    };
    use alloc::{boxed::Box, vec, vec::Vec};
    fn oracle(ty: &StructuredInfoType, maximum: usize, input: &[u8]) -> Result<(), Refusal> {
        if input.len() > maximum {
            return Err(Refusal::CanonicalEncodingTooLarge);
        }
        let prefix = ty.canonical_bytes()?;
        let node = input
            .strip_prefix(prefix.as_slice())
            .ok_or(Refusal::WrongType)?;
        let mut cursor = Cursor::new(node);
        let mut remaining = MAXIMUM_STRUCTURED_INFO_NODES;
        validate_node(ty, &mut cursor, &mut remaining, true)?;
        if !cursor.remaining.is_empty() {
            return Err(Refusal::MalformedCanonicalEncoding);
        }
        Ok(())
    }
    fn parity(value: StructuredInfoValue) {
        let ty = value.value_type().clone();
        let schema = Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice());
        let encoded = value.canonical_bytes().unwrap();
        let maximum = encoded.len() + 8;
        let owned = PreparedStructuredValueValidator::new(&ty, maximum).unwrap();
        let borrowed =
            PreparedStructuredValueValidator::with_static_schema(&ty, schema, maximum).unwrap();
        let mut inputs: Vec<Vec<u8>> = (0..=encoded.len()).map(|n| encoded[..n].to_vec()).collect();
        for i in 0..encoded.len() {
            let mut bytes = encoded.clone();
            bytes[i] ^= 255;
            inputs.push(bytes);
        }
        for n in [1, 8, 9] {
            let mut bytes = encoded.clone();
            bytes.extend(vec![0; n]);
            inputs.push(bytes);
        }
        for input in inputs {
            let expected = oracle(&ty, maximum, &input);
            assert_eq!(owned.validate(&input), expected, "owned bytes {input:?}");
            assert_eq!(
                borrowed.validate(&input),
                expected,
                "static bytes {input:?}"
            );
        }
        assert!(
            PreparedStructuredValueValidator::with_static_schema(&ty, b"wrong", maximum).is_err()
        );
        for limit in [0, schema.len(), MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1] {
            assert_eq!(
                PreparedStructuredValueValidator::new(&ty, limit).err(),
                PreparedStructuredValueValidator::with_static_schema(&ty, schema, limit).err()
            );
        }
    }
    #[test]
    fn canonical_schema_walker_matches_original_type_walker_for_every_shape_and_mutation() {
        let leaf = StructuredInfoType::leaf(kind_id(crate::BOOL_INFO_ID)).unwrap();
        let boolean =
            || StructuredInfoValue::leaf(leaf.clone(), InfoBool::TRUE.encode().to_vec()).unwrap();
        parity(boolean());
        let collection = StructuredInfoType::collection(leaf.clone(), Some(2)).unwrap();
        parity(StructuredInfoValue::collection(collection, vec![boolean(), boolean()]).unwrap());
        let sequence = StructuredInfoType::sequence(leaf.clone(), 3).unwrap();
        parity(StructuredInfoValue::sequence(sequence, vec![boolean()]).unwrap());
        let record = StructuredInfoType::record(
            kind_id("parity/record"),
            vec![StructuredFieldType::new("on", leaf.clone()).unwrap()],
        )
        .unwrap();
        parity(
            StructuredInfoValue::record(
                record,
                vec![StructuredFieldValue::new("on", boolean()).unwrap()],
            )
            .unwrap(),
        );
        let variant = StructuredInfoType::variant(
            kind_id("parity/variant"),
            vec![
                StructuredVariantCase::new("a", leaf.clone()).unwrap(),
                StructuredVariantCase::new("z", leaf.clone()).unwrap(),
            ],
        )
        .unwrap();
        parity(StructuredInfoValue::variant(variant, "z", boolean()).unwrap());
        let nominal = StructuredInfoType::nominal(kind_id("parity/nominal"), leaf.clone()).unwrap();
        parity(StructuredInfoValue::nominal(nominal, boolean()).unwrap());
    }
}
