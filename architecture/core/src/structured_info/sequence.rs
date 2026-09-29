use alloc::vec::Vec;

use crate::{validate_primitive_info, KindId};

use super::{
    StructuredInfoRefusal, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    MAXIMUM_STRUCTURED_COLLECTION_ITEMS,
};

/// Allocation-prepared encoder for one bounded sequence of semantic leaf values.
///
/// The sequence type and its complete output envelope are prepared before Play.
/// Runtime encoding only validates and copies admitted canonical leaf bytes.
pub struct PreparedLeafSequenceEncoder {
    element_kind: KindId,
    element_maximum: usize,
    maximum_items: u16,
    prefix: Vec<u8>,
    output: Vec<u8>,
    maximum_bytes: u32,
}

impl PreparedLeafSequenceEncoder {
    pub fn new(
        element_kind: KindId,
        element_maximum: u32,
        maximum_items: u16,
    ) -> Result<Self, StructuredInfoRefusal> {
        if maximum_items == 0 || usize::from(maximum_items) > MAXIMUM_STRUCTURED_COLLECTION_ITEMS {
            return Err(StructuredInfoRefusal::CollectionTooLarge);
        }
        let value_type = StructuredInfoType::sequence(
            StructuredInfoType::leaf(element_kind.clone())?,
            maximum_items,
        )?;
        let mut prefix = value_type.canonical_bytes()?;
        prefix.push(1); // sequence value
        let element_maximum = usize::try_from(element_maximum)
            .map_err(|_| StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
        let maximum = prefix
            .len()
            .checked_add(core::mem::size_of::<u32>())
            .and_then(|length| {
                element_maximum
                    .checked_add(1 + core::mem::size_of::<u32>())
                    .and_then(|per_item| {
                        per_item
                            .checked_mul(usize::from(maximum_items))
                            .and_then(|items| length.checked_add(items))
                    })
            })
            .ok_or(StructuredInfoRefusal::CanonicalEncodingTooLarge)?;
        if maximum > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            element_kind,
            element_maximum,
            maximum_items,
            prefix,
            output: Vec::with_capacity(maximum),
            maximum_bytes: maximum as u32,
        })
    }

    pub fn value_type(&self) -> Result<StructuredInfoType, StructuredInfoRefusal> {
        StructuredInfoType::sequence(
            StructuredInfoType::leaf(self.element_kind.clone())?,
            self.maximum_items,
        )
    }

    pub const fn maximum_bytes(&self) -> u32 {
        self.maximum_bytes
    }

    pub fn encoded(&self) -> &[u8] {
        &self.output
    }

    pub fn encode<'a>(
        &mut self,
        values: impl ExactSizeIterator<Item = &'a [u8]>,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        if values.len() > usize::from(self.maximum_items) {
            return Err(StructuredInfoRefusal::WrongCollectionLength);
        }
        self.output.clear();
        self.output.extend_from_slice(&self.prefix);
        self.output
            .extend_from_slice(&(values.len() as u32).to_le_bytes());
        for value in values {
            if value.len() > self.element_maximum {
                return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
            }
            validate_primitive_info(self.element_kind.as_str(), value)
                .map_err(StructuredInfoRefusal::InvalidPrimitiveLeaf)?;
            self.output.push(0); // leaf value
            self.output
                .extend_from_slice(&(value.len() as u32).to_le_bytes());
            self.output.extend_from_slice(value);
        }
        debug_assert!(self.output.len() <= self.maximum_bytes as usize);
        Ok(&self.output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StructuredInfoValue, StructuredInfoValueShape, TEXT_INFO_ID};

    #[test]
    fn prepared_sequence_matches_the_canonical_bounded_sequence_type() {
        let mut encoder =
            PreparedLeafSequenceEncoder::new(crate::kind_id(TEXT_INFO_ID), 8, 3).unwrap();
        let expected = encoder.value_type().unwrap();
        let maximum = encoder.maximum_bytes();
        let encoded = encoder
            .encode([b"one".as_slice(), b"two"].into_iter())
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        assert_eq!(value.value_type(), &expected);
        let StructuredInfoValueShape::Collection(values) = value.shape() else {
            panic!("window must remain one exact bounded sequence")
        };
        assert_eq!(values.len(), 2);
        assert!(encoded.len() <= maximum as usize);
    }

    #[test]
    fn prepared_sequence_refuses_excess_items_and_bytes() {
        let mut encoder =
            PreparedLeafSequenceEncoder::new(crate::kind_id(TEXT_INFO_ID), 3, 2).unwrap();
        assert_eq!(
            encoder.encode([b"a".as_slice(), b"b", b"c"].into_iter()),
            Err(StructuredInfoRefusal::WrongCollectionLength)
        );
        assert_eq!(
            encoder.encode([b"long".as_slice()].into_iter()),
            Err(StructuredInfoRefusal::CanonicalEncodingTooLarge)
        );
    }
}
