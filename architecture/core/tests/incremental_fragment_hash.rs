//! Encoding/order/storage proof; a fingerprint grants no execution authority.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/sealed_state.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn split_encoding_preserves_complete_fragments_and_rejects_incomplete_sequences() {
    let fragment = fixture::fragment();
    let mut builder = FragmentFingerprintBuilder::new((&fragment).into(), 1);
    builder.push_placement(&fragment.placements[0]).unwrap();
    assert_eq!(
        builder.finish((&fragment).into()).unwrap(),
        compute_fragment_id(&fragment)
    );
    let missing = FragmentFingerprintBuilder::new((&fragment).into(), 1);
    assert_eq!(
        missing.finish((&fragment).into()),
        Err(FragmentFingerprintRefusal::IncompletePlacements)
    );
    let mut extra = FragmentFingerprintBuilder::new((&fragment).into(), 0);
    assert_eq!(
        extra.push_placement(&fragment.placements[0]),
        Err(FragmentFingerprintRefusal::UnexpectedPlacement)
    );
    assert_eq!(
        extra.finish((&fragment).into()),
        Err(FragmentFingerprintRefusal::UnexpectedPlacement)
    );
    let mut extra = FragmentFingerprintBuilder::new((&fragment).into(), 1);
    extra.push_placement(&fragment.placements[0]).unwrap();
    assert_eq!(
        extra.push_placement(&fragment.placements[0]),
        Err(FragmentFingerprintRefusal::UnexpectedPlacement)
    );
    assert_eq!(
        extra.finish((&fragment).into()),
        Err(FragmentFingerprintRefusal::UnexpectedPlacement)
    );
}
#[test]
fn placement_count_cannot_wrap_the_canonical_u32_length() {
    let fragment = fixture::fragment();
    if let Some(overflow) = (u32::MAX as usize).checked_add(1) {
        let mut builder = FragmentFingerprintBuilder::new((&fragment).into(), overflow);
        assert_eq!(
            builder.push_placement(&fragment.placements[0]),
            Err(FragmentFingerprintRefusal::PlacementCountOverflow)
        );
        assert_eq!(
            builder.finish((&fragment).into()),
            Err(FragmentFingerprintRefusal::PlacementCountOverflow)
        );
    }
    let empty = FragmentFingerprintBuilder::new((&fragment).into(), 0);
    let mut without_placements = fragment.clone();
    without_placements.placements.clear();
    assert_eq!(
        empty.finish((&fragment).into()).unwrap(),
        compute_fragment_id(&without_placements)
    );
}

fn placement(prototype: &PlannedGear, index: usize) -> PlannedGear {
    let mut gear = prototype.clone();
    gear.placement_id = format!("placement/{index:03}").into();
    gear.gear_id = format!("gear/{index:03}").into();
    gear.configuration = vec![ConfigurationEntry {
        key: "program".into(),
        value: ConfigurationValue::Text(format!("{index:03}\0{}", "é".repeat(4096)).into()),
    }];
    gear
}
#[test]
fn placements_can_be_materialized_and_retired_individually_without_retaining_the_vector() {
    const COUNT: usize = 24;
    let mut metadata = fixture::fragment();
    let prototype = metadata.placements.pop().unwrap();
    metadata.states.clear();
    metadata.startup_order = (0..COUNT)
        .map(|i| format!("placement/{i:03}").into())
        .collect();
    let (complete, all_memory) = allocation_probe::observe(|| {
        let mut complete = metadata.clone();
        complete.placements = (0..COUNT).map(|i| placement(&prototype, i)).collect();
        complete
    });
    let expected = compute_fragment_id(&complete);
    let (actual, streaming_memory) = allocation_probe::observe(|| {
        let mut builder = FragmentFingerprintBuilder::new((&metadata).into(), COUNT);
        for index in 0..COUNT {
            let gear = placement(&prototype, index);
            builder.push_placement(&gear).unwrap();
            // Only the hash state survives this iteration, not Gear/program data.
        }
        builder.finish((&metadata).into()).unwrap()
    });
    assert_eq!(actual, expected);
    assert!(
        streaming_memory.peak_bytes <= 24 * 1024,
        "{streaming_memory:?}"
    );
    assert!(streaming_memory.peak_bytes * 8 < all_memory.peak_bytes);
    eprintln!(
        "incremental_fragment_peak={} retained_placements_peak={}",
        streaming_memory.peak_bytes, all_memory.peak_bytes
    );
    // Order is encoded, never silently sorted or selected by this low layer.
    let mut reversed = FragmentFingerprintBuilder::new((&metadata).into(), COUNT);
    for gear in complete.placements.iter().rev() {
        reversed.push_placement(gear).unwrap();
    }
    assert_ne!(reversed.finish((&metadata).into()).unwrap(), expected);
}

#[test]
fn separately_borrowed_configuration_keeps_the_canonical_placement_identity() {
    let mut fragment = fixture::fragment();
    fragment.placements[0].configuration = vec![
        ConfigurationEntry {
            key: "program".into(),
            value: ConfigurationValue::Text("é\0canonical".into()),
        },
        ConfigurationEntry {
            key: "count".into(),
            value: ConfigurationValue::U64(27),
        },
    ];
    let mut metadata = fragment.placements[0].clone();
    let entries = core::mem::take(&mut metadata.configuration);
    assert!(!entries.is_empty());
    let mut hash = FragmentFingerprintBuilder::new((&fragment).into(), 1);
    hash.push_placement_with_configuration(&metadata, &entries)
        .unwrap();
    assert_eq!(
        hash.finish((&fragment).into()).unwrap(),
        compute_fragment_id(&fragment)
    );
}
