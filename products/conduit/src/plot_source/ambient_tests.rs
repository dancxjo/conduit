//! The public product compiler uses the same installed ambient contracts.
use super::*;

#[test]
fn ambient_clock_home_uses_the_ordinary_product_source_catalog() {
    let source = parse(include_str!("../../../../plots/ambient-home/clock.conduit")).unwrap();
    let expanded = source.expand_entry_for_authoring().unwrap();
    assert_eq!(expanded.expanded.name, "ambient-clock-home");
    assert_eq!(expanded.expanded.gears.len(), 3);
    assert_eq!(expanded.expanded.connections.len(), 2);
}
