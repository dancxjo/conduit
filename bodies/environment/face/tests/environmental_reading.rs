use conduit_core::{Quantity, Unit};
use conduit_environment_face::{contribution_reading, EnvironmentalReading, SensorStatus};
use conduit_face::{FaceContributionBasis, FacePropertyValue};
fn basis() -> FaceContributionBasis {
    // Diagnostic fixture identities, never admitted producer evidence.
    FaceContributionBasis {
        checked_plot_id: "fixture/checked".into(),
        plan_id: "fixture/plan".into(),
        active_play_id: "fixture/play".into(),
        required_interaction_context: None,
    }
}
#[test]
fn common_face_preserves_bosch_quantities_and_explicit_bmp_support() {
    for humidity in [None, Some(0), Some(51_234), Some(102_400)] {
        let reading =
            EnvironmentalReading::centi_celsius_centi_pascal(-125, 10_065_328, humidity).unwrap();
        let expected_basis = basis();
        let fragment = contribution_reading(
            reading,
            true,
            SensorStatus::Available,
            expected_basis.clone(),
        )
        .unwrap();
        assert_eq!(fragment.basis, expected_basis);
        let property = |name| {
            &fragment
                .properties
                .iter()
                .find(|p| p.name == name)
                .unwrap()
                .value
        };
        let FacePropertyValue::TypedValue { contract, bytes } = property("atmospheric-pressure")
        else {
            panic!("typed pressure missing")
        };
        contract.validate(bytes).unwrap();
        assert_eq!(
            Quantity::decode(bytes).unwrap(),
            Quantity::from_decimal(10_065_328, -2, Unit::Pascal).unwrap()
        );
        assert!(contract
            .validate(&Quantity::new(1, Unit::Percent).encode())
            .is_err());
        assert!(contract
            .validate(&Quantity::new(0, Unit::Pascal).encode())
            .is_err());
        assert_eq!(
            property("relative-humidity-support"),
            &FacePropertyValue::Text(
                if humidity.is_some() {
                    "supported"
                } else {
                    "unsupported"
                }
                .into()
            )
        );
        assert_eq!(
            fragment
                .properties
                .iter()
                .any(|p| p.name == "relative-humidity"),
            humidity.is_some()
        );
    }
}
#[test]
fn legacy_lift_does_not_create_pressure_or_change_temperature() {
    let t = Quantity::from_decimal(731, -1, Unit::Fahrenheit).unwrap();
    let reading =
        EnvironmentalReading::temperature_humidity(t, Quantity::new(0, Unit::Percent)).unwrap();
    let fragment = contribution_reading(reading, true, SensorStatus::Available, basis()).unwrap();
    assert!(!fragment
        .properties
        .iter()
        .any(|p| p.name == "atmospheric-pressure"));
    let p = fragment
        .properties
        .iter()
        .find(|p| p.name == "temperature")
        .unwrap();
    let FacePropertyValue::TypedValue { bytes, .. } = &p.value else {
        panic!("temperature missing")
    };
    assert_eq!(bytes, &t.encode());
}
