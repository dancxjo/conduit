use super::*;
use std::string::ToString;
#[path = "../build/definition.rs"]
mod definition;

#[test]
fn layout_reserves_exact_disjoint_journal_and_existing_bootstrap() {
    assert_eq!(BOOTSTRAP.address(), 0x101f_f000);
    assert_eq!(BOOTSTRAP.bytes(), 4096);
    assert_eq!(CREDENTIALS.bytes(), 8192);
    assert_eq!(FIRMWARE.offset(), 0);
    assert_eq!(FIRMWARE.bytes(), CREDENTIALS.offset());
    assert_eq!(
        CREDENTIALS.offset() + CREDENTIALS.bytes(),
        BOOTSTRAP.offset()
    );
    assert_eq!(BOOTSTRAP.offset() + BOOTSTRAP.bytes(), FLASH_BYTES);
    for region in [FIRMWARE, CREDENTIALS, BOOTSTRAP] {
        assert_eq!(region.offset() % ERASE_BYTES, 0);
        assert_eq!(region.bytes() % ERASE_BYTES, 0);
    }
    let layout = definition::Layout::decode(LAYOUT_MODULE).unwrap();
    assert_eq!(layout.linker(), LINKER_MEMORY_X);
    assert!(layout.rust().contains(&FIRMWARE.bytes().to_string()));
}
#[test]
fn reserved_pages_and_overflow_refuse_without_widening_bounds() {
    assert_eq!(
        classify_image_page(FLASH_START, PAGE_BYTES),
        Ok(ImageRegion::Firmware)
    );
    assert_eq!(
        classify_image_page(CREDENTIALS.address() - PAGE_BYTES, PAGE_BYTES),
        Ok(ImageRegion::Firmware)
    );
    assert_eq!(
        classify_image_page(BOOTSTRAP.address(), PAGE_BYTES),
        Ok(ImageRegion::BodyBootstrap)
    );
    for address in [
        0,
        CREDENTIALS.address(),
        BOOTSTRAP.address() - PAGE_BYTES,
        FLASH_START + FLASH_BYTES,
        u32::MAX - 255,
    ] {
        assert_eq!(
            classify_image_page(address, PAGE_BYTES),
            Err(Refusal::ReservedOrOutsideFlash)
        );
    }
    assert_eq!(
        classify_image_page(FLASH_START + 1, PAGE_BYTES),
        Err(Refusal::Alignment)
    );
    assert!(!CREDENTIALS.contains(CREDENTIALS.offset(), 0));
    assert!(!CREDENTIALS.contains(u32::MAX, 2));
    assert!(!CREDENTIALS.contains(CREDENTIALS.offset(), CREDENTIALS.bytes() + 1));
}
#[test]
fn malformed_geometry_never_generates_a_linker_contract() {
    for source in [
        LAYOUT_MODULE.replace("2097152", "4194304"),
        LAYOUT_MODULE.replace("8192", "4096"),
        LAYOUT_MODULE.replace("4096", "4095"),
        LAYOUT_MODULE.replace("256", "512"),
        LAYOUT_MODULE.replace("flash-layout@1", "flash-layout@2"),
        LAYOUT_MODULE.replace("\"flash_bytes\":", "\"unknown\":"),
        LAYOUT_MODULE.replace(
            "\"flash_bytes\": 2097152,",
            "\"flash_bytes\": 2097152, \"flash_bytes\": 2097152,",
        ),
        " ".repeat(1025),
    ] {
        assert!(definition::Layout::decode(&source).is_err());
    }
}

fn block(address: u32, index: u32, count: u32) -> [u8; UF2_BLOCK_BYTES] {
    let mut block = [0; UF2_BLOCK_BYTES];
    for (at, word) in [
        (0, 0x0a32_4655u32),
        (4, 0x9e5d_5157),
        (8, 0x2000),
        (12, address),
        (16, PAGE_BYTES),
        (20, index),
        (24, count),
        (28, 0xe48b_ff56),
        (508, 0x0ab1_6f30),
    ] {
        block[at..at + 4].copy_from_slice(&word.to_le_bytes());
    }
    block
}
#[test]
fn streaming_uf2_admits_only_complete_ordered_firmware_and_bootstrap_pages() {
    let mut validator = Uf2Validator::new(1024).unwrap();
    validator.push(&block(FLASH_START, 0, 2)).unwrap();
    validator.push(&block(BOOTSTRAP.address(), 1, 2)).unwrap();
    validator.finish().unwrap();
    let mut validator = Uf2Validator::new(1024).unwrap();
    validator.push(&block(FLASH_START, 0, 2)).unwrap();
    assert_eq!(validator.finish(), Err(Uf2Refusal::Incomplete));
    for address in [
        CREDENTIALS.address(),
        BOOTSTRAP.address() - PAGE_BYTES,
        FLASH_START + FLASH_BYTES,
    ] {
        assert_eq!(
            Uf2Validator::new(512).unwrap().push(&block(address, 0, 1)),
            Err(Uf2Refusal::Geometry(Refusal::ReservedOrOutsideFlash))
        );
    }
    let mut validator = Uf2Validator::new(1024).unwrap();
    validator.push(&block(FLASH_START, 0, 2)).unwrap();
    assert_eq!(
        validator.push(&block(FLASH_START + ERASE_BYTES, 1, 2)),
        Err(Uf2Refusal::Sparse)
    );
    for (at, value) in [
        (0, 0),
        (8, 0),
        (28, 0),
        (508, 0),
        (20, 1),
        (24, 2),
        (16, 255),
    ] {
        let mut bad = block(FLASH_START, 0, 1);
        bad[at..at + 4].copy_from_slice(&u32::to_le_bytes(value));
        assert!(Uf2Validator::new(512).unwrap().push(&bad).is_err());
    }
    for bytes in [0, 511, u64::from(FLASH_BYTES) * 2 + 512, u64::MAX] {
        assert!(matches!(Uf2Validator::new(bytes), Err(Uf2Refusal::Extent)));
    }
}
