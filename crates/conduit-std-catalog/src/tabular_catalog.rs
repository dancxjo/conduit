//! Canonical Form catalog and finite hosted offers for tabular Info.

use alloc::{format, string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    tabular_person_row_type, tabular_query_outcome_type, tabular_query_result_type,
    tabular_row_slot_type, tabular_rows_four_type, tabular_schema_type,
    tabular_selected_text_type, TABULAR_PERSON_ROW_TYPE, TABULAR_QUERY_OUTCOME_TYPE,
    TABULAR_QUERY_RESULT_TYPE, TABULAR_ROWS_FOUR_TYPE, TABULAR_ROW_SLOT_TYPE,
    TABULAR_SCHEMA_TYPE, TABULAR_SELECTED_TEXT_TYPE,
};

pub const TABULAR_PROVIDER_KIND: &str = "tabular/person-query-four";
pub const TABULAR_FILTER_KIND: &str = "tabular/filter-active-four";
pub const TABULAR_REVISION: &str = "conduit.std/tabular-query@1";
pub const TABULAR_PROFILE: &str = "std/tabular-kernel-hosted@1";
pub const TABULAR_ARTIFACT: &str = "conduit-std-host/tabular@1";
pub const TABULAR_HOST_OPERATION: &str = "conduit.host/tabular@1";

pub fn install_tabular_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in tabular_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for kind in [TABULAR_PROVIDER_KIND, TABULAR_FILTER_KIND] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    let result = tabular_query_result_type();
    profile
        .insert(KindDefinition {
            kind_id: kind_id(TABULAR_PROVIDER_KIND),
            kind_contract_revision: KindContractRevision::from(TABULAR_REVISION),
            inputs: vec![],
            outputs: vec![port("result", &result, PortDirection::Output)],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(TABULAR_FILTER_KIND),
            kind_contract_revision: KindContractRevision::from(TABULAR_REVISION),
            inputs: vec![port("result", &result, PortDirection::Input)],
            outputs: vec![port("result", &result, PortDirection::Output)],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

pub fn tabular_std_offers() -> Vec<CapabilityOffer> {
    let result = tabular_query_result_type();
    vec![
        offer(
            TABULAR_PROVIDER_KIND,
            vec![],
            vec![port("result", &result, PortDirection::Output)],
        ),
        offer(
            TABULAR_FILTER_KIND,
            vec![port("result", &result, PortDirection::Input)],
            vec![port("result", &result, PortDirection::Output)],
        ),
    ]
}

fn tabular_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (TABULAR_SCHEMA_TYPE, tabular_schema_type()),
        (TABULAR_PERSON_ROW_TYPE, tabular_person_row_type()),
        (TABULAR_ROW_SLOT_TYPE, tabular_row_slot_type()),
        (TABULAR_ROWS_FOUR_TYPE, tabular_rows_four_type()),
        (TABULAR_QUERY_RESULT_TYPE, tabular_query_result_type()),
        (TABULAR_QUERY_OUTCOME_TYPE, tabular_query_outcome_type()),
        (TABULAR_SELECTED_TEXT_TYPE, tabular_selected_text_type()),
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

fn offer(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(TABULAR_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TABULAR_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(TABULAR_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(TABULAR_HOST_OPERATION),
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
