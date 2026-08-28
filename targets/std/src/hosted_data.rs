//! Finite std Host offers for the host-neutral finance and tabular contracts.

use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_data::*;

pub const FINANCE_PROFILE: &str = "std/finance-kernel-hosted@1";
pub const FINANCE_ARTIFACT: &str = "conduit-std-host/finance@1";
pub const FINANCE_HOST_OPERATION: &str = "conduit.host/finance-exact@1";
pub const TABULAR_PROFILE: &str = "std/tabular-kernel-hosted@1";
pub const TABULAR_ARTIFACT: &str = "conduit-std-host/tabular@1";
pub const TABULAR_HOST_OPERATION: &str = "conduit.host/tabular@1";

pub fn finance_std_offers() -> Vec<CapabilityOffer> {
    let money = finance_money_type();
    vec![
        offer(
            FINANCE_FIXTURE_KIND,
            FINANCE_REVISION,
            FINANCE_PROFILE,
            FINANCE_ARTIFACT,
            FINANCE_HOST_OPERATION,
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
            FINANCE_REVISION,
            FINANCE_PROFILE,
            FINANCE_ARTIFACT,
            FINANCE_HOST_OPERATION,
            vec![
                port("left", &money, PortDirection::Input),
                port("right", &money, PortDirection::Input),
            ],
            vec![port("sum", &money, PortDirection::Output)],
        ),
        offer(
            FINANCE_COMPARE_KIND,
            FINANCE_REVISION,
            FINANCE_PROFILE,
            FINANCE_ARTIFACT,
            FINANCE_HOST_OPERATION,
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
            FINANCE_REVISION,
            FINANCE_PROFILE,
            FINANCE_ARTIFACT,
            FINANCE_HOST_OPERATION,
            vec![
                port("money", &money, PortDirection::Input),
                port("rate", &finance_rate_type(), PortDirection::Input),
            ],
            vec![port("converted", &money, PortDirection::Output)],
        ),
    ]
}

pub fn tabular_std_offers() -> Vec<CapabilityOffer> {
    let result = tabular_query_result_type();
    vec![
        offer(
            TABULAR_PROVIDER_KIND,
            TABULAR_REVISION,
            TABULAR_PROFILE,
            TABULAR_ARTIFACT,
            TABULAR_HOST_OPERATION,
            vec![],
            vec![port("result", &result, PortDirection::Output)],
        ),
        offer(
            TABULAR_FILTER_KIND,
            TABULAR_REVISION,
            TABULAR_PROFILE,
            TABULAR_ARTIFACT,
            TABULAR_HOST_OPERATION,
            vec![port("result", &result, PortDirection::Input)],
            vec![port("result", &result, PortDirection::Output)],
        ),
    ]
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[allow(clippy::too_many_arguments)]
fn offer(
    kind: &str,
    revision: &str,
    profile: &str,
    artifact: &str,
    operation: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(operation),
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
