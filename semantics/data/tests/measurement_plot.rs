use conduit_core::{Quantity, QuantityUnit, TemporalInstant, TemporalScale};
use conduit_data::*;
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

fn window(count: usize) -> BoundedMeasurementWindow {
    let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
        capacity: 8,
        unit: QuantityUnit::Millivolt,
        range: MeasurementRange {
            minimum: Quantity::new(-100, QuantityUnit::Millivolt),
            maximum: Quantity::new(100, QuantityUnit::Millivolt),
        },
        clock_basis: "fixture-clock".into(),
        full_policy: FullWindowPolicy::Reject,
    })
    .unwrap();
    for index in 0..count {
        window
            .push(MeasurementSample {
                value: Quantity::new(-100 + index as i64 * 25, QuantityUnit::Millivolt),
                observed_at: TemporalInstant {
                    ticks: index as u64 + 1,
                    scale: TemporalScale::Milliseconds,
                    clock_basis: "fixture-clock".into(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                },
                uncertainty: None,
            })
            .unwrap();
    }
    window
}

#[test]
fn bounded_projection_retains_endpoints_and_reports_omissions() {
    let series = MeasurementPlotSeries::project(
        &window(8),
        MeasurementPlotProfile {
            point_capacity: 4,
            overflow_policy: MeasurementPlotOverflowPolicy::EvenlySpaced,
        },
    )
    .unwrap();
    assert_eq!(series.source_samples(), 8);
    assert_eq!(series.omitted_samples(), 4);
    assert_eq!(series.points().len(), 4);
    assert_eq!(series.points()[0].source_index, 0);
    assert_eq!(series.points()[3].source_index, 7);
    assert_eq!(series.points()[0].time_millionths, 0);
    assert_eq!(series.points()[3].time_millionths, PLOT_AXIS_MILLIONTHS);
    assert_eq!(series.points()[0].value_millionths, 0);
    assert_eq!(series.points()[3].value_millionths, 875_000);
}

#[test]
fn projection_pressure_and_invalid_profiles_refuse_distinctly() {
    assert_eq!(
        MeasurementPlotSeries::project(
            &window(8),
            MeasurementPlotProfile {
                point_capacity: 4,
                overflow_policy: MeasurementPlotOverflowPolicy::Reject,
            },
        ),
        Err(MeasurementPlotRefusal::Full)
    );
    assert_eq!(
        MeasurementPlotSeries::project(
            &window(1),
            MeasurementPlotProfile {
                point_capacity: 0,
                overflow_policy: MeasurementPlotOverflowPolicy::EvenlySpaced,
            },
        ),
        Err(MeasurementPlotRefusal::InvalidPointCapacity)
    );
}

#[test]
fn reusable_plot_projection_has_an_exact_checked_form_contract() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_measurement_window_catalog(&mut startup, &mut profile).unwrap();
    install_measurement_plot_catalog(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../forms/measurement-plot/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "measurement-plot", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(
        authored.expanded.gears[0].kind_id.as_str(),
        MEASUREMENT_PLOT_KIND
    );
}
