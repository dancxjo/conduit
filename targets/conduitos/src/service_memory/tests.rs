use super::*;
use alloc::{vec, vec::Vec};

#[allow(dead_code)]
#[path = "../../domain/layout.rs"]
mod ordinary_layout;

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn artifact() -> Vec<u8> {
    let mut bytes = vec![0; 0x3000];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    put16(&mut bytes, 16, 2);
    put16(&mut bytes, 18, 62);
    put64(&mut bytes, 24, 0x0040_0000);
    put64(&mut bytes, 32, 64);
    put16(&mut bytes, 54, 56);
    put16(&mut bytes, 56, 2);
    for (header, offset, address, flags) in
        [(64, 0x1000, 0x0040_0000, 5), (120, 0x2000, 0x0044_0000, 4)]
    {
        put32(&mut bytes, header, 1);
        put32(&mut bytes, header + 4, flags);
        put64(&mut bytes, header + 8, offset);
        put64(&mut bytes, header + 16, address);
        put64(&mut bytes, header + 32, 16);
        put64(&mut bytes, header + 40, 16);
        put64(&mut bytes, header + 48, PAGE_BYTES);
    }
    bytes
}

#[test]
fn compositor_admits_larger_immutable_image_without_widening_text_contract() {
    let bytes = artifact();
    let admitted = ServiceImage::compositor(&bytes).unwrap();
    assert_eq!(admitted.image().segments[1].address, 0x0044_0000);
    assert!(DomainImage::parse(&bytes, 62).is_err());
    assert_eq!(crate::domain_image::MAXIMUM_IMAGE_BYTES, 128 * 1024);
    assert_eq!(ordinary_layout::RETAINED_BYTES, 16 * 1024);
    assert_eq!(
        admitted
            .layout()
            .pixels()
            .expect("compositor pixel range")
            .bytes(),
        8 * 1024 * 1024
    );
    assert_eq!(
        admitted
            .layout()
            .scratch()
            .expect("compositor scratch range")
            .bytes(),
        8 * 1024 * 1024
    );
    assert_eq!(
        admitted.layout().mapped_bytes(),
        2 * 1024 * 1024 + 4096 + 64 * 1024 + 64 * 1024 + 256 * 1024 + 16 * 1024 * 1024
    );
    for pair in admitted.layout().ranges.windows(2) {
        assert_eq!(pair[1].start() - pair[0].end(), PAGE_BYTES);
    }
}

#[test]
fn refuses_writable_overlapping_out_of_window_and_malformed_images() {
    for (offset, value) in [
        (120 + 16, 0x0040_0000),
        (
            120 + 16,
            ServiceMemoryLayout::compositor().exchange().start(),
        ),
        (64 + 32, 2 * 1024 * 1024 + 1),
        (120 + 16, u64::MAX - 4095),
        (32, u64::MAX),
        (64 + 8, u64::MAX),
        (64 + 40, 4096),
        (24, 0x0044_0000),
    ] {
        let mut bytes = artifact();
        put64(&mut bytes, offset, value);
        assert!(ServiceImage::compositor(&bytes).is_err(), "offset={offset}");
    }
    for header in [64, 120] {
        for flags in [0, 2, 6, 7] {
            let mut bytes = artifact();
            put32(&mut bytes, header + 4, flags);
            assert!(ServiceImage::compositor(&bytes).is_err());
        }
    }
    for length in [0, 6, 63, 119, 4096] {
        assert!(ServiceImage::compositor(&artifact()[..length]).is_err());
    }
    let mut bytes = artifact();
    put16(&mut bytes, 18, 183);
    assert_eq!(
        ServiceImage::compositor(&bytes).err(),
        Some(ImageRefusal::WrongTarget)
    );
}

#[test]
fn layout_refuses_overlap_missing_guards_overflow_and_unaligned_ranges() {
    let original = ServiceMemoryLayout::compositor();
    assert!(original.validate().is_ok());
    for (index, start, bytes) in [
        (1, original.image().end(), PAGE_BYTES),
        (1, original.image().start(), PAGE_BYTES),
        (2, original.output().start() + 1, PAGE_BYTES),
        (3, original.stack().start(), 0),
        (4, original.metadata().start(), u64::MAX),
        (5, u64::MAX - 4095, PAGE_BYTES),
        (5, 0x0000_8000_0000_0000 - PAGE_BYTES, PAGE_BYTES),
    ] {
        let mut layout = original;
        layout.ranges[index] = MemoryRange { start, bytes };
        assert_eq!(layout.validate(), Err(ImageRefusal::InvalidMapping));
    }
}

#[test]
fn draft_has_finite_distinct_admission_and_no_raster_arenas() {
    let layout = ServiceProfile::Draft.layout();
    assert!(layout.validate().is_ok());
    assert_eq!(layout.active_ranges().len(), 5);
    assert_eq!(layout.mapped_bytes(), 3_231_744);
    assert_eq!(layout.pixels(), None);
    assert_eq!(layout.scratch(), None);
    assert!(ServiceImage::for_profile(&artifact(), ServiceProfile::Draft).is_ok());
    let mut bytes = artifact();
    put64(&mut bytes, 120 + 16, 0x0042_0000);
    let admitted = ServiceImage::for_profile(&bytes, ServiceProfile::Draft).unwrap();
    assert_eq!(admitted.profile(), ServiceProfile::Draft);
    assert_eq!(admitted.layout(), layout);
    assert!(ServiceImage::compositor(&bytes).is_ok());
    put64(&mut bytes, 120 + 16, layout.image().end());
    assert!(ServiceImage::for_profile(&bytes, ServiceProfile::Draft).is_err());
    put64(&mut bytes, 120 + 16, 0x0042_0000);
    put32(&mut bytes, 120 + 4, 6);
    assert!(ServiceImage::for_profile(&bytes, ServiceProfile::Draft).is_err());
}

#[test]
fn every_draft_range_and_absent_arena_is_validated() {
    let original = ServiceMemoryLayout::draft();
    for index in 0..7 {
        let mut layout = original;
        if index < 5 {
            layout.ranges[index].bytes = 0;
        } else {
            layout.ranges[index] = MemoryRange {
                start: 0x600000,
                bytes: 4096,
            };
        }
        assert_eq!(layout.validate(), Err(ImageRefusal::InvalidMapping));
    }
}
