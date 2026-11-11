use super::*;
use conduit_data::{decode_measurement_plot_series, MeasurementPlotPoint};

fn series() -> MeasurementPlotSeries {
    MeasurementPlotSeries::from_projected(
        vec![
            MeasurementPlotPoint::new(0, 0, 250_000).unwrap(),
            MeasurementPlotPoint::new(1, 500_000, 750_000).unwrap(),
            MeasurementPlotPoint::new(2, 1_000_000, 500_000).unwrap(),
        ],
        3,
        0,
    )
    .unwrap()
}
fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/fixture-inspection".into(),
        plan_id: "plan/fixture-inspection".into(),
        active_play_id: "play/fixture-inspection".into(),
        required_interaction_context: None,
    }
}

#[test]
fn measurement_face_preserves_each_typed_series_without_inventing_host_or_time() {
    let value = series();
    let inputs = ["Blue", "Green", "Yellow"].map(|label| MeasurementFaceSeries {
        label,
        series: &value,
    });
    let fragment =
        measurement_plot_fragment("measurements", "Measurements", &inputs, basis()).unwrap();
    assert_eq!(fragment.basis, basis());
    assert_eq!(
        fragment.subjects[0].role,
        FaceRole::Semantic(kind_id(MEASUREMENT_PLOT_FACE_KIND))
    );
    assert_eq!(fragment.properties.len(), 3);
    for (property, input) in fragment.properties.iter().zip(&inputs) {
        assert_eq!(property.name, input.label);
        let FacePropertyValue::TypedValue { contract, bytes } = &property.value else {
            panic!("typed series required")
        };
        assert_eq!(
            contract.value_kind.as_str(),
            MEASUREMENT_PLOT_SERIES_INFO_ID
        );
        assert_eq!(decode_measurement_plot_series(bytes).unwrap(), value);
    }
    assert!(fragment.temporal_facts.is_empty());
    assert!(fragment.text[0].text.contains("sample index"));
    assert!(fragment.text[0].text.contains("normalized"));
}

#[test]
fn measurement_face_refuses_collisions_invalid_names_and_unbounded_series() {
    let value = series();
    let duplicate = [
        MeasurementFaceSeries {
            label: "same",
            series: &value,
        },
        MeasurementFaceSeries {
            label: "same",
            series: &value,
        },
    ];
    assert_eq!(
        measurement_plot_fragment("plot", "Measurements", &duplicate, basis()).unwrap_err(),
        MeasurementFaceRefusal::DuplicateSeries
    );
    let too_many = ["a", "b", "c", "d", "e"].map(|label| MeasurementFaceSeries {
        label,
        series: &value,
    });
    assert_eq!(
        measurement_plot_fragment("plot", "Measurements", &too_many, basis()).unwrap_err(),
        MeasurementFaceRefusal::SeriesCount
    );
    let invalid = [MeasurementFaceSeries {
        label: "\n",
        series: &value,
    }];
    assert_eq!(
        measurement_plot_fragment("plot", "Measurements", &invalid, basis()).unwrap_err(),
        MeasurementFaceRefusal::InvalidName
    );
}
