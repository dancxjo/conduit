use conduit_core::{Quantity, Unit};
use conduit_environment_face::*;
use conduit_face::{Face, FaceBasis, FaceContributionBasis, FacePropertyValue};
use conduit_seven_segment_mask::{
    render, MaskDisposition, MaskUnavailable, PreparedEnvironmentFace, SegmentPatternForm,
    SevenSegmentLook, SevenSegmentSelection as Selection,
};
fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/environment".into(),
        plan_id: "plan/environment".into(),
        active_play_id: "play/environment".into(),
        required_interaction_context: None,
    }
}
fn q(text: &str) -> Quantity {
    Quantity::parse_plot_literal(text).unwrap()
}
fn face(
    temp: Option<Quantity>,
    humidity: Option<Quantity>,
    fresh: bool,
    status: SensorStatus,
) -> Face {
    let basis = basis();
    let fragment = contribution(temp, humidity, fresh, status, basis.clone()).unwrap();
    assert_eq!(fragment.basis, basis);
    assert!(fragment.properties.len() <= 5);
    Face::new(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/environment".into()),
            checked_plot_id: Some(basis.checked_plot_id),
            expanded_plot_id: Some("expanded/environment".into()),
            plan_id: Some(basis.plan_id),
            active_play_id: Some(basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
    )
    .unwrap()
}
fn property<'a>(face: &'a Face, name: &str) -> &'a FacePropertyValue {
    &face
        .properties
        .iter()
        .find(|p| p.name == name)
        .unwrap()
        .value
}
#[test]
fn semantic_observation_reaches_fahrenheit_show_without_changing_the_face() {
    for literal in ["19.8°C", "67.64°F"] {
        let temperature = q(literal);
        let humidity = q("56%");
        let face = face(
            Some(temperature),
            Some(humidity),
            true,
            SensorStatus::Available,
        );
        for (name, expected) in [
            ("temperature", temperature),
            ("relative-humidity", humidity),
        ] {
            let FacePropertyValue::TypedValue { contract, bytes } = property(&face, name) else {
                panic!("semantic quantity")
            };
            contract.validate(bytes).unwrap();
            assert_eq!(Quantity::decode(bytes).unwrap(), expected);
        }
        let projection = PreparedEnvironmentFace::prepare(&face, SUBJECT).unwrap();
        let output = render(
            &projection,
            Selection::Temperature,
            &SevenSegmentLook::fahrenheit(),
        );
        assert_eq!(
            output
                .show
                .digits()
                .map(|d| SegmentPatternForm::encode(d)[0]),
            [0x7d, 0x07, 0x7d, 0x71]
        );
        assert_eq!(*output.show.decimal_points(), [false, true, false, false]);
        assert!(!output.show.colon());
        assert_eq!(
            output.disposition,
            MaskDisposition::Temperature { decimal_places: 1 }
        );
    }
}
#[test]
fn freshness_failure_and_last_observation_remain_independent() {
    for (fresh, status, why) in [
        (false, SensorStatus::Available, MaskUnavailable::Stale),
        (
            true,
            SensorStatus::Unavailable(SensorFailure::CaptureTimeout),
            MaskUnavailable::Sensor,
        ),
    ] {
        let face = face(Some(q("19.8°C")), Some(q("56%")), fresh, status);
        assert_eq!(property(&face, "fresh"), &FacePropertyValue::Flag(fresh));
        let view = PreparedEnvironmentFace::prepare(&face, SUBJECT).unwrap();
        let output = render(
            &view,
            Selection::Temperature,
            &SevenSegmentLook::fahrenheit(),
        );
        assert_eq!(output.disposition, MaskDisposition::Unavailable(why));
        assert_eq!(
            output
                .show
                .digits()
                .map(|d| SegmentPatternForm::encode(d)[0]),
            [0x40; 4]
        );
        if let SensorStatus::Unavailable(reason) = status {
            assert_eq!(
                property(&face, "sensor-failure"),
                &FacePropertyValue::Text(reason.label().into())
            );
        }
    }
}
#[test]
fn warming_and_temperature_only_sensors_do_not_fabricate_values() {
    let warming = face(None, None, false, SensorStatus::Warming);
    assert!(warming
        .properties
        .iter()
        .all(|p| p.name != "temperature" && p.name != "relative-humidity"));
    let only = face(Some(q("20°C")), None, true, SensorStatus::Available);
    let view = PreparedEnvironmentFace::prepare(&only, SUBJECT).unwrap();
    assert_eq!(
        render(&view, Selection::Humidity, &SevenSegmentLook::fahrenheit()).disposition,
        MaskDisposition::Unavailable(MaskUnavailable::MissingValue)
    );
}
#[test]
fn invalid_semantic_inputs_refuse_before_a_face_is_published() {
    use ContributionRefusal as R;
    for (t, h, fresh, status, expected) in [
        (
            Some(q("1m")),
            None,
            true,
            SensorStatus::Available,
            R::InvalidTemperature,
        ),
        (
            Some(q("20°C")),
            Some(q("1m")),
            true,
            SensorStatus::Available,
            R::InvalidHumidity,
        ),
        (
            Some(q("20°C")),
            Some(q("101%")),
            true,
            SensorStatus::Available,
            R::InvalidHumidity,
        ),
        (
            Some(q("20°C")),
            Some(q("-1%")),
            true,
            SensorStatus::Available,
            R::InvalidHumidity,
        ),
        (
            None,
            None,
            false,
            SensorStatus::Available,
            R::MissingTemperature,
        ),
        (
            None,
            None,
            true,
            SensorStatus::Warming,
            R::FreshWithoutObservation,
        ),
    ] {
        assert_eq!(
            contribution(t, h, fresh, status, basis()).unwrap_err(),
            expected
        );
    }
    let ratio = Quantity::new(1, Unit::One);
    assert!(contribution(
        Some(q("20°C")),
        Some(ratio),
        true,
        SensorStatus::Available,
        basis()
    )
    .is_ok());
}
