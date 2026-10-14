use super::parse_scalar_configuration;

#[test]
fn decimals_lower_to_exact_microunits_without_reinterpreting_integer_literals() {
    assert_eq!(parse_scalar_configuration("0.5"), Some(500_000));
    assert_eq!(parse_scalar_configuration("0.015"), Some(15_000));
    assert_eq!(parse_scalar_configuration("-1.25"), Some(-1_250_000));
    assert_eq!(parse_scalar_configuration("500000"), Some(500_000));
    assert_eq!(parse_scalar_configuration("0.0000001"), None);
    assert_eq!(parse_scalar_configuration(".5"), None);
}

#[test]
fn data_labels_require_meaningful_bounded_text() {
    use crate::{ConfigurationValue, KindConfigurationField, KindConfigurationRule};
    let field = KindConfigurationField {
        key: "label".into(),
        default_value: ConfigurationValue::Text("".into()),
        rule: KindConfigurationRule::DataLabel { maximum: 16 },
    };
    for label in [
        "",
        "   ",
        "\t",
        "Room\nheat",
        "Room\u{7f}heat",
        "12345678901234567",
    ] {
        assert!(super::validate_configuration_value(
            &field,
            &ConfigurationValue::Text(label.into())
        )
        .is_err());
    }
    assert!(super::validate_configuration_value(&field, &ConfigurationValue::U64(12)).is_err());
    assert!(super::validate_configuration_value(
        &field,
        &ConfigurationValue::Text("Room temperature".into())
    )
    .is_ok());
}
