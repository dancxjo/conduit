use super::*;
use QuantityFormRefusal as R;

fn temperature(integer: QuantityFormInteger) -> DecimalQuantityForm {
    DecimalQuantityForm {
        integer,
        unit: Unit::Celsius,
        exponent: -2,
        role: QuantityRole::Point,
    }
}

#[test]
fn selected_width_is_exact_for_decode_and_encode_without_truncation() {
    for integer in [
        QuantityFormInteger::I16Le,
        QuantityFormInteger::U16Le,
        QuantityFormInteger::I128Le,
    ] {
        let form = temperature(integer);
        let value = Quantity::parse_plot_literal("12.34°C").unwrap();
        for width in 0..=17 {
            let mut bytes = [0xa5; 17];
            if width == form.exact_bytes() {
                assert_eq!(form.encode_into(value, &mut bytes[..width]), Ok(width));
                assert_eq!(form.decode(&bytes[..width]), Ok(value));
            } else {
                assert_eq!(
                    form.encode_into(value, &mut bytes[..width]),
                    Err(R::WrongLength)
                );
                assert_eq!(bytes, [0xa5; 17]);
                assert_eq!(form.decode(&bytes[..width]), Err(R::WrongLength));
            }
        }
    }
    assert_eq!(
        temperature(QuantityFormInteger::I128Le)
            .encode(Quantity::parse_plot_literal("12.34°C").unwrap()),
        Err(R::WrongLength)
    );
}

#[test]
fn i128_forms_preserve_exact_fixed_point_quantity_meaning() {
    for (coefficient, exponent, unit, role, expected) in [
        (
            -1234_i128,
            -2,
            Unit::Celsius,
            QuantityRole::Point,
            "-12.34°C",
        ),
        (
            25767233 * 390625,
            -8,
            Unit::Pascal,
            QuantityRole::Linear,
            "100653.25390625Pa",
        ),
        (
            51200 * 9765625,
            -10,
            Unit::Percent,
            QuantityRole::Linear,
            "50%",
        ),
    ] {
        let form = DecimalQuantityForm {
            integer: QuantityFormInteger::I128Le,
            unit,
            exponent,
            role,
        };
        let expected = Quantity::parse_plot_literal(expected).unwrap();
        assert_eq!(form.decode(&coefficient.to_le_bytes()), Ok(expected));
        let mut encoded = [0; 16];
        assert_eq!(form.encode_into(expected, &mut encoded), Ok(16));
        assert_eq!(encoded, coefficient.to_le_bytes());
    }
}

#[test]
fn coefficient_exponent_family_role_and_inexact_refusals_remain_strict() {
    let form = temperature(QuantityFormInteger::I128Le);
    for coefficient in [i128::MIN, i128::MAX] {
        assert_eq!(form.decode(&coefficient.to_le_bytes()), Err(R::Overflow));
    }
    let maximum = 10_i128.pow(38) - 1;
    assert!(form.decode(&maximum.to_le_bytes()).is_ok());
    assert!(form.decode(&(-maximum).to_le_bytes()).is_ok());
    let mut invalid = form;
    invalid.exponent = QUANTITY_MAX_EXPONENT + 1;
    assert_eq!(invalid.decode(&[0; 16]), Err(R::InvalidDefinition));
    invalid = form;
    invalid.role = QuantityRole::Linear;
    assert_eq!(invalid.decode(&[0; 16]), Err(R::InvalidDefinition));
    let mut out = [0; 16];
    assert_eq!(
        form.encode_into(Quantity::parse_plot_literal("1Pa").unwrap(), &mut out),
        Err(R::WrongFamily)
    );
    let delta = Quantity::from_decimal_role(1, 0, Unit::Celsius, QuantityRole::Delta).unwrap();
    assert_eq!(form.encode_into(delta, &mut out), Err(R::WrongRole));
    assert_eq!(
        form.encode_into(Quantity::parse_plot_literal("0.001°C").unwrap(), &mut out),
        Err(R::Inexact)
    );
    assert_eq!(
        form.encode_into(
            Quantity::from_decimal(1, 38, Unit::Celsius).unwrap(),
            &mut out
        ),
        Err(R::Overflow)
    );
}

#[test]
fn existing_two_byte_forms_keep_exact_encoding_and_range_refusals() {
    let signed = temperature(QuantityFormInteger::I16Le);
    assert_eq!(
        signed.encode(Quantity::parse_plot_literal("-12.34°C").unwrap()),
        Ok((-1234_i16).to_le_bytes())
    );
    assert_eq!(
        signed.encode(Quantity::parse_plot_literal("327.68°C").unwrap()),
        Err(R::Overflow)
    );
    let unsigned = temperature(QuantityFormInteger::U16Le);
    assert_eq!(
        unsigned.encode(Quantity::parse_plot_literal("655.35°C").unwrap()),
        Ok(u16::MAX.to_le_bytes())
    );
    assert_eq!(
        unsigned.encode(Quantity::parse_plot_literal("-0.01°C").unwrap()),
        Err(R::Overflow)
    );
    assert_eq!(
        signed.encode(Quantity::parse_plot_literal("0.001°C").unwrap()),
        Err(R::Inexact)
    );
}
