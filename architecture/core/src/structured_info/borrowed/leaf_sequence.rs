//! Borrowed access to a validated homogeneous collection of primitive leaves.

use super::{malformed, Cursor, StructuredInfoRefusal, ValidatedCanonicalStructuredValue};

impl<'a> ValidatedCanonicalStructuredValue<'a> {
    /// Copy a sequence or fixed collection of one-byte primitive leaves into
    /// caller-owned storage. Both the exact element identity and capacity are
    /// checked before any write. No allocation or provider-specific framing.
    pub fn copy_octet_collection(
        self,
        element_identity: &str,
        destination: &mut [u8],
    ) -> Result<usize, StructuredInfoRefusal> {
        let mut kind = Cursor::new(self.type_bytes);
        match kind.byte()? {
            1 => {
                kind.u16()?;
            }
            4 => {
                kind.u16()?;
                kind.u16()?;
            }
            _ => return Err(malformed()),
        }
        if kind.byte()? != 0 || kind.text()? != element_identity || !kind.remaining.is_empty() {
            return Err(malformed());
        }
        let mut value = Cursor::new(self.value_node);
        if value.byte()? != 1 {
            return Err(malformed());
        }
        let count = value.length()?;
        if count > destination.len() {
            return Err(StructuredInfoRefusal::WrongCollectionLength);
        }
        // Validate every leaf width before modifying the destination. Canonical
        // validation already established membership and primitive validity.
        let mut check = Cursor::new(value.remaining);
        for _ in 0..count {
            if check.byte()? != 0 || check.bytes()?.len() != 1 {
                return Err(malformed());
            }
        }
        if !check.remaining.is_empty() {
            return Err(malformed());
        }
        for target in &mut destination[..count] {
            value.byte()?;
            *target = value.bytes()?[0];
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kind_id, validate_canonical_structured_value, PreparedLeafSequenceEncoder};

    #[test]
    fn borrowed_copy_preserves_destination_on_capacity_identity_and_width_refusal() {
        let mut encoder = PreparedLeafSequenceEncoder::new(kind_id("value/u8"), 1, 3).unwrap();
        let encoded = encoder.encode([&[1][..], &[2], &[3]].into_iter()).unwrap();
        let value = validate_canonical_structured_value(encoded).unwrap();
        let mut destination = [0xa5; 4];
        assert!(value
            .copy_octet_collection("value/u8", &mut destination[..2])
            .is_err());
        assert_eq!(destination, [0xa5; 4]);
        assert!(value
            .copy_octet_collection("value/i8", &mut destination)
            .is_err());
        assert_eq!(destination, [0xa5; 4]);
        assert_eq!(
            value.copy_octet_collection("value/u8", &mut destination),
            Ok(3)
        );
        assert_eq!(destination, [1, 2, 3, 0xa5]);

        let mut wide = PreparedLeafSequenceEncoder::new(kind_id("value/u16"), 2, 1).unwrap();
        let encoded = wide.encode([&[1, 0][..]].into_iter()).unwrap();
        let value = validate_canonical_structured_value(encoded).unwrap();
        assert!(value
            .copy_octet_collection("value/u16", &mut destination)
            .is_err());
        assert_eq!(destination, [1, 2, 3, 0xa5]);
    }
}
