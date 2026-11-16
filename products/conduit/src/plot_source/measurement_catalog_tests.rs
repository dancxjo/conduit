use crate::plot_source;

#[test]
fn ordinary_product_source_expands_the_shared_history_pipeline() {
    let source = plot_source::parse(plot_source::measurement_history_fixture_source()).unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    assert_eq!(plot.expanded.name, "inspection/history");
    assert_eq!(plot.input_bindings.len(), 2);
    assert_eq!(plot.expanded.gears.len(), 5);
    for kind in [
        "data/measurement-count-window",
        "flow/singleton",
        "face/measurement-chart-flow",
    ] {
        assert!(plot
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == kind));
    }
}

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
