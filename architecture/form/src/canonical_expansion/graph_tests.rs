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
