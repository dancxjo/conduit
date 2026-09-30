use conduit_finance::{finance_currency_type, Currency, FinanceCurrency};
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
