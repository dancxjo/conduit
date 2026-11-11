use super::*;
use alloc::{vec, vec::Vec};
fn series() -> MeasurementPlotSeries {
    MeasurementPlotSeries::from_projected(
        (0..32)
            .map(|i| {
                crate::MeasurementPlotPoint::new(
                    i,
                    i as i64 * 1_000_000 / 31,
                    i as i64 * 1_000_000 / 31,
                )
                .unwrap()
            })
            .collect(),
        32,
        0,
    )
    .unwrap()
}
fn entry(name: &str) -> LabelledMeasurementSeries {
    LabelledMeasurementSeries::new(name.into(), series()).unwrap()
}
#[test]
fn four_labelled_lines_roundtrip_order_points_and_standard_intrinsic_wrapping() {
    let names = ["Blue", "Green", "Yellow", "温室"];
    let chart = MeasurementChart::new(names.into_iter().map(entry).collect()).unwrap();
    let encoded = encode_measurement_chart(&chart).unwrap();
    assert_eq!(decode_measurement_chart(&encoded).unwrap(), chart);
    assert_eq!(
        chart
            .series()
            .iter()
            .map(|item| item.label())
            .collect::<Vec<_>>(),
        names
    );
    assert!(
        chart
            .series()
            .iter()
            .all(|item| item.series().points().len() == 32)
    );
    let canonical = conduit_core::StructuredInfoValue::leaf(measurement_chart_type(), encoded)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert!(canonical.len() <= measurement_chart_canonical_bytes());
    let value = conduit_core::validate_canonical_structured_value(&canonical).unwrap();
    assert_eq!(
        decode_measurement_chart(value.primitive_bytes(MEASUREMENT_CHART_INFO_ID).unwrap())
            .unwrap(),
        chart
    );
}
#[test]
fn count_label_duplicate_and_physical_capacity_refuse() {
    assert_eq!(
        MeasurementChart::new(vec![]),
        Err(MeasurementChartRefusal::Count)
    );
    assert_eq!(
        MeasurementChart::new(vec![entry("same"), entry("same")]),
        Err(MeasurementChartRefusal::DuplicateLabel)
    );
    assert_eq!(
        MeasurementChart::new((0..5).map(|i| entry(&alloc::format!("{i}"))).collect()),
        Err(MeasurementChartRefusal::Count)
    );
    for label in ["", " ", "a\nb", "a\0b", &"a".repeat(65)] {
        assert_eq!(
            LabelledMeasurementSeries::new(label.into(), series()),
            Err(MeasurementChartRefusal::InvalidLabel)
        );
    }
    let mut oversized = Vec::with_capacity(5);
    oversized.push(entry("Blue"));
    assert_eq!(
        MeasurementChart::new(oversized),
        Err(MeasurementChartRefusal::Capacity)
    );
}
#[test]
fn truncated_oversize_badversion_badutf8_and_trailing_encoding_refuse() {
    let bytes =
        encode_measurement_chart(&MeasurementChart::new(vec![entry("Blue")]).unwrap()).unwrap();
    for length in 0..bytes.len() {
        assert!(decode_measurement_chart(&bytes[..length]).is_err());
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert_eq!(
        decode_measurement_chart(&bad),
        Err(MeasurementChartRefusal::Encoding)
    );
    bad = bytes.clone();
    bad[0] = 2;
    assert_eq!(
        decode_measurement_chart(&bad),
        Err(MeasurementChartRefusal::UnsupportedVersion)
    );
    bad = bytes.clone();
    bad[1] = 5;
    assert_eq!(
        decode_measurement_chart(&bad),
        Err(MeasurementChartRefusal::Count)
    );
    bad = bytes.clone();
    bad[3] = 255;
    assert_eq!(
        decode_measurement_chart(&bad),
        Err(MeasurementChartRefusal::InvalidLabel)
    );
    bad = bytes.clone();
    bad[2] = 65;
    assert_eq!(
        decode_measurement_chart(&bad),
        Err(MeasurementChartRefusal::InvalidLabel)
    );
    bad = bytes.clone();
    bad[7..9].copy_from_slice(&1025u16.to_le_bytes());
    assert_eq!(
        decode_measurement_chart(&bad),
        Err(MeasurementChartRefusal::Capacity)
    );
    assert_eq!(
        decode_measurement_chart(&vec![0; MAXIMUM_MEASUREMENT_CHART_BYTES + 1]),
        Err(MeasurementChartRefusal::Capacity)
    );
    let duplicate = encode_measurement_chart(
        &MeasurementChart::new(vec![entry("Blue"), entry("Gold")]).unwrap(),
    )
    .unwrap();
    let mut bad = duplicate;
    let second = 2
        + 1
        + 4
        + 2
        + crate::encode_measurement_plot_series(&series())
            .unwrap()
            .len();
    bad[second + 1..second + 5].copy_from_slice(b"Blue");
    assert_eq!(
        decode_measurement_chart(&bad),
        Err(MeasurementChartRefusal::DuplicateLabel)
    );
}
#[test]
fn bounds_are_explicit_and_one_chart_input_is_authored_with_label() {
    assert_eq!(MAXIMUM_MEASUREMENT_CHART_BYTES, 4366);
    assert!(measurement_chart_decode_workspace_bytes() > MAXIMUM_MEASUREMENT_CHART_BYTES);
    let chart = MeasurementChart::new(
        (0..4)
            .map(|i| entry(&alloc::format!("{}{}", i, "x".repeat(63))))
            .collect(),
    )
    .unwrap();
    assert_eq!(encode_measurement_chart(&chart).unwrap().len(), 3414);
    let kind = crate::measurement_chart_face_semantic_contract();
    kind.validate().unwrap();
    assert_eq!(kind.inputs.len(), 1);
    assert_eq!(kind.inputs[0].port_id.as_str(), "chart");
    assert!(kind.outputs.is_empty());
    assert_eq!(kind.startup_parameters[0].name, "label");
    assert_eq!(kind.configuration[0].key, "label");
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    crate::install_measurement_chart_catalog(&mut startup, &mut profiles).unwrap();
    let document = conduit_plot::parse_syntax_document(
        r#"plot chart (
    >> chart: MeasurementChart...|
) {
    show: face/measurement-chart(label = "Conditions")
    chart >> show.chart
}"#,
    );
    conduit_plot::check_syntax_document(&document, &startup)
        .unwrap_or_else(|diagnostic| panic!("{diagnostic:?}"));
}
