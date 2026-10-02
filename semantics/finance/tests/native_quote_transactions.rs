use conduit_finance::{
    finance_freshness_type, finance_instant_type, finance_quote_type,
    finance_transaction_event_type, finance_transaction_events_type, FinanceObservedInstant,
    FinanceOrderIdentity, FinanceQuote, FinanceQuoteFreshness, FinanceQuoteSource,
    FinanceRejectionReason, FinanceTransactionEvent, FinanceTransactionEventsThree,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[test]
fn quote_and_transaction_family_is_owned_by_native_types() {
    assert_eq!(
        finance_instant_type(),
        FinanceObservedInstant::semantic_type().unwrap()
    );
    assert_eq!(
        finance_freshness_type(),
        FinanceQuoteFreshness::semantic_type().unwrap()
    );
    assert_eq!(finance_quote_type(), FinanceQuote::semantic_type().unwrap());
    assert_eq!(
        finance_transaction_event_type(),
        FinanceTransactionEvent::semantic_type().unwrap()
    );
    assert_eq!(
        finance_transaction_events_type(),
        FinanceTransactionEventsThree::semantic_type().unwrap()
    );

    let instant = FinanceObservedInstant::new(1_788_000_000).unwrap();
    assert_eq!(
        FinanceObservedInstant::from_structured(instant.clone().into_structured().unwrap())
            .unwrap(),
        instant
    );
}

#[test]
fn portable_text_bounds_refuse_empty_identifiers_and_sources() {
    for refusal in [
        FinanceQuoteSource::new(String::new()).unwrap_err(),
        FinanceOrderIdentity::new(String::new()).unwrap_err(),
        FinanceRejectionReason::new(String::new()).unwrap_err(),
    ] {
        assert!(matches!(
            refusal,
            NativeBindingRefusal::ViolatedConstraint { .. }
        ));
    }
}
