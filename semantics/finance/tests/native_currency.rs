use conduit_finance::{
    finance_currency_type, finance_instrument_type, finance_money_comparison_type, Currency,
    FinanceCurrency, FinanceCurrencyPair, FinanceMoneyComparison,
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
