//! Ordinary product Source entrance owns measurement authoring registration.
use super::parse;

#[test]
fn ordinary_source_checks_and_expands_the_typed_measurement_projection() {
    let source = parse(
        r#"
plot projection (
  >> window: MeasurementWindow
  series: MeasurementPlotSeries >>
) {
  project: data/measurement-plot(points = 2, when-full = "evenly-spaced")
  window >> project.window
  project.series >> series
}
"#,
    )
    .unwrap();
    source.check().unwrap();
    let expanded = source.expand_entry_for_authoring().unwrap();
    assert_eq!(expanded.expanded.name, "projection");
    assert_eq!(expanded.expanded.gears.len(), 1);
    assert_eq!(
        expanded.expanded.gears[0].kind_id.as_str(),
        "data/measurement-plot"
    );
    assert_eq!(expanded.input_bindings.len(), 1);
    assert_eq!(expanded.output_bindings.len(), 1);
    assert!(expanded.expanded.activations.is_empty());
}

#[test]
fn ordinary_source_refuses_a_fore_type_that_does_not_match_the_window() {
    let source = parse(
        r#"
plot projection (
  >> window: Text
  series: MeasurementPlotSeries >>
) {
  project: data/measurement-plot(points = 2, when-full = "evenly-spaced")
  window >> project.window
  project.series >> series
}
"#,
    )
    .unwrap();
    assert!(source.syntax.diagnostics.is_empty());
    assert!(source.expand_entry_for_authoring().is_err());
}
