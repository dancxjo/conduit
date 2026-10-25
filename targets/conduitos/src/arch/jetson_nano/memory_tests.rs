use super::*;

fn bank(start: u64, bytes: u64) -> Region {
    Region::new(start, bytes).unwrap()
}

#[test]
fn carveouts_reject_partial_and_enclosing_writes_but_allow_adjacent_ranges() {
    let mut ram = Regions::new();
    ram.push(bank(0x8000_0000, 0x1_0000_0000), true).unwrap();
    let mut reserved = Regions::new();
    reserved.push(bank(0x8300_0000, 0x10000), false).unwrap();
    for (start, bytes) in [
        (0x8300_0000, 0x1000),
        (0x82ff_f000, 0x2000),
        (0x8300_f000, 0x2000),
        (0x82ff_f000, 0x12000),
    ] {
        assert_eq!(
            admit(&ram, &reserved, bank(start, bytes), 4096),
            Err(Refusal::Reserved)
        );
    }
    for start in [0x82ff_f000, 0x8301_0000, 0x1_0000_0000] {
        assert!(admit(&ram, &reserved, bank(start, 4096), 4096).is_ok());
    }
}

#[test]
fn placements_cannot_cross_ram_holes_or_wrap_the_address_space() {
    let mut ram = Regions::new();
    ram.push(bank(0x8000_0000, 0x1000), true).unwrap();
    ram.push(bank(0x8000_2000, 0x1000), true).unwrap();
    assert_eq!(
        admit(&ram, &Regions::new(), bank(0x8000_0000, 0x3000), 4096),
        Err(Refusal::OutsideRam)
    );
    assert_eq!(Region::new(u64::MAX - 4095, 4096), Err(Refusal::Overflow));
    assert_eq!(Region::new(0, 0), Err(Refusal::Empty));
}

#[test]
fn malformed_alignment_and_overlapping_ram_are_refused() {
    let mut ram = Regions::new();
    ram.push(bank(0x8000_0000, 0x10000), true).unwrap();
    assert_eq!(
        ram.push(bank(0x8000_f000, 0x2000), true),
        Err(Refusal::Overlap)
    );
    for alignment in [0, 3, 4096] {
        assert_eq!(
            admit(&ram, &Regions::new(), bank(0x8000_0001, 4096), alignment),
            Err(Refusal::Alignment)
        );
    }
}

#[test]
fn duplicate_reservations_are_safe_and_capacity_is_finite() {
    let mut reserved = Regions::new();
    for _ in 0..MAX_REGIONS {
        reserved.push(bank(0x8300_0000, 4096), false).unwrap();
    }
    assert_eq!(
        reserved.push(bank(0x8400_0000, 4096), false),
        Err(Refusal::Capacity)
    );
    assert_eq!(reserved.iter().count(), MAX_REGIONS);
}
