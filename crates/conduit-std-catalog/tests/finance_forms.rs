use conduit_core::{
    BootId, ConnectionBase, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PortTemporal, StructuredInfoValue, StructuredInfoValueShape, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    add_money, add_money_values, compare_money, compare_money_values, convert_money,
    convert_money_values, decode_money_value, deterministic_finance_fixture,
    deterministic_rate_observation, finance_std_offers, install_finance_catalogs, Currency,
    FinanceRefusal, FixedDecimal, Money, FINANCE_ADD_KIND, FINANCE_COMPARE_KIND,
    FINANCE_CONVERT_KIND, FINANCE_FIXED_DECIMAL_INFO_ID, FINANCE_FIXTURE_KIND,
    FINANCE_HOST_OPERATION, FINANCE_MAXIMUM_DECIMAL_SCALE,
};
use core::cmp::Ordering;

const SOURCE: &str = include_str!("../../../examples/money-quote.conduit");

#[test]
fn canonical_form_flows_money_quotes_events_and_exact_comparison() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_finance_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let mut selector_offers = Vec::new();
    for stage in checked.forms[0]
        .cords
        .iter()
        .flat_map(|cord| cord.stages.iter())
    {
        if let CheckedCordStage::StructuredSelector { selector, .. } = stage {
            profile
                .insert(structured_selector_definition(
                    selector,
                    PortTemporal::Value,
                ))
                .unwrap();
            selector_offers.push(conduit_std_catalog::structured_selector_std_offer(
                selector,
                PortTemporal::Value,
            ));
        }
    }
    assert_eq!(selector_offers.len(), 1);
    let authored = expand_canonical_form_for_authoring(&checked, "money-quote", &profile).unwrap();
    let mut offers = finance_std_offers();
    offers.extend(selector_offers);
    let host = host(offers);
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    for kind in [
        FINANCE_FIXTURE_KIND,
        FINANCE_ADD_KIND,
        FINANCE_COMPARE_KIND,
        FINANCE_CONVERT_KIND,
    ] {
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .unwrap();
        assert_eq!(
            placement.host_operations[0].contract_id.as_str(),
            FINANCE_HOST_OPERATION
        );
    }
}

#[test]
fn same_currency_arithmetic_and_comparison_are_exact_across_scales() {
    let left = Money {
        amount: FixedDecimal::new(123, 2).unwrap(),
        currency: Currency::Usd,
    };
    let right = Money {
        amount: FixedDecimal::new(7, 1).unwrap(),
        currency: Currency::Usd,
    };
    let sum = add_money(left, right).unwrap();
    assert_eq!(sum.amount, FixedDecimal::new(193, 2).unwrap());
    assert_eq!(compare_money(left, right), Ok(Ordering::Greater));
    assert_eq!(sum.amount.encode().len(), 9);
    assert_eq!(FINANCE_FIXED_DECIMAL_INFO_ID, "finance/fixed-decimal@1");
    assert_eq!(
        FixedDecimal::new(1, FINANCE_MAXIMUM_DECIMAL_SCALE + 1),
        Err(FinanceRefusal::ScaleOutOfRange {
            maximum: FINANCE_MAXIMUM_DECIMAL_SCALE,
            actual: FINANCE_MAXIMUM_DECIMAL_SCALE + 1,
        })
    );
}

#[test]
fn cross_currency_requires_one_explicit_exact_rate_observation() {
    let euros = Money {
        amount: FixedDecimal::new(1_000, 2).unwrap(),
        currency: Currency::Eur,
    };
    let dollars = Money {
        amount: FixedDecimal::new(1_000, 2).unwrap(),
        currency: Currency::Usd,
    };
    assert_eq!(
        add_money(euros, dollars),
        Err(FinanceRefusal::CurrencyMismatch {
            left: Currency::Eur,
            right: Currency::Usd,
        })
    );
    let rate = deterministic_rate_observation().unwrap();
    assert_eq!(rate.source, "fixture/ecb-reference");
    assert_eq!(rate.profile, "finance/exact-decimal-rate@1");
    assert_eq!(
        convert_money(euros, &rate).unwrap(),
        Money {
            amount: FixedDecimal::new(108_250_000, 7).unwrap(),
            currency: Currency::Usd,
        }
    );
}

#[test]
fn deterministic_fixture_keeps_quote_age_transaction_variants_and_types_visible() {
    let fixture = deterministic_finance_fixture().unwrap();
    let sum = add_money_values(&fixture.left, &fixture.right).unwrap();
    assert_eq!(
        decode_money_value(&sum).unwrap(),
        Money {
            amount: FixedDecimal::new(1_300, 2).unwrap(),
            currency: Currency::Usd,
        }
    );
    assert_eq!(
        variant_tag(&compare_money_values(&fixture.left, &fixture.right).unwrap()),
        "greater"
    );
    assert_eq!(
        decode_money_value(&convert_money_values(&fixture.convertible, &fixture.rate).unwrap())
            .unwrap(),
        Money {
            amount: FixedDecimal::new(108_250_000, 7).unwrap(),
            currency: Currency::Usd,
        }
    );
    assert_eq!(
        variant_tag(record_field(&fixture.quote, "freshness")),
        "stale"
    );
    assert_eq!(
        leaf_text(record_field(&fixture.quote, "source")),
        "fixture/eur-usd"
    );
    let events = collection(&fixture.events);
    assert_eq!(events.len(), 3);
    assert_eq!(variant_tag(&events[0]), "placed");
    assert_eq!(variant_tag(&events[1]), "filled");
    assert_eq!(variant_tag(&events[2]), "rejected");
    for event in events {
        let StructuredInfoValueShape::Variant { payload, .. } = event.shape() else {
            panic!("expected transaction variant")
        };
        assert!(record_field(payload, "observed_at")
            .value_type()
            .profile()
            .is_ok());
    }
}

fn host(capabilities: Vec<conduit_core::CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/finance-proof"),
        boot_id: BootId::from("boot/finance-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/finance-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}

fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn collection(value: &StructuredInfoValue) -> &[StructuredInfoValue] {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("expected collection")
    };
    values
}

fn variant_tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("expected variant")
    };
    tag
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected leaf")
    };
    core::str::from_utf8(bytes).unwrap()
}
