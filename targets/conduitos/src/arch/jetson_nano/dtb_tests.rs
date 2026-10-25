use super::{
    dtb::{Dtb, Error, Token},
    memory::{Region, Regions, admit},
};

fn put(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn blob(structure: &[u32]) -> Vec<u8> {
    let end = 56 + structure.len() * 4;
    let mut bytes = vec![0; end + 4];
    for (offset, value) in [
        (0, 0xd00dfeed),
        (4, (end + 4) as u32),
        (8, 56),
        (12, end as u32),
        (16, 40),
        (20, 17),
        (24, 16),
        (32, 4),
        (36, (structure.len() * 4) as u32),
    ] {
        put(&mut bytes, offset, value);
    }
    for (i, value) in structure.iter().enumerate() {
        put(&mut bytes, 56 + i * 4, *value);
    }
    bytes[end..].copy_from_slice(b"reg\0");
    bytes
}

#[test]
fn parses_tokens_and_protects_blob_even_without_firmware_reservation() {
    let bytes = blob(&[1, 0, 3, 4, 0, 0x12345678, 2, 9]);
    let dtb = Dtb::parse(&bytes, 0x83000000).unwrap();
    assert_eq!(dtb.total_bytes, bytes.len());
    let mut tokens = dtb.tokens();
    assert_eq!(tokens.next_token().unwrap(), Some(Token::Begin(b"")));
    assert_eq!(
        tokens.next_token().unwrap(),
        Some(Token::Property {
            name: b"reg",
            value: &[0x12, 0x34, 0x56, 0x78]
        })
    );
    assert_eq!(tokens.next_token().unwrap(), Some(Token::End));
    assert_eq!(tokens.next_token().unwrap(), None);
    let mut ram = Regions::new();
    ram.push(Region::new(0x80000000, 0x10000000).unwrap(), true)
        .unwrap();
    assert!(
        admit(
            &ram,
            &dtb.reservations,
            Region::new(0x83000000, 4096).unwrap(),
            4096
        )
        .is_err()
    );
}

#[test]
fn every_truncation_is_refused_without_panicking() {
    let bytes = blob(&[1, 0, 2, 9]);
    for length in 0..bytes.len() {
        assert!(Dtb::parse(&bytes[..length], 0x83000000).is_err());
    }
}

#[test]
fn rejects_invalid_tree_grammar_and_property_bounds() {
    for words in [
        vec![9],
        vec![1, 0, 9],
        vec![1, 0, 2, 2, 9],
        vec![1, 0, 2, 1, 0, 2, 9],
        vec![1, 0, 3, u32::MAX, 0, 2, 9],
        vec![1, 0, 3, 0, 99, 2, 9],
        vec![1, 0, 2, 9, 4],
        vec![1, 0, 42, 2, 9],
        // Property after a child, an invalid ordering even when bounds are valid.
        vec![1, 0, 1, 0x61000000, 2, 3, 0, 0, 2, 9],
    ] {
        assert!(Dtb::parse(&blob(&words), 0x83000000).is_err(), "{words:?}");
    }
}

#[test]
fn rejects_aliasing_sections_unaligned_addresses_and_incompatible_versions() {
    for (offset, value) in [
        (0, 0),
        (4, u32::MAX),
        (8, 40),
        (8, 57),
        (12, 56),
        (16, 41),
        (20, 16),
        (24, 18),
    ] {
        let mut bytes = blob(&[1, 0, 2, 9]);
        put(&mut bytes, offset, value);
        assert!(Dtb::parse(&bytes, 0x83000000).is_err(), "{offset}: {value}");
    }
    assert!(matches!(
        Dtb::parse(&blob(&[1, 0, 2, 9]), 3),
        Err(Error::Alignment)
    ));
}

#[test]
fn refuses_unterminated_reservation_map_and_excessive_nesting() {
    let mut bytes = blob(&[1, 0, 2, 9]);
    bytes[40..48].copy_from_slice(&0x84000000u64.to_be_bytes());
    bytes[48..56].copy_from_slice(&4096u64.to_be_bytes());
    assert!(Dtb::parse(&bytes, 0x83000000).is_err());
    let mut words = vec![1, 0];
    for _ in 0..32 {
        words.extend([1, 0x61000000]);
    }
    words.extend([2; 33]);
    words.push(9);
    assert!(matches!(
        Dtb::parse(&blob(&words), 0x83000000),
        Err(Error::Depth)
    ));
}
