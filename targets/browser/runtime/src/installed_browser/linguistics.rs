//! Browser installations for bounded linguistic structured Info.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserBack;
use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, Kind, PlannedGear,
    StructuredInfoValue, FACE_RESOURCE_CLASS, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{HostedValueStore, ValueStorage};
use conduit_plot::rust_binding::NativeRustBinding;

const ARTIFACT: &str = "conduit-browser-runtime/installed-linguistics@2";
const TOKENIZE_IMPLEMENTATION: &str = "browser/kernel-language-tokenize-four@2";
const ANNOTATE_IMPLEMENTATION: &str = "browser/kernel-language-annotate-four@2";
const FACE_IMPLEMENTATION: &str = "browser/face-structured-info@1";
const HOST_CALL: &str = "conduit.host/browser-linguistics@1";

pub(super) static TOKENIZE: BrowserInstallation = BrowserInstallation {
    implementation_id: TOKENIZE_IMPLEMENTATION,
    offer: tokenize_offer,
    prepare: prepare_tokenize,
    perform: None,
};
pub(super) static ANNOTATE: BrowserInstallation = BrowserInstallation {
    implementation_id: ANNOTATE_IMPLEMENTATION,
    offer: annotate_offer,
    prepare: prepare_annotate,
    perform: Some(perform_annotate),
};
pub(super) static FACE: BrowserInstallation = BrowserInstallation {
    implementation_id: FACE_IMPLEMENTATION,
    offer: face_offer,
    prepare: prepare_face,
    perform: Some(perform_face),
};

pub(super) fn install_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    use conduit_plot::KindSignature;
    conduit_language::install_linguistics_catalogs(startup, profile)?;
    let contract = face_contract();
    startup.insert(KindSignature {
        kind: conduit_semantic_catalog::STRUCTURED_FACE_KIND.into(),
        startup_parameters: conduit_semantic_catalog::data_label_startup_signature(),
    })?;
    profile
        .insert_kind(contract.into())
        .map_err(|error| error.to_string())
}

fn tokenize_offer() -> CapabilityOffer {
    offer(
        conduit_language::tokenize_four_semantic_contract(),
        TOKENIZE_IMPLEMENTATION,
        Vec::new(),
    )
}

fn annotate_offer() -> CapabilityOffer {
    offer(
        conduit_language::annotate_four_semantic_contract(),
        ANNOTATE_IMPLEMENTATION,
        vec![operation(
            ANNOTATE_IMPLEMENTATION,
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        )],
    )
}

fn face_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        face_contract().into(),
        Back {
            capability_id: CapabilityId::from(FACE_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(FACE_IMPLEMENTATION),
            implementation_id: ImplementationId::from(FACE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![operation(FACE_IMPLEMENTATION, 0)],
            resource_requirements: vec![conduit_core::resource_requirement(FACE_RESOURCE_CLASS, 1)],
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn offer(
    contract: Kind,
    implementation: &str,
    host_calls: Vec<HostCallRequirement>,
) -> CapabilityOffer {
    let mut offered = BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls,
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build();
    offered.realization_properties =
        vec![
            conduit_language::language_coverage_property(english_coverage())
                .expect("bounded fixture coverage"),
        ];
    offered
}

fn operation(target: &str, output: u32) -> HostCallRequirement {
    HostCallRequirement {
        contract_id: HostCallContractId::from(HOST_CALL),
        target_kind: Some(kind_id(target)),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: output,
    }
}

fn face_contract() -> conduit_semantic_catalog::StructuredValueContract {
    conduit_semantic_catalog::structured_face_contract(
        conduit_language::ANNOTATION_BUNDLE_FOUR_TYPE,
        &conduit_language::annotation_bundle_four_type(),
    )
}

fn prepare_tokenize(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &tokenize_offer())?;
    let material = conduit_language::configured_language_material(&placement.configuration)
        .map_err(debug_error)?;
    admit_prepared_language(
        placement,
        &conduit_language::language_material_request(&material),
    )?;
    let value = conduit_language::tokenize_four(&material)
        .map_err(|error| format!("tokenize four: {error:?}"))?;
    let canonical = value
        .canonical_bytes()
        .map_err(|error| format!("encode linguistic tokens: {error:?}"))?;
    let stored = values.store(&canonical).map_err(debug_error)?;
    Ok(BrowserBack::source(stored))
}

fn prepare_annotate(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &annotate_offer())?;
    let request = conduit_language::configured_language_request(&placement.configuration)
        .map_err(debug_error)?;
    admit_prepared_language(placement, &request)?;
    Ok(BrowserBack::unary(
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        1,
    ))
}

fn admit_prepared_language(
    placement: &PlannedGear,
    request: &conduit_language::LanguageRequest,
) -> Result<(), String> {
    let [property] = placement.realization_properties.as_slice() else {
        return Err("prepared linguistic Back has no exact Language declaration".into());
    };
    let coverage = conduit_language::LanguageCoverage::decode(property.canonical_value())
        .map_err(debug_error)?;
    conduit_language::admit_language_coverage(request, Some(&coverage)).map_err(debug_error)?;
    Ok(())
}

fn prepare_face(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &face_offer())?;
    Ok(BrowserBack::face(
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        1,
    ))
}

fn perform_annotate(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let tokens = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode linguistic tokens: {error:?}"))?;
    let native = conduit_language::LinguisticTokensFour::from_structured(tokens.clone())
        .map_err(debug_error)?;
    let request = conduit_language::configured_language_request(&placement.configuration)
        .map_err(debug_error)?;
    conduit_language::validate_linguistic_request(&request, &native).map_err(debug_error)?;
    let annotated = conduit_language::annotate_with_unicode_library(&tokens)
        .map_err(|error| format!("annotate four: {error:?}"))?;
    Ok(BrowserHostResult {
        output: Some(
            annotated
                .canonical_bytes()
                .map_err(|error| format!("encode annotations: {error:?}"))?,
        ),
        manifestation: None,
    })
}

fn perform_face(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let value = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode structured face: {error:?}"))?;
    if value.value_type() != &conduit_language::annotation_bundle_four_type() {
        return Err("structured face has the wrong exact linguistic type".into());
    }
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            data_label: super::factory::manifestation_label(placement)?,
            kind_id: conduit_semantic_catalog::STRUCTURED_FACE_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

fn english_coverage() -> conduit_language::LanguageCoverage {
    use conduit_plot::rust_binding::BoundedSequence;
    conduit_language::LanguageCoverage::new(
        "repository/four-token-English-rule-fixture".into(),
        BoundedSequence::try_from_iter([conduit_language::LanguageId::new(
            "language/english".into(),
        )
        .expect("finite identity")])
        .expect("one language"),
        BoundedSequence::try_from_iter([]).expect("no mappings"),
        "four-token-rules@1".into(),
        BoundedSequence::try_from_iter([]).expect("undeclared varieties"),
        false,
    )
    .expect("bounded coverage")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planned_linguistics() -> Vec<PlannedGear> {
        let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
        let syntax = conduit_plot::parse_syntax_document(
            r#"plot preparation-language {
            tokenize: language/tokenize-four(material = { identity: "text/fixture", language: "language/english", revision: "source/1", text: "Bright stars shine." })
            annotate: language/annotate-four(language-request = { language: "language/english", variety: none(empty), variety_policy: language_sufficient(empty) })
            tokenize.tokens >> annotate.tokens
        }"#,
        );
        let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
        let plot = conduit_plot::expand_canonical_plot(&checked, "preparation-language", &catalog)
            .unwrap();
        let hosts = [crate::installed_browser::advertisement_for_face(
            "browser/fixture".into(),
            "boot/fixture".into(),
            crate::installed_browser::FaceProfile::Annotation,
        )];
        let placements = conduit_planner::default_expanded_placements(&plot, &hosts).unwrap();
        conduit_planner::plan_expanded_canonical_with_options(
            &plot,
            &hosts,
            &placements,
            &crate::installed_browser::local_bases(),
            conduit_planner::PlanningOptions {
                connection_bases: &std::collections::BTreeMap::new(),
                line_candidates: &std::collections::BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity:
                    crate::installed_browser::MAXIMUM_BROWSER_STORED_VALUE_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap()
        .fragments
        .remove(0)
        .placements
    }

    #[test]
    fn browser_tokenizer_refuses_source_language_changed_after_planning() {
        let mut placement = planned_linguistics()
            .into_iter()
            .find(|p| p.kind_id.as_str() == conduit_language::TOKENIZE_FOUR_KIND)
            .unwrap();
        let mut values = HostedValueStore::new(4, 32768, 131072).unwrap();
        assert!(prepare_tokenize(&placement, &mut values).is_ok());
        let material = conduit_language::LanguageText::new(
            conduit_language::LanguageTextId::new("text/fixture".into()).unwrap(),
            conduit_language::LanguageId::new("language/french".into()).unwrap(),
            conduit_language::LanguageTextRevisionId::new("source/1".into()).unwrap(),
            "Bright stars shine.".into(),
        )
        .unwrap();
        placement
            .configuration
            .iter_mut()
            .find(|e| e.key == "material")
            .unwrap()
            .value = conduit_language::language_material_configuration(material).unwrap();
        assert!(prepare_tokenize(&placement, &mut values).is_err());
    }

    #[test]
    fn browser_annotator_refuses_request_language_changed_after_planning() {
        let mut placement = planned_linguistics()
            .into_iter()
            .find(|p| p.kind_id.as_str() == conduit_language::ANNOTATE_FOUR_KIND)
            .unwrap();
        let mut values = HostedValueStore::new(4, 32768, 131072).unwrap();
        assert!(prepare_annotate(&placement, &mut values).is_ok());
        let request = conduit_language::LanguageRequest::new(
            conduit_language::LanguageId::new("language/french".into()).unwrap(),
            None,
            conduit_language::LanguageVarietyPolicy::LanguageSufficient,
        )
        .unwrap();
        placement
            .configuration
            .iter_mut()
            .find(|e| e.key == "language-request")
            .unwrap()
            .value = conduit_language::language_request_configuration(request).unwrap();
        assert!(prepare_annotate(&placement, &mut values).is_err());
    }

    #[test]
    fn browser_linguistics_preserves_each_exact_semantic_contract() {
        for (offer, semantic) in [
            (
                tokenize_offer(),
                conduit_language::tokenize_four_semantic_contract(),
            ),
            (
                annotate_offer(),
                conduit_language::annotate_four_semantic_contract(),
            ),
            (face_offer(), face_contract().into()),
        ] {
            assert_eq!(offer.startup_parameters, semantic.startup_parameters);
            assert_eq!(offer.shorthand, semantic.shorthand);
            assert_eq!(offer.kind_id, semantic.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                semantic.kind_contract_revision
            );
            assert_eq!(offer.inputs, semantic.inputs);
            assert_eq!(offer.outputs, semantic.outputs);
            assert_eq!(offer.limits, semantic.limits);
        }
    }
}
