//! Authoritative collapsed/open projection of one recursively realized Form gear.

use crate::prelude::*;

use conduit_core::{
    CheckedFace, CheckedFormId, ExpandedFormId, GearId, KindContractRevision, KindId,
    SourceDocumentId,
};
use conduit_form::{CheckedConnection, ExpandedCanonicalForm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveFormGearProjection {
    pub invocation_path: String,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub face: CheckedFace,
    pub open: bool,
    pub nested_gear_count: u16,
    pub boundary_connections: Vec<CheckedConnection>,
    pub visible_gears: Vec<GearId>,
    pub visible_connections: Vec<CheckedConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecursiveFormProjectionError {
    MissingRealizationBack,
    MissingExpandedGears,
    TooManyExpandedGears,
}

/// Projects exact expansion truth already sealed into the expanded Form.
/// Opening changes visibility only; the face, invocation identity, caller
/// boundary connections, and every realization identity remain unchanged.
pub fn project_recursive_form_gear(
    form: &ExpandedCanonicalForm,
    invocation_path: &str,
    face: CheckedFace,
    open: bool,
) -> Result<RecursiveFormGearProjection, RecursiveFormProjectionError> {
    let back = form
        .realization_backs
        .iter()
        .find(|back| back.invocation_path == invocation_path)
        .ok_or(RecursiveFormProjectionError::MissingRealizationBack)?;
    let prefix = format!("{invocation_path}/");
    let nested_gears = form
        .gears
        .iter()
        .filter(|gear| gear.gear_id.as_str().starts_with(&prefix))
        .map(|gear| gear.gear_id.clone())
        .collect::<Vec<_>>();
    if nested_gears.is_empty() {
        return Err(RecursiveFormProjectionError::MissingExpandedGears);
    }
    let nested_gear_count = u16::try_from(nested_gears.len())
        .map_err(|_| RecursiveFormProjectionError::TooManyExpandedGears)?;
    let is_nested = |gear: &GearId| gear.as_str().starts_with(&prefix);
    let boundary_connections = form
        .connections
        .iter()
        .filter(|connection| {
            is_nested(&connection.source_gear_id) != is_nested(&connection.sink_gear_id)
        })
        .cloned()
        .collect();
    let visible_connections = if open {
        form.connections
            .iter()
            .filter(|connection| {
                is_nested(&connection.source_gear_id) && is_nested(&connection.sink_gear_id)
            })
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    let visible_gears = if open { nested_gears } else { Vec::new() };

    Ok(RecursiveFormGearProjection {
        invocation_path: back.invocation_path.clone(),
        kind_id: back.kind_id.clone(),
        kind_contract_revision: back.kind_contract_revision.clone(),
        source_document_id: back.source_document_id.clone(),
        checked_form_id: back.checked_form_id.clone(),
        expanded_form_id: form.expanded_form_id.clone(),
        face,
        open,
        nested_gear_count,
        boundary_connections,
        visible_gears,
        visible_connections,
    })
}
