use conduit_core::{
    BaseImplementationId, BootId, CapabilityOffer, HostAdvertisement, HostId, HostProfileId,
    Observation, ObservationKind, OfferGeneration, PortTemporal, SignId, StructuredInfoType,
    StructuredInfoValue, ValuePayload, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    structured_selector_definition, ProfileCatalog, StartupCatalog,
};
use std::collections::BTreeMap;

struct Specimen {
    form_name: &'static str,
    type_name: &'static str,
    value_type: StructuredInfoType,
    default_value: StructuredInfoValue,
    literal: &'static str,
    field: &'static str,
    selected_type: StructuredInfoType,
}

fn specimens() -> [Specimen; 5] {
    [
        Specimen {
            form_name: "geometry-region",
            type_name: conduit_semantic_catalog::GEOMETRY_REGION_TYPE,
            value_type: conduit_semantic_catalog::geometry_region_type(),
            default_value: conduit_semantic_catalog::geometry_region_example(),
            literal: "{frame: \"image/content\", height: 480mm, width: 640mm, x: 12mm, y: 24mm}",
            field: "width",
            selected_type: leaf(conduit_core::QUANTITY_INFO_ID),
        },
        Specimen {
            form_name: "robotics-range",
            type_name: conduit_semantic_catalog::ROBOTICS_RANGE_TYPE,
            value_type: conduit_semantic_catalog::robotics_range_sample_type(),
            default_value: conduit_semantic_catalog::robotics_range_sample_example(),
            literal: "{distance: 850mm, frame: \"sensor/forward\", uncertainty: 5mm}",
            field: "distance",
            selected_type: leaf(conduit_core::QUANTITY_INFO_ID),
        },
        Specimen {
            form_name: "language-annotation",
            type_name: conduit_semantic_catalog::LANGUAGE_ANNOTATION_TYPE,
            value_type: conduit_semantic_catalog::language_annotation_type(),
            default_value: conduit_semantic_catalog::language_annotation_example(),
            literal: "{end: 11, label: \"noun-phrase\", start: 0, tokens: [\"bright\", \"star\"]}",
            field: "label",
            selected_type: leaf("value/text@1"),
        },
        Specimen {
            form_name: "message-envelope",
            type_name: conduit_semantic_catalog::MESSAGE_ENVELOPE_TYPE,
            value_type: conduit_semantic_catalog::message_envelope_type(),
            default_value: conduit_semantic_catalog::message_envelope_example(),
            literal:
                "{message_id: \"message/7\", state: delivered(true), subject: \"lesson/feedback\"}",
            field: "subject",
            selected_type: leaf("value/text@1"),
        },
        Specimen {
            form_name: "education-feedback",
            type_name: conduit_semantic_catalog::EDUCATION_FEEDBACK_TYPE,
            value_type: conduit_semantic_catalog::education_feedback_type(),
            default_value: conduit_semantic_catalog::education_feedback_example(),
            literal: "{outcome: passed(true), prompt_id: \"question/3\", score: 88%}",
            field: "score",
            selected_type: leaf(conduit_core::QUANTITY_INFO_ID),
        },
    ]
}

#[test]
fn five_unrelated_forms_use_exact_structured_values_and_the_same_selector_substrate() {
    let mut selector_kinds = Vec::new();
    for specimen in specimens() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        conduit_semantic_catalog::install_structured_value_catalogs(
            specimen.type_name,
            &specimen.value_type,
            &specimen.default_value,
            &mut startup,
            &mut profile,
        )
        .unwrap();
        startup
            .insert_structured_type("SelectedValue", specimen.selected_type.clone())
            .unwrap();
        let source = format!(
            "form {} (\n    selected: SelectedValue >\n) {{\n value: structured-info/literal(value = {})\n value > project({}.{}) > selected\n}}\n",
            specimen.form_name,
            specimen.literal,
            specimen.type_name,
            specimen.field
        );
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        let conduit_form::CheckedCordStage::StructuredSelector { selector, .. } =
            &checked.forms[0].cords[0].stages[1]
        else {
            panic!("{} did not check one selector", specimen.form_name)
        };
        profile
            .insert(structured_selector_definition(
                selector,
                PortTemporal::Value,
            ))
            .unwrap();
        let authored =
            expand_canonical_form_for_authoring(&checked, specimen.form_name, &profile).unwrap();
        assert_eq!(authored.expanded.gears.len(), 2);
        assert_eq!(authored.output_bindings.len(), 1);
        let selector_offer = structured_selector_proof_offer(selector, PortTemporal::Value);
        selector_kinds.push(selector_offer.kind_id.clone());
        let host = host(vec![
            structured_literal_proof_offer(specimen.type_name, &specimen.value_type),
            selector_offer,
        ]);
        let placements = conduit_planner::default_expanded_placements(
            &authored.expanded,
            core::slice::from_ref(&host),
        )
        .unwrap();
        conduit_planner::plan_expanded_canonical_with_options(
            &authored.expanded,
            &[host],
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
    }
    selector_kinds.sort();
    selector_kinds.dedup();
    assert_eq!(selector_kinds.len(), 5);
}

fn structured_literal_proof_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
) -> CapabilityOffer {
    let contract = conduit_semantic_catalog::structured_literal_contract(type_name, value_type);
    CapabilityOffer {
        startup_parameters: contract.startup_parameters,
        shorthand: None,
        capability_id: "proof/structured-literal".into(),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: "proof/structured-literal".into(),
            implementation_id: "proof/structured-literal".into(),
            artifact_id: "proof/structured-literal".into(),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn structured_selector_proof_offer(
    selector: &conduit_core::StructuredSelector,
    temporal: PortTemporal,
) -> CapabilityOffer {
    let contract = conduit_semantic_catalog::structured_selector_contract(selector, temporal);
    CapabilityOffer {
        startup_parameters: contract.startup_parameters,
        shorthand: contract.shorthand,
        capability_id: "proof/structured-selector".into(),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: "proof/structured-selector".into(),
            implementation_id: "proof/structured-selector".into(),
            artifact_id: "proof/structured-selector".into(),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[test]
fn one_typed_presentation_path_inspects_all_five_domains_without_leaf_text() {
    for (sequence, specimen) in specimens().into_iter().enumerate() {
        let observation = Observation {
            sign_id: SignId::from(format!("sign/{sequence}")),
            active_play_id: None,
            presentation_id: None,
            host_id: HostId::from("host/diverse-structured-proof"),
            boot_id: BootId::from("boot/diverse-structured-proof"),
            plan_id: None,
            placement_id: None,
            connection_id: None,
            kind: ObservationKind::ValuePresented {
                value: ValuePayload {
                    value_kind: specimen.value_type.profile().unwrap().value_kind().clone(),
                    encoded: specimen.default_value.canonical_bytes().unwrap(),
                },
            },
        };
        let artifact = conduit_presentation::StructuredSignPresentation::from_sign(
            sequence as u64,
            &observation,
            &specimen.value_type,
        )
        .unwrap();
        assert_eq!(
            artifact.presentation.basis.sign_ids,
            vec![observation.sign_id]
        );
        assert!(artifact.presentation.text.is_empty());
        assert!(artifact.presentation.properties.iter().any(|property| {
            property.name == "leaf-content-redacted"
                && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
        }));
    }
}

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(conduit_core::kind_id(kind)).unwrap()
}

fn host(capabilities: Vec<CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/diverse-structured-proof"),
        boot_id: conduit_core::BootId::from("boot/diverse-structured-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/diverse-structured-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}
