//! Canonical Form catalog for exact finance Info.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::*;

pub const FINANCE_FIXTURE_KIND: &str = "finance/deterministic-fixture";
pub const FINANCE_ADD_KIND: &str = "finance/add-money";
pub const FINANCE_COMPARE_KIND: &str = "finance/compare-money";
pub const FINANCE_CONVERT_KIND: &str = "finance/convert-money";
pub const FINANCE_REVISION: &str = "conduit.std/finance-exact@1";

pub fn install_finance_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in finance_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    let money = finance_money_type();
    let comparison = finance_money_comparison_type();
    let quote = finance_quote_type();
    let rate = finance_rate_type();
    let events = finance_transaction_events_type();
    insert_kind(
        startup,
        profile,
        FINANCE_FIXTURE_KIND,
        vec![],
        vec![
            port("convertible", &money, PortDirection::Output),
            port("events", &events, PortDirection::Output),
            port("left", &money, PortDirection::Output),
            port("quote", &quote, PortDirection::Output),
            port("rate", &rate, PortDirection::Output),
            port("right", &money, PortDirection::Output),
        ],
    )?;
    insert_kind(
        startup,
        profile,
        FINANCE_ADD_KIND,
        vec![
            port("left", &money, PortDirection::Input),
            port("right", &money, PortDirection::Input),
        ],
        vec![port("sum", &money, PortDirection::Output)],
    )?;
    insert_kind(
        startup,
        profile,
        FINANCE_COMPARE_KIND,
        vec![
            port("left", &money, PortDirection::Input),
            port("right", &money, PortDirection::Input),
        ],
        vec![port("result", &comparison, PortDirection::Output)],
    )?;
    insert_kind(
        startup,
        profile,
        FINANCE_CONVERT_KIND,
        vec![
            port("money", &money, PortDirection::Input),
            port("rate", &rate, PortDirection::Input),
        ],
        vec![port("converted", &money, PortDirection::Output)],
    )
}

fn finance_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (FINANCE_FIXED_DECIMAL_TYPE, finance_fixed_decimal_type()),
        (FINANCE_CURRENCY_TYPE, finance_currency_type()),
        (FINANCE_MONEY_TYPE, finance_money_type()),
        (FINANCE_INSTRUMENT_TYPE, finance_instrument_type()),
        (FINANCE_INSTANT_TYPE, finance_instant_type()),
        (FINANCE_FRESHNESS_TYPE, finance_freshness_type()),
        (FINANCE_QUOTE_TYPE, finance_quote_type()),
        (FINANCE_RATE_TYPE, finance_rate_type()),
        (
            FINANCE_TRANSACTION_EVENT_TYPE,
            finance_transaction_event_type(),
        ),
        (
            FINANCE_TRANSACTION_EVENTS_TYPE,
            finance_transaction_events_type(),
        ),
        (
            FINANCE_MONEY_COMPARISON_TYPE,
            finance_money_comparison_type(),
        ),
    ]
}

fn insert_kind(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(FINANCE_REVISION),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
