use super::*;
const VA: u64 = 0xffff_ffc0_8000_0000;
const PA: u64 = 0x8000_0000;
const SATP: u64 = (8 << 60) | 1;
fn leaf(pa: u64) -> u64 {
    ((pa >> 12) << 10) | 1 | 2 | 4 | 64 | 128
}
#[test]
fn each_page_of_superpage_including_partial_last_page_is_checked() {
    let mut reads = 0;
    let evidence = validate(VA, PA, 4097, SATP, |_| {
        reads += 1;
        Ok(leaf(PA))
    })
    .unwrap();
    assert_eq!(evidence.checked_pages, 2);
    assert_eq!(reads, 2);
}
#[test]
fn three_level_walk_uses_actual_descriptor_addresses() {
    let mut reads = alloc::vec::Vec::new();
    let e = validate(VA, PA, PAGE, SATP, |address| {
        reads.push(address);
        Ok(match reads.len() {
            1 => (2 << 10) | 1,
            2 => (3 << 10) | 1,
            _ => leaf(PA),
        })
    })
    .unwrap();
    assert_eq!(reads, [0x1810, 0x2000, 0x3000]);
    assert_eq!(e.descriptors_read, 3);
}
#[test]
fn writable_user_clear_accessed_dirty_and_supported_attributes_are_required() {
    for (entry, expected) in [
        (leaf(PA) | 16, Refusal::Permission),
        (leaf(PA) & !4, Refusal::Permission),
        (leaf(PA) & !64, Refusal::Permission),
        (leaf(PA) & !128, Refusal::Permission),
        (leaf(PA) | (1 << 61), Refusal::MemoryAttribute),
        (leaf(PA) | (1 << 63), Refusal::MemoryAttribute),
        (leaf(PA + PAGE), Refusal::Table),
        (leaf(0xc000_0000), Refusal::PhysicalMismatch),
    ] {
        assert_eq!(validate(VA, PA, PAGE, SATP, |_| Ok(entry)), Err(expected));
    }
}
#[test]
fn bounds_regime_and_unreadable_tables_refuse_before_writes() {
    for (va, bytes) in [
        (VA + 1, PAGE),
        (VA, 0),
        (VA, MAX_BYTES + 1),
        (1 << 39, PAGE),
    ] {
        assert_eq!(
            validate(va, PA, bytes, SATP, |_| panic!("invalid extent")),
            Err(Refusal::Extent)
        );
    }
    assert_eq!(
        validate(VA, PA, PAGE, 9 << 60, |_| panic!("wrong mode")),
        Err(Refusal::Regime)
    );
    assert_eq!(
        validate(VA, PA, PAGE, SATP, |_| Err(Refusal::Table)),
        Err(Refusal::Table)
    );
}
