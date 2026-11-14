//! Allocation diagnostics belong to the actual allocator-owning product roots.
use crate::{allocation_probe, plot_source};

#[test]
fn source_preparation_reports_current_thread_requested_backing() {
    let probe = allocation_probe::begin();
    let source = plot_source::parse(include_str!("../../../plots/clock/main.conduit")).unwrap();
    let checked = source.expand_entry_for_authoring().unwrap();
    let statistics = probe.finish_statistics();
    assert!(statistics.allocations > 0);
    assert!(statistics.peak_live_bytes >= statistics.live_bytes);
    println!("Source current-thread allocation diagnostic: {statistics:?}");
    let warm_probe = allocation_probe::begin();
    let second = plot_source::parse(include_str!("../../../plots/clock/main.conduit")).unwrap();
    let second_checked = second.expand_entry_for_authoring().unwrap();
    let warm = warm_probe.finish_statistics();
    println!("Source warm-catalog current-thread allocation diagnostic: {warm:?}");
    let copy_probe = allocation_probe::begin();
    let catalog_copy = plot_source::standard_catalogs().unwrap();
    let copied = copy_probe.finish_statistics();
    println!("Explicit immutable catalog copy allocation diagnostic: {copied:?}");
    std::hint::black_box((&source, &checked, &second, &second_checked, &catalog_copy));
}
