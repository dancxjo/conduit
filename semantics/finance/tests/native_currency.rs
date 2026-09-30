use conduit_finance::{
    finance_currency_type, finance_fixed_decimal_type, finance_instrument_type,
    finance_money_comparison_type, finance_money_type, Currency, FinanceCurrency,
    FinanceCurrencyPair, FinanceFixedDecimal, FinanceMoney, FinanceMoneyComparison,
    FINANCE_MAXIMUM_DECIMAL_SCALE,
};
use conduit_form::rust_binding::NativeRustBinding;

#[test]
fn native_currency_source_owns_catalog_identity_and_rust_round_trip() {
    assert_eq!(
        finance_currency_type(),
        FinanceCurrency::semantic_type().unwrap()
    );
    for (currency, tag) in [
        (Currency::Eur, "eur"),
        (Currency::Gbp, "gbp"),
        (Currency::Usd, "usd"),
    ] {
        assert_eq!(currency.tag(), tag);
        assert_eq!(Currency::from_tag(tag).unwrap(), currency);
        let encoded = currency.clone().encode().unwrap();
        assert_eq!(FinanceCurrency::decode(&encoded).unwrap(), currency);
    }
}

#[test]
fn native_money_source_owns_catalog_identity_and_nested_round_trip() {
    assert_eq!(finance_money_type(), FinanceMoney::semantic_type().unwrap());
    let money =
        FinanceMoney::new(FinanceFixedDecimal::new(12_345, 2).unwrap(), Currency::Usd).unwrap();
    let encoded = money.clone().encode().unwrap();
    assert_eq!(FinanceMoney::decode(&encoded).unwrap(), money);
}

#[test]
fn native_fixed_decimal_owns_scale_bound_identity_and_round_trip() {
    assert_eq!(
        finance_fixed_decimal_type(),
        FinanceFixedDecimal::semantic_type().unwrap()
    );
    let decimal = FinanceFixedDecimal::new(-12_345, 3).unwrap();
    assert_eq!(decimal.coefficient(), &-12_345);
    assert_eq!(decimal.scale(), &3);
    let encoded = decimal.clone().encode().unwrap();
    assert_eq!(FinanceFixedDecimal::decode(&encoded).unwrap(), decimal);
    assert!(FinanceFixedDecimal::new(1, FINANCE_MAXIMUM_DECIMAL_SCALE + 1).is_err());
}

#[test]
fn native_currency_pair_source_owns_catalog_identity_and_round_trip() {
    assert_eq!(
        finance_instrument_type(),
        FinanceCurrencyPair::semantic_type().unwrap()
    );
    let pair = FinanceCurrencyPair::new(Currency::Eur, Currency::Usd).unwrap();
    assert_eq!(pair.base(), &Currency::Eur);
    assert_eq!(pair.quote(), &Currency::Usd);
    let encoded = pair.clone().encode().unwrap();
    assert_eq!(FinanceCurrencyPair::decode(&encoded).unwrap(), pair);
}

#[test]
fn native_comparison_source_owns_catalog_identity_and_rust_round_trip() {
    assert_eq!(
        finance_money_comparison_type(),
        FinanceMoneyComparison::semantic_type().unwrap()
    );
    for comparison in [
        FinanceMoneyComparison::equal(),
        FinanceMoneyComparison::greater(),
        FinanceMoneyComparison::less(),
    ] {
        let encoded = comparison.clone().encode().unwrap();
        assert_eq!(
            FinanceMoneyComparison::decode(&encoded).unwrap(),
            comparison
        );
    }
}
