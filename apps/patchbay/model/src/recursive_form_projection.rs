//! Authoritative collapsed/open projection of one recursively realized Form gear.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_and_open_change_visibility_without_rewriting_recursive_truth() {
        let proof = crate::patchbay_presenter_plans().unwrap();
        let (back, face) = proof
            .recursive_expanded
            .realization_backs
            .iter()
            .find_map(|back| {
                proof
                    .direct_host
                    .capabilities
                    .iter()
                    .find(|offer| {
                        offer.kind_id == back.kind_id
                            && offer.kind_contract_revision == back.kind_contract_revision
                    })
                    .map(|offer| (back.clone(), offer.checked_face()))
            })
            .unwrap();
        let collapsed = project_recursive_form_gear(
            &proof.recursive_expanded,
            &back.invocation_path,
            face.clone(),
            false,
        )
        .unwrap();
        let opened = project_recursive_form_gear(
            &proof.recursive_expanded,
            &back.invocation_path,
            face,
            true,
        )
        .unwrap();

        assert!(!collapsed.open);
        assert!(collapsed.visible_gears.is_empty());
        assert!(opened.open);
        assert_eq!(
            opened.visible_gears.len(),
            usize::from(opened.nested_gear_count)
        );
        assert_eq!(collapsed.invocation_path, opened.invocation_path);
        assert_eq!(collapsed.kind_id, opened.kind_id);
        assert_eq!(
            collapsed.kind_contract_revision,
            opened.kind_contract_revision
        );
        assert_eq!(collapsed.source_document_id, opened.source_document_id);
        assert_eq!(collapsed.checked_form_id, opened.checked_form_id);
        assert_eq!(collapsed.expanded_form_id, opened.expanded_form_id);
        assert_eq!(collapsed.face, opened.face);
        assert_eq!(collapsed.nested_gear_count, opened.nested_gear_count);
        assert_eq!(collapsed.boundary_connections, opened.boundary_connections);
    }

    #[test]
    fn projection_refuses_an_unselected_or_fabricated_back_path() {
        let proof = crate::patchbay_presenter_plans().unwrap();
        assert_eq!(
            project_recursive_form_gear(
                &proof.direct_expanded,
                "patchbay-capstone/canvas",
                proof.direct_host.capabilities[1].checked_face(),
                true,
            ),
            Err(RecursiveFormProjectionError::MissingRealizationBack)
        );
    }
}
