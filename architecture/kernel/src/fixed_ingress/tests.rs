extern crate std;
use super as ingress;
use crate::{FixedValueStore, ValueStorage};
use std::vec;

#[test]
fn exact_ingress_preserves_reference_identity_payload_and_custody_counts() {
    let mut source = FixedValueStore::<3, 24>::new(72).unwrap();
    let first = source.store(b"retained Source frame").unwrap();
    let second = source.store(b"exact bound anchor").unwrap();
    source.retain(first).unwrap();
    let mut target = FixedValueStore::<3, 24>::new(72).unwrap();
    ingress::transfer(&source, &mut target, &[first, second]).unwrap();
    for reference in [first, second] {
        assert_eq!(source.get(reference), target.get(reference));
        assert_eq!(
            source.reference_count(reference),
            target.reference_count(reference)
        );
    }
    assert_eq!(source.used_bytes(), target.used_bytes());
}
#[test]
fn missing_foreign_reordered_and_capacity_refusals_leave_source_untouched() {
    let mut source = FixedValueStore::<3, 24>::new(72).unwrap();
    let first = source.store(b"first admitted frame").unwrap();
    let second = source.store(b"second admitted frame").unwrap();
    let mut foreign = second;
    foreign.generation += 1;
    for references in [
        vec![first],
        vec![second, first],
        vec![first, first],
        vec![first, foreign],
    ] {
        let mut target = FixedValueStore::<3, 24>::new(72).unwrap();
        assert!(ingress::transfer(&source, &mut target, &references).is_err());
        assert_eq!(target.used_items(), 0);
        assert_eq!(source.get(first).unwrap(), b"first admitted frame");
        assert_eq!(source.get(second).unwrap(), b"second admitted frame");
    }
    let mut limited = FixedValueStore::<3, 24>::new(24).unwrap();
    assert!(ingress::transfer(&source, &mut limited, &[first, second]).is_err());
    assert_eq!(limited.used_items(), 0);
    let mut occupied = FixedValueStore::<3, 24>::new(72).unwrap();
    let existing = occupied.store(b"preserve existing").unwrap();
    assert!(ingress::transfer(&source, &mut occupied, &[first, second]).is_err());
    assert_eq!(occupied.get(existing).unwrap(), b"preserve existing");
}
