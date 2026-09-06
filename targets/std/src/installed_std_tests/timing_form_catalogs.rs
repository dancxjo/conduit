//! Catalog fixture shared by canonical timing Form conformance consumers.
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

pub(super) fn install_catalogs(startup: &mut StartupCatalog, profile: &mut ProfileCatalog) {
    conduit_semantic_catalog::install_generalized_input_catalogs(startup, profile).unwrap();
    conduit_semantic_catalog::install_timed_pattern_catalogs(startup, profile).unwrap();
    conduit_semantic_catalog::install_timed_button_attempt_catalogs(startup, profile).unwrap();
    conduit_semantic_catalog::install_sequence_normalization_catalogs(startup, profile).unwrap();
    conduit_semantic_catalog::install_template_storage_catalogs(startup, profile).unwrap();
    conduit_semantic_catalog::install_final_normalized_pattern_catalogs(startup, profile).unwrap();
    conduit_semantic_catalog::install_pattern_comparison_catalogs(startup, profile).unwrap();
    startup
        .insert(KindSignature {
            kind: conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    let presenter = conduit_semantic_catalog::structured_presentation_contract(
        conduit_semantic_catalog::PATTERN_COMPARISON_TYPE,
        &conduit_semantic_catalog::pattern_comparison_type(),
    );
    profile
        .insert(KindDefinition {
            kind_id: presenter.kind_id,
            kind_contract_revision: presenter.kind_contract_revision,
            inputs: presenter.inputs,
            outputs: presenter.outputs,
            configuration: Vec::new(),
        })
        .unwrap();
}
