//! Form catalog and hosted offer boundary for application networking.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    application_network_registered_types, dns_query_type, dns_result_type,
    network_connection_state_type, network_endpoint_type,
};

pub const DNS_FIXTURE_KIND: &str = "net/deterministic-dns-query";
pub const DNS_RESOLVE_KIND: &str = "net/resolve-dns";
pub const ENDPOINT_FIXTURE_KIND: &str = "net/deterministic-endpoint";
pub const NETWORK_CONNECT_KIND: &str = "net/connect";
pub const APPLICATION_NETWORK_REVISION: &str = "conduit.net/application-network@1";
pub const APPLICATION_NETWORK_PROFILE: &str = "std/application-network-hosted@1";
pub const APPLICATION_NETWORK_ARTIFACT: &str = "conduit-std-host/application-network@1";
pub const DNS_RESOLVE_OPERATION: &str = "conduit.host/dns-resolve@1";
pub const NETWORK_CONNECT_OPERATION: &str = "conduit.host/network-connect@1";
pub const DNS_FIXTURE_OPERATION: &str = "conduit.host/dns-fixture@1";
pub const ENDPOINT_FIXTURE_OPERATION: &str = "conduit.host/endpoint-fixture@1";
pub const DNS_RESOLVER_RESOURCE: &str = "conduit.resource/network/dns-resolver@1";
pub const NETWORK_CONNECTION_RESOURCE: &str = "conduit.resource/network/application-connection@1";
pub const DNS_RESOLVE_AUTHORITY: &str = "conduit.authority/dns-resolve@1";
pub const NETWORK_CONNECT_AUTHORITY: &str = "conduit.authority/network-connect@1";

pub fn install_application_network_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in application_network_registered_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for (kind, inputs, outputs) in definitions() {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .map_err(|error| error.to_string())?;
        profile
            .insert(KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from(APPLICATION_NETWORK_REVISION),
                inputs,
                outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn application_network_std_offers() -> Vec<CapabilityOffer> {
    definitions()
        .into_iter()
        .map(|(kind, inputs, outputs)| {
            let (operation, resource, authority) = match kind {
                DNS_RESOLVE_KIND => (
                    DNS_RESOLVE_OPERATION,
                    Some(DNS_RESOLVER_RESOURCE),
                    Some(DNS_RESOLVE_AUTHORITY),
                ),
                NETWORK_CONNECT_KIND => (
                    NETWORK_CONNECT_OPERATION,
                    Some(NETWORK_CONNECTION_RESOURCE),
                    Some(NETWORK_CONNECT_AUTHORITY),
                ),
                DNS_FIXTURE_KIND => (DNS_FIXTURE_OPERATION, None, None),
                ENDPOINT_FIXTURE_KIND => (ENDPOINT_FIXTURE_OPERATION, None, None),
                _ => unreachable!("reviewed network catalog kind"),
            };
            offer(kind, operation, inputs, outputs, resource, authority)
        })
        .collect()
}

fn definitions() -> Vec<(&'static str, Vec<PortDescriptor>, Vec<PortDescriptor>)> {
    vec![
        (
            DNS_FIXTURE_KIND,
            vec![],
            vec![port("query", &dns_query_type(), PortDirection::Output)],
        ),
        (
            DNS_RESOLVE_KIND,
            vec![port("query", &dns_query_type(), PortDirection::Input)],
            vec![port("result", &dns_result_type(), PortDirection::Output)],
        ),
        (
            ENDPOINT_FIXTURE_KIND,
            vec![],
            vec![port(
                "endpoint",
                &network_endpoint_type(),
                PortDirection::Output,
            )],
        ),
        (
            NETWORK_CONNECT_KIND,
            vec![port(
                "endpoint",
                &network_endpoint_type(),
                PortDirection::Input,
            )],
            vec![port(
                "state",
                &network_connection_state_type(),
                PortDirection::Output,
            )],
        ),
    ]
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed application network profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn offer(
    kind: &str,
    operation: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    resource: Option<&str>,
    authority: Option<&str>,
) -> CapabilityOffer {
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(operation),
        target_kind: Some(kind_id(kind)),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(APPLICATION_NETWORK_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(APPLICATION_NETWORK_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(APPLICATION_NETWORK_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![operation.clone()],
        resource_requirements: resource
            .map(|class| resource_requirement(class, 1))
            .into_iter()
            .collect(),
        authority_requirements: authority
            .map(|contract| AuthorityRequirement {
                contract_id: AuthorityContractId::from(contract),
                host_operation_contract_id: operation.contract_id,
                subject_kind: kind_id(kind),
            })
            .into_iter()
            .collect(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
