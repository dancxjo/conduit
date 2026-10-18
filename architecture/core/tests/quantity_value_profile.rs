//! Profile and physical Type admission; not an installed Source transport.
use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn rational(source: &str) -> Vec<u8> {
    let value =
        RationalQuantity::from_quantity(Quantity::parse_plot_literal(source).unwrap(), &mut work())
            .unwrap();
    let mut bytes = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let length = encode_rational_quantity_value(value, &mut bytes).unwrap();
    bytes.truncate(length);
    bytes
}
#[test]
fn optional_numeric_representation_preserves_the_declared_family_role_and_dimension() {
    let bytes = rational("1m");
    let quantity = Quantity::parse_plot_literal("1m").unwrap();
    let exact_family = quantity_role_info_id(quantity.family(), QuantityRole::Linear).unwrap();
    for kind in [QUANTITY_INFO_ID, DISTANCE_INFO_ID, exact_family.as_str()] {
        let value = QuantityValueProfile::Exact
            .decode(kind, &bytes, &mut work())
            .unwrap();
        assert_eq!(value.unit(), quantity.unit());
        assert_eq!(value.role(), quantity.role());
    }
    assert!(QuantityValueProfile::Exact
        .decode(DURATION_INFO_ID, &bytes, &mut work())
        .is_err());
    let mut wrong_kind_work = ExactNumericWork::new(0);
    assert_eq!(
        QuantityValueProfile::Exact
            .decode(F64_INFO_ID, &bytes, &mut wrong_kind_work)
            .unwrap_err(),
        QuantityValueContractRefusal::NotQuantityKind
    );
    assert_eq!(wrong_kind_work.spent(), 0);
    let point = rational("1°F");
    let temperature = Quantity::parse_plot_literal("1°F").unwrap();
    let point_kind = quantity_role_info_id(temperature.family(), QuantityRole::Point).unwrap();
    let delta_kind = quantity_role_info_id(temperature.family(), QuantityRole::Delta).unwrap();
    assert!(QuantityValueProfile::Exact
        .decode(&point_kind, &point, &mut work())
        .is_ok());
    assert_eq!(
        QuantityValueProfile::Exact
            .decode(&delta_kind, &point, &mut work())
            .unwrap_err(),
        QuantityValueContractRefusal::WrongRole
    );
    assert_eq!(
        QuantityValueProfile::Exact
            .decode(&exact_family, &point, &mut work())
            .unwrap_err(),
        QuantityValueContractRefusal::WrongFamily
    );
}
#[test]
fn decimal_consumers_refuse_an_exact_producer_without_an_admitted_projection() {
    use QuantityValueProfile::{Decimal, Exact};
    assert!(Decimal.accepts(Decimal));
    assert!(Exact.accepts(Decimal));
    assert!(Exact.accepts(Exact));
    assert!(!Decimal.accepts(Exact));
    assert_eq!(Decimal.maximum_encoded_bytes(), QUANTITY_ENCODED_LEN);
    assert_eq!(
        Exact.maximum_encoded_bytes(),
        QUANTITY_VALUE_MAX_ENCODED_BYTES
    );
    let bytes = rational("1m");
    let mut budget = ExactNumericWork::new(0);
    assert_eq!(
        Decimal
            .decode(QUANTITY_INFO_ID, &bytes, &mut budget)
            .unwrap_err(),
        QuantityValueContractRefusal::UnsupportedRepresentation {
            profile: Decimal,
            version: 2
        }
    );
    assert_eq!(budget.spent(), 0);
    let decimal = Quantity::parse_plot_literal("1m").unwrap().encode();
    assert!(Decimal
        .decode(DISTANCE_INFO_ID, &decimal, &mut work())
        .is_ok());
    // Explicit exact projection can narrow this value, preserving Unit/role.
    let narrowed = Exact
        .decode(DISTANCE_INFO_ID, &bytes, &mut work())
        .unwrap()
        .project_decimal(&mut work())
        .unwrap()
        .encode();
    assert!(Decimal
        .decode(DISTANCE_INFO_ID, &narrowed, &mut work())
        .is_ok());
    let mut truncated = bytes.clone();
    truncated.truncate(1);
    assert!(matches!(
        Exact.decode(QUANTITY_INFO_ID, &truncated, &mut work()),
        Err(QuantityValueContractRefusal::Encoding(_))
    ));
    assert!(matches!(
        Exact.decode(QUANTITY_INFO_ID, &bytes, &mut ExactNumericWork::new(0)),
        Err(QuantityValueContractRefusal::Encoding(_))
    ));
}
