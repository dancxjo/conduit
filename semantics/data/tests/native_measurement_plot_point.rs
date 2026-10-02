use conduit_data::{
    decode_measurement_plot_series, encode_measurement_plot_series, MeasurementPlotPoint,
    MeasurementPlotSeries, PLOT_AXIS_MILLIONTHS,
};
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn measurement_plot_point_round_trips_exact_native_bounds() {
    let value = MeasurementPlotPoint::new(u64::MAX, 0, PLOT_AXIS_MILLIONTHS).unwrap();
    let structured = value.into_structured().unwrap();
    assert_eq!(
        MeasurementPlotPoint::from_structured(structured).unwrap(),
        value
    );
    assert_eq!(*value.source_index(), u64::MAX);
    assert_eq!(*value.time_millionths(), 0);
    assert_eq!(*value.value_millionths(), PLOT_AXIS_MILLIONTHS);

    assert!(MeasurementPlotPoint::new(0, -1, 0).is_err());
    assert!(MeasurementPlotPoint::new(0, 0, PLOT_AXIS_MILLIONTHS + 1).is_err());
}

#[test]
fn measurement_plot_point_preserves_the_existing_series_wire_bytes() {
    let point = MeasurementPlotPoint::new(0, 0, PLOT_AXIS_MILLIONTHS).unwrap();
    let series = MeasurementPlotSeries::from_projected(vec![point], 1, 0).unwrap();
    let expected = [
        1, 1, // version and point count
        1, 0, 0, 0, 0, 0, 0, 0, // source samples
        0, 0, 0, 0, 0, 0, 0, 0, // omitted samples
        0, 0, 0, 0, 0, 0, 0, 0, // source index
        0, 0, 0, 0, 0, 0, 0, 0, // time millionths
        64, 66, 15, 0, 0, 0, 0, 0, // value millionths
    ];

    assert_eq!(encode_measurement_plot_series(&series).unwrap(), expected);
    assert_eq!(decode_measurement_plot_series(&expected).unwrap(), series);
}
