//! Physical USB presentation identity, never Host membership or authority.
//! A missing/unprogrammed flash identity has no shared fallback serial.
pub fn serial(unique: [u8; 8]) -> Option<[u8; 16]> {
    if unique == [0; 8] || unique == [0xff; 8] {
        return None;
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut serial = [0; 16];
    for (index, byte) in unique.into_iter().enumerate() {
        serial[index * 2] = HEX[usize::from(byte >> 4)];
        serial[index * 2 + 1] = HEX[usize::from(byte & 15)];
    }
    Some(serial)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_identity_is_lossless_and_independent_of_firmware() {
        assert_eq!(
            serial([0xe6, 0x60, 0, 1, 0x7f, 0x80, 0xfe, 0xff]),
            Some(*b"e66000017f80feff")
        );
        let mut identities = std::collections::BTreeSet::new();
        for byte in 0..=255 {
            assert!(identities.insert(serial([1, 2, 3, 4, 5, 6, 7, byte]).unwrap()));
        }
    }
    #[test]
    fn unavailable_placeholder_is_never_a_shared_serial() {
        assert_eq!(serial([0; 8]), None);
        assert_eq!(serial([0xff; 8]), None);
    }
}
