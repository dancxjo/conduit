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

#[test]
fn clock_and_weather_are_independent_consumers_of_one_observation() {
    let source = parse(include_str!(
        "../../../../plots/ambient-home/readings.conduit"
    ))
    .unwrap();
    let clock = source
        .expand_named_for_authoring("ambient-clock-display")
        .unwrap();
    let weather = source
        .expand_named_for_authoring("ambient-weather-display")
        .unwrap();
    assert_ne!(
        clock.expanded.checked_plot_id,
        weather.expanded.checked_plot_id
    );

    let root = source.expand_entry_for_authoring().unwrap();
    assert_eq!(root.expanded.name, "ambient-readings");
    let count = |kind: &str| {
        root.expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == kind)
            .count()
    };
    assert_eq!(count("time/every"), 1);
    assert_eq!(count("ambient/resident-observation"), 1);
    assert_eq!(count("face/resident-ambient-wallpaper"), 1);
    assert_eq!(count("face/text"), 3);
    assert_eq!(root.expanded.connections.len(), 8);
}
