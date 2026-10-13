//! Construction-time names for data manifested through a Face.
use alloc::{string::ToString, vec, vec::Vec};

pub use conduit_core::{data_label_configuration, face_data_label, MAXIMUM_DATA_LABEL_BYTES};

#[cfg(feature = "plot-catalog")]
pub fn data_label_startup_signature() -> Vec<conduit_plot::StartupParameterSignature> {
    vec![conduit_plot::StartupParameterSignature {
        name: "label".to_string(),
        value_type: "Text".to_string(),
        default: None,
    }]
}

#[cfg(all(test, feature = "plot-catalog"))]
mod tests {
    use super::*;
    use conduit_plot::{check_syntax_document, expand_canonical_plot, parse_syntax_document};

    #[test]
    fn source_construction_requires_a_checked_data_name_and_retains_typed_quantity() {
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        crate::install_quantity_info_catalog(&mut startup, &mut profile).unwrap();
        assert!(check_syntax_document(
            &parse_syntax_document("plot main {\n show: face/quantity\n}\n"),
            &startup
        )
        .is_err());
        for value in ["\"\"", "\"   \"", "42", "\"Room\\nheat\""] {
            let source = alloc::format!("plot main {{\n show: face/quantity({value})\n}}\n");
            let parsed = parse_syntax_document(&source);
            match check_syntax_document(&parsed, &startup) {
                Err(_) => {}
                Ok(checked) => assert!(expand_canonical_plot(&checked, "main", &profile).is_err()),
            }
        }
        let source = "plot main {\n show: face/quantity(\"Room temperature\")\n}\n";
        let parsed = parse_syntax_document(source);
        let checked = check_syntax_document(&parsed, &startup).unwrap();
        let expanded = expand_canonical_plot(&checked, "main", &profile).unwrap();
        let gear = &expanded.gears[0];
        assert_eq!(
            face_data_label(&gear.configuration).unwrap(),
            "Room temperature"
        );
        assert_eq!(
            gear.inputs[0].value_kind,
            crate::wrapped_quantity_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        );
    }
}

/// Authored Face Gear contracts require the canonical construction label.
/// Call only with Gear Kind identities, never carrier value identities.
pub fn face_data_label_required(kind: &str) -> bool {
    kind.starts_with("face/")
}

#[cfg(all(test, feature = "plot-catalog"))]
mod all_face_construction_tests {
    use alloc::string::ToString;
    use conduit_plot::{check_syntax_document, parse_syntax_document, ProfileCatalog};
    #[test]
    fn every_registered_face_front_refuses_an_implicit_construction_name() {
        let mut kinds = crate::supported_nucleus_contracts_with_revisions()
            .into_iter()
            .map(|(contract, revision)| contract.into_semantic_contract(revision))
            .collect::<alloc::vec::Vec<_>>();
        kinds.extend(conduit_face::spoken_mask_kinds());
        kinds.push(conduit_face::component_mask_kind(4096));
        kinds.push(crate::quantity_face_semantic_contract());
        kinds.push(
            crate::structured_face_contract("Quantity", &crate::wrapped_quantity_type()).into(),
        );
        kinds.push(
            crate::scalar_value_face_contract()
                .into_semantic_contract(crate::VALUE_PRIMITIVE_CONTRACT_REVISION),
        );
        kinds.push(
            crate::bool_value_face_contract()
                .into_semantic_contract(crate::VALUE_PRIMITIVE_CONTRACT_REVISION),
        );
        kinds.push(conduit_face::show_resource_source_kind());
        kinds.push(conduit_face::resource_renderer_kind());
        kinds.push(
            crate::view_face_contract()
                .into_semantic_contract(crate::APPLICATION_CONTRACT_REVISION),
        );
        kinds.push(crate::rhythm_face_semantic_contract());
        kinds.push(crate::pulse_face_semantic_contract());
        kinds.push(crate::indicator_face_semantic_contract());
        kinds.push(crate::indicator_state_face_semantic_contract());
        kinds.push(crate::scalar_field_face_semantic_contract());
        for contract in crate::patchbay_face_contracts() {
            kinds.push(contract.into_semantic_contract(crate::PATCHBAY_FACE_REVISION));
        }
        for kind in kinds
            .into_iter()
            .filter(|kind| kind.kind_id.as_str().starts_with("face/"))
        {
            assert_eq!(
                kind.startup_parameters.first().unwrap().name,
                "label",
                "{}",
                kind.kind_id.as_str()
            );
            assert!(!kind.startup_parameters[0].has_default);
            let identity = kind.kind_id.as_str().to_string();
            let mut profile = ProfileCatalog::new();
            profile.insert_kind(kind).unwrap();
            let startup = profile.startup_catalog().unwrap();
            assert!(startup.signature(&identity).unwrap().startup_parameters[0]
                .default
                .is_none());
            let source = alloc::format!("plot main {{\n value: {identity}\n}}\n");
            let parsed = parse_syntax_document(&source);
            let refusal = check_syntax_document(&parsed, &startup).unwrap_err();
            assert!(
                refusal.message.contains("label"),
                "{identity}: {}",
                refusal.message
            );
        }
        for definition in [
            conduit_face::renderer_kind_projection(),
            conduit_face::face_interaction_kind_projection(),
            conduit_face::face_tee_kind_projection(),
            conduit_face::presenter_stage_kind_projection(),
            crate::garden_state_face_definition(),
        ] {
            let identity = definition.kind_id.as_str().to_string();
            let mut profile = ProfileCatalog::new();
            profile.insert(definition).unwrap();
            let startup = profile.startup_catalog().unwrap();
            let source = alloc::format!("plot main {{\n value: {identity}\n}}\n");
            let parsed = parse_syntax_document(&source);
            let refusal = check_syntax_document(&parsed, &startup).unwrap_err();
            assert!(
                refusal.message.contains("label"),
                "{identity}: {}",
                refusal.message
            );
        }
    }
}
