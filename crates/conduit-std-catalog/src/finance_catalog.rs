//! Canonical Form catalog and finite hosted offers for exact finance Info.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::*;

pub const FINANCE_FIXTURE_KIND: &str = "finance/deterministic-fixture";
pub const FINANCE_ADD_KIND: &str = "finance/add-money";
pub const FINANCE_COMPARE_KIND: &str = "finance/compare-money";
pub const FINANCE_CONVERT_KIND: &str = "finance/convert-money";
pub const FINANCE_REVISION: &str = "conduit.std/finance-exact@1";
pub const FINANCE_PROFILE: &str = "std/finance-kernel-hosted@1";
pub const FINANCE_ARTIFACT: &str = "conduit-std-host/finance@1";
pub const FINANCE_HOST_OPERATION: &str = "conduit.host/finance-exact@1";

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

pub fn finance_std_offers() -> Vec<CapabilityOffer> {
    let money = finance_money_type();
    vec![
        offer(
            FINANCE_FIXTURE_KIND,
            vec![],
            vec![
                port("convertible", &money, PortDirection::Output),
                port(
                    "events",
                    &finance_transaction_events_type(),
                    PortDirection::Output,
                ),
                port("left", &money, PortDirection::Output),
                port("quote", &finance_quote_type(), PortDirection::Output),
                port("rate", &finance_rate_type(), PortDirection::Output),
                port("right", &money, PortDirection::Output),
            ],
        ),
        offer(
            FINANCE_ADD_KIND,
            vec![
                port("left", &money, PortDirection::Input),
                port("right", &money, PortDirection::Input),
            ],
            vec![port("sum", &money, PortDirection::Output)],
        ),
        offer(
            FINANCE_COMPARE_KIND,
            vec![
                port("left", &money, PortDirection::Input),
                port("right", &money, PortDirection::Input),
            ],
            vec![port(
                "result",
                &finance_money_comparison_type(),
                PortDirection::Output,
            )],
        ),
        offer(
            FINANCE_CONVERT_KIND,
            vec![
                port("money", &money, PortDirection::Input),
                port("rate", &finance_rate_type(), PortDirection::Input),
            ],
            vec![port("converted", &money, PortDirection::Output)],
        ),
    ]
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

fn offer(kind: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(FINANCE_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(FINANCE_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(FINANCE_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(FINANCE_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
