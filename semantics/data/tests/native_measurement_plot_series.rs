use conduit_data::{
    MeasurementPlotOverflowPolicy, MeasurementPlotPoint, MeasurementPlotProfile,
    MeasurementPlotRefusal, MeasurementPlotSeries,
};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn measurement_plot_profile_and_series_round_trip_at_exact_bounds() {
    let profile = MeasurementPlotProfile {
        point_capacity: 32,
        overflow_policy: MeasurementPlotOverflowPolicy::EvenlySpaced,
    };
    let structured = profile.into_structured().unwrap();
    assert_eq!(
        MeasurementPlotProfile::from_structured(structured).unwrap(),
        profile
    );

    let points = (0..32)
        .map(|index| MeasurementPlotPoint::new(index, index as i64, index as i64).unwrap())
        .collect();
    let series = MeasurementPlotSeries::from_projected(points, 40, 8).unwrap();
    let structured = series.clone().into_structured().unwrap();
    assert_eq!(
        MeasurementPlotSeries::from_structured(structured).unwrap(),
        series
    );
}

#[test]
fn measurement_plot_series_retains_empty_oversize_and_accounting_refusals() {
    assert_eq!(
        MeasurementPlotSeries::from_projected(Vec::new(), 0, 0),
        Err(MeasurementPlotRefusal::InvalidProjection)
    );
    let points = (0..33)
        .map(|index| MeasurementPlotPoint::new(index, index as i64, index as i64).unwrap())
        .collect();
    assert_eq!(
        MeasurementPlotSeries::from_projected(points, 33, 0),
        Err(MeasurementPlotRefusal::InvalidProjection)
    );
    assert_eq!(
        MeasurementPlotSeries::from_projected(
            vec![MeasurementPlotPoint::new(0, 0, 0).unwrap()],
            2,
            0,
        ),
        Err(MeasurementPlotRefusal::InvalidProjection)
    );
}
