//! Compatibility projection with hosted integration proof retained here.
pub use patchbay_graph::{
    project_recursive_form_gear, RecursiveFormGearProjection, RecursiveFormProjectionError,
};

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
