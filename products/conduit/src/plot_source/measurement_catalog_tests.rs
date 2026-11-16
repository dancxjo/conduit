use crate::plot_source;

#[test]
fn ordinary_product_source_expands_the_shared_three_series_chart() {
    let text = plot_source::measurement_chart_fixture_source();
    let canonical = plot_source::parse(&text).unwrap();
    let expanded = canonical.expand_entry_for_authoring().unwrap();
    assert_eq!(expanded.expanded.name, "inspection/three-series");
    assert_eq!(expanded.expanded.gears.len(), 4);
    assert_eq!(expanded.input_bindings.len(), 3);
    assert!(expanded
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == "data/measurement-chart/3"));
    assert!(expanded
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == "face/measurement-chart-flow"));
}
