use super::*;
use alloc::{vec, vec::Vec};

fn artifact() -> Vec<u8> {
    let mut bytes = vec![0; 0x3000];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    put16(&mut bytes, 16, 2);
    put16(&mut bytes, 18, 62);
    put64(&mut bytes, 24, USER_TEXT_START);
    put64(&mut bytes, 32, 64);
    put16(&mut bytes, 54, 56);
    put16(&mut bytes, 56, 2);
    for (header, offset, address, flags) in [
        (64, 0x1000, USER_TEXT_START, 5),
        (120, 0x2000, USER_TEXT_START + 4096, 4),
    ] {
        put32(&mut bytes, header, 1);
        put32(&mut bytes, header + 4, flags);
        put64(&mut bytes, header + 8, offset);
        put64(&mut bytes, header + 16, address);
        put64(&mut bytes, header + 32, 16);
        put64(&mut bytes, header + 40, 16);
        put64(&mut bytes, header + 48, 4096);
    }
    bytes
}

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn loader_only_admits_independent_immutable_code_and_constants() {
    let bytes = artifact();
    let image = DomainImage::parse(&bytes, 62).unwrap();
    assert_eq!(image.entry, USER_TEXT_START);
    assert!(image.segments[0].executable);
    assert!(!image.segments[1].executable);
    assert_eq!(image.segments[0].mapped_bytes(), 4096);
    assert_eq!(
        DomainImage::parse(&bytes, 183).err(),
        Some(ImageRefusal::WrongTarget)
    );
}

#[test]
fn malformed_extents_writable_segments_aliases_and_foreign_entries_refuse() {
    for (offset, value) in [
        (32, u64::MAX),
        (64 + 8, u64::MAX),
        (64 + 32, u64::MAX),
        (64 + 16, u64::MAX - 4095),
        (120 + 16, USER_TEXT_START),
        (24, USER_TEXT_START + 4096),
        (64 + 40, 4096),
        (120 + 16, 0xffff_ffff_8000_0000),
    ] {
        let mut bytes = artifact();
        put64(&mut bytes, offset, value);
        assert!(DomainImage::parse(&bytes, 62).is_err(), "offset {offset}");
    }
    for flags in [0, 6, 7] {
        let mut bytes = artifact();
        put32(&mut bytes, 64 + 4, flags);
        assert_eq!(
            DomainImage::parse(&bytes, 62).err(),
            Some(ImageRefusal::InvalidMapping)
        );
    }
    for length in [0, 6, 63, 100, 0x1000] {
        assert!(DomainImage::parse(&artifact()[..length], 62).is_err());
    }
}

#[test]
fn ia32_loader_enforces_the_same_immutable_memory_contract() {
    let mut bytes = artifact();
    bytes[4] = 1;
    put16(&mut bytes, 18, 3);
    put32(&mut bytes, 24, IA32_USER_TEXT_START as u32);
    put32(&mut bytes, 28, 52);
    put16(&mut bytes, 42, 32);
    put16(&mut bytes, 44, 2);
    for (header, offset, address, flags) in [
        (52, 0x1000, IA32_USER_TEXT_START as u32, 5),
        (84, 0x2000, IA32_USER_TEXT_START as u32 + 4096, 4),
    ] {
        put32(&mut bytes, header, 1);
        put32(&mut bytes, header + 4, offset);
        put32(&mut bytes, header + 8, address);
        put32(&mut bytes, header + 16, 16);
        put32(&mut bytes, header + 20, 16);
        put32(&mut bytes, header + 24, flags);
        put32(&mut bytes, header + 28, 4096);
    }
    let image = DomainImage::parse(&bytes, 3).unwrap();
    assert_eq!(image.entry, IA32_USER_TEXT_START);
    for (offset, value) in [
        (52 + 24, 7),
        (52 + 8, USER_TEXT_START as u32),
        (84 + 8, IA32_USER_TEXT_START as u32),
        (52 + 4, u32::MAX),
        (52 + 20, 4096),
        (24, IA32_USER_TEXT_START as u32 + 4096),
    ] {
        let mut malformed = bytes.clone();
        put32(&mut malformed, offset, value);
        assert!(DomainImage::parse(&malformed, 3).is_err());
    }
}
