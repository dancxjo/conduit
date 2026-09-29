use alloc::{format, string::String, vec, vec::Vec};

use crate::{KindId, StructuredFieldType};

use super::{StructuredInfoRefusal, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES};

/// Canonical anonymous tuple meaning shared by expressions and temporal joins.
///
/// The identity depends only on the ordered exact member types. Source spelling,
/// location, and the Gear which produced the tuple never participate.
pub fn tuple_info_type(
    values: Vec<StructuredInfoType>,
) -> Result<StructuredInfoType, StructuredInfoRefusal> {
    if values.is_empty() {
        return Err(StructuredInfoRefusal::EmptyShape);
    }
    let mut fields = values
        .into_iter()
        .enumerate()
        .map(|(index, value_type)| StructuredFieldType::new(format!("item-{index:05}"), value_type))
        .collect::<Result<Vec<_>, _>>()?;
    fields.sort_by(|left, right| left.name().cmp(right.name()));
    let mut identity = Vec::new();
    for field in &fields {
        push_identity_field(&mut identity, field.name());
        let member = field.value_type().profile()?;
        push_identity_field(&mut identity, member.value_kind().as_str());
    }
    let digest = crate::semantic_digest("conduit.conduitese.anonymous-tuple.v1", &identity);
    StructuredInfoType::record(
        crate::kind_id(&format!(
            "conduitese/anonymous-tuple-{}@1",
            encode_hex(&digest)
        )),
        fields,
    )
}

fn push_identity_field(encoded: &mut Vec<u8>, value: &str) {
    encoded.extend_from_slice(&(value.len() as u64).to_le_bytes());
    encoded.extend_from_slice(value.as_bytes());
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

/// Allocation-prepared encoder for one exact two-member tuple of semantic
/// leaf values. Runtime calls validate and copy into the admitted buffer only.
pub struct PreparedTuplePairEncoder {
    left_kind: KindId,
    right_kind: KindId,
    left_maximum: usize,
    right_maximum: usize,
    prefix: Vec<u8>,
    middle: Vec<u8>,
    output: Vec<u8>,
    maximum_bytes: u32,
}

impl PreparedTuplePairEncoder {
    pub fn new(
        left_kind: KindId,
        left_maximum: u32,
        right_kind: KindId,
        right_maximum: u32,
    ) -> Result<Self, StructuredInfoRefusal> {
        let value_type = tuple_info_type(vec![
            StructuredInfoType::leaf(left_kind.clone())?,
            StructuredInfoType::leaf(right_kind.clone())?,
        ])?;
        let mut prefix = value_type.canonical_bytes()?;
        prefix.push(2); // record value
        prefix.extend_from_slice(&2_u32.to_le_bytes());
        encode_prepared_text("item-00000", &mut prefix);
        prefix.push(0); // leaf value
        let mut middle = Vec::new();
        encode_prepared_text("item-00001", &mut middle);
        middle.push(0); // leaf value
        let left_maximum = usize::try_from(left_maximum)
            .map_err(|_| StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
        let right_maximum = usize::try_from(right_maximum)
            .map_err(|_| StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
        let maximum = prefix
            .len()
            .checked_add(core::mem::size_of::<u32>())
            .and_then(|length| length.checked_add(left_maximum))
            .and_then(|length| length.checked_add(middle.len()))
            .and_then(|length| length.checked_add(core::mem::size_of::<u32>()))
            .and_then(|length| length.checked_add(right_maximum))
            .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
        if maximum > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            left_kind,
            right_kind,
            left_maximum,
            right_maximum,
            prefix,
            middle,
            output: Vec::with_capacity(maximum),
            maximum_bytes: maximum as u32,
        })
    }

    pub fn value_type(&self) -> Result<StructuredInfoType, StructuredInfoRefusal> {
        tuple_info_type(vec![
            StructuredInfoType::leaf(self.left_kind.clone())?,
            StructuredInfoType::leaf(self.right_kind.clone())?,
        ])
    }

    pub fn maximum_bytes(&self) -> u32 {
        self.maximum_bytes
    }

    pub fn encoded(&self) -> &[u8] {
        &self.output
    }

    pub fn encode(&mut self, left: &[u8], right: &[u8]) -> Result<&[u8], StructuredInfoRefusal> {
        if left.len() > self.left_maximum || right.len() > self.right_maximum {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        crate::validate_primitive_info(self.left_kind.as_str(), left)
            .map_err(StructuredInfoRefusal::InvalidPrimitiveLeaf)?;
        crate::validate_primitive_info(self.right_kind.as_str(), right)
            .map_err(StructuredInfoRefusal::InvalidPrimitiveLeaf)?;
        self.output.clear();
        self.output.extend_from_slice(&self.prefix);
        self.output
            .extend_from_slice(&(left.len() as u32).to_le_bytes());
        self.output.extend_from_slice(left);
        self.output.extend_from_slice(&self.middle);
        self.output
            .extend_from_slice(&(right.len() as u32).to_le_bytes());
        self.output.extend_from_slice(right);
        debug_assert!(self.output.len() <= self.maximum_bytes as usize);
        Ok(&self.output)
    }
}

fn encode_prepared_text(value: &str, output: &mut Vec<u8>) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StructuredInfoValue, StructuredInfoValueShape};

    #[test]
    fn prepared_pair_is_the_same_canonical_tuple_used_by_expression_types() {
        let mut encoder = PreparedTuplePairEncoder::new(
            crate::kind_id(crate::TEXT_INFO_ID),
            16,
            crate::kind_id(crate::COUNT_INFO_ID),
            crate::COUNT_ENCODED_LEN as u32,
        )
        .unwrap();
        let expected_type = encoder.value_type().unwrap();
        let maximum_bytes = encoder.maximum_bytes();
        let count = crate::encode_count(7);
        let encoded = encoder.encode(b"hello", &count).unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        assert_eq!(value.value_type(), &expected_type);
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("tuple must remain an exact structured record")
        };
        assert_eq!(fields[0].name(), "item-00000");
        assert_eq!(fields[1].name(), "item-00001");
        assert!(
            matches!(fields[0].value().shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == b"hello")
        );
        assert!(
            matches!(fields[1].value().shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == count)
        );
        assert!(encoded.len() <= maximum_bytes as usize);
    }
}
