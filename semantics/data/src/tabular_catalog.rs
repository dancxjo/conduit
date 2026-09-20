//! Canonical Form catalog for tabular Info.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, SemanticCapabilityContract, StructuredInfoType,
};
use conduit_form::{KindDefinition, KindSignature};

use crate::{
    tabular_person_row_type, tabular_query_outcome_type, tabular_query_result_type,
    tabular_row_slot_type, tabular_rows_four_type, tabular_schema_type, tabular_selected_text_type,
    TABULAR_PERSON_ROW_TYPE, TABULAR_QUERY_OUTCOME_TYPE, TABULAR_QUERY_RESULT_TYPE,
    TABULAR_ROWS_FOUR_TYPE, TABULAR_ROW_SLOT_TYPE, TABULAR_SCHEMA_TYPE, TABULAR_SELECTED_TEXT_TYPE,
};

pub const TABULAR_PROVIDER_KIND: &str = "tabular/person-query-four";
pub const TABULAR_FILTER_KIND: &str = "tabular/filter-active-four";
pub const TABULAR_REVISION: &str = "conduit.std/tabular-query@1";

pub fn tabular_semantic_contracts() -> Vec<SemanticCapabilityContract> {
    let result = tabular_query_result_type();
    [
        (
            TABULAR_PROVIDER_KIND,
            vec![],
            vec![port("result", &result, PortDirection::Output)],
        ),
        (
            TABULAR_FILTER_KIND,
            vec![port("result", &result, PortDirection::Input)],
            vec![port("result", &result, PortDirection::Output)],
        ),
    ]
    .into_iter()
    .map(|(kind, inputs, outputs)| SemanticCapabilityContract {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(TABULAR_REVISION),
        inputs,
        outputs,
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    })
    .collect()
}

pub fn install_tabular_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in tabular_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    for contract in tabular_semantic_contracts() {
        startup
            .insert(KindSignature {
                kind: contract.kind_id.as_str().to_string(),
                startup_parameters: vec![],
            })
            .map_err(|error| error.to_string())?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
