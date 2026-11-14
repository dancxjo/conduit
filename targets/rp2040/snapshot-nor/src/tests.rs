use super::*;

#[test]
fn only_aligned_durable_subslots_can_be_selected() {
    let first = Slot::new(0, 8192).unwrap();
    let next = Slot::new(8192, 8192).unwrap();
    assert_eq!(first.physical_offset(), DURABLE.offset());
    assert_eq!(
        next.physical_offset(),
        first.physical_offset() + first.bytes()
    );
    assert!(Slot::new(0, DURABLE.bytes()).is_ok());
    for (offset, bytes) in [
        (1, 8192),
        (0, 0),
        (0, 8191),
        (DURABLE.bytes(), 256),
        (u32::MAX - 255, 256),
        (256, DURABLE.bytes()),
    ] {
        assert_eq!(Slot::new(offset, bytes), Err(NorRefusal::Geometry));
    }
}

#[test]
fn every_operation_is_bounded_to_its_original_slot() {
    let slot = Slot::new(0, 8192).unwrap();
    assert_eq!(slot.at(8191, 1), Ok(slot.physical_offset() + 8191));
    assert!(slot.at(8192, 1).is_err());
    assert!(slot.at(u32::MAX, 2).is_err());
    assert!(slot.program_at(0, 256).is_ok());
    assert!(slot.program_at(8192 - 256, 256).is_ok());
    for (offset, length) in [(1, 256), (0, 255), (0, 257), (8192, 256)] {
        assert!(slot.program_at(offset, length).is_err());
    }
}
