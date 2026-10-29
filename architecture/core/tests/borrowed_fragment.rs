//! Complete descriptive encoding preserves the historical fingerprint and limits.
extern crate alloc;
use conduit_core::*;
mod api {
    pub use conduit_core::*;
}
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/fingerprint_refs.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn borrowed_header_and_tail_stream_without_record_allocations() {
    let f = fixture::populated_fragment();
    let connections: Vec<_> = f.connections.iter().map(Into::into).collect();
    let dependencies: Vec<_> = f.startup_dependencies.iter().map(Into::into).collect();
    let order: Vec<_> = f.startup_order.iter().map(|v| v.as_str()).collect();
    let terminals: Vec<_> = f.expected_terminals.iter().map(Into::into).collect();
    let signs: Vec<_> = f.expected_sign.iter().map(Into::into).collect();
    let mut tail = FragmentFingerprintTailRef::from(&f);
    tail.connections = PlannedConnectionRefs::Borrowed(&connections);
    tail.startup_dependencies = StartupDependencyRefs::Borrowed(&dependencies);
    tail.startup_order = PlacementIdRefs::Borrowed(&order);
    tail.expected_terminals = ExpectedTerminalRefs::Borrowed(&terminals);
    tail.expected_sign = ExpectedSignRefs::Borrowed(&signs);
    let (builder, stream) = allocation_probe::observe(|| {
        let mut builder = FragmentFingerprintBuilder::new_ref((&f).into(), f.placements.len());
        for gear in &f.placements {
            builder.push_placement_ref(gear.into()).unwrap();
        }
        builder
    });
    assert_eq!((stream.allocations, stream.reallocations), (0, 0));
    let (id, finish) = allocation_probe::observe(|| builder.finish_ref(tail).unwrap());
    assert_eq!(id, compute_fragment_id(&f));
    // Only the existing public hexadecimal FragmentId result allocates64 bytes.
    // No borrowed tail record, optional Line, state or pool is cloned/materialized.
    assert_eq!(
        (
            finish.allocations,
            finish.reallocations,
            finish.requested_bytes
        ),
        (1, 0, 64)
    );
}
#[test]
fn borrowed_header_tail_preserve_count_and_poison_refusals() {
    let f = fixture::populated_fragment();
    for count in [0, 1, 2] {
        let mut old = FragmentFingerprintBuilder::new((&f).into(), count);
        let mut new = FragmentFingerprintBuilder::new_ref((&f).into(), count);
        for _ in 0..3 {
            assert_eq!(
                old.push_placement(&f.placements[0]),
                new.push_placement_ref((&f.placements[0]).into())
            );
        }
        assert_eq!(old.finish((&f).into()), new.finish_ref((&f).into()));
    }
    let missing = FragmentFingerprintBuilder::new_ref((&f).into(), 1);
    assert_eq!(
        missing.finish_ref((&f).into()),
        Err(FragmentFingerprintRefusal::IncompletePlacements)
    );
    if let Some(overflow) = (u32::MAX as usize).checked_add(1) {
        let mut builder = FragmentFingerprintBuilder::new_ref((&f).into(), overflow);
        assert_eq!(
            builder.push_placement_ref((&f.placements[0]).into()),
            Err(FragmentFingerprintRefusal::PlacementCountOverflow)
        );
        assert_eq!(
            builder.finish_ref((&f).into()),
            Err(FragmentFingerprintRefusal::PlacementCountOverflow)
        );
    }
    let mut empty = f.clone();
    empty.placements.clear();
    let builder = FragmentFingerprintBuilder::new_ref((&empty).into(), 0);
    assert_eq!(
        builder.finish_ref((&empty).into()).unwrap(),
        compute_fragment_id(&empty)
    );
}
