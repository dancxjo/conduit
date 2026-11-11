//! Authored continuing interpretation; no Pete-private execution or scheduler.
use conduit_plot::{ProfileCatalog, StartupCatalog};

pub const PETE_COGNITION_PLOT_SOURCE: &str =
    include_str!("../../../plots/pete-cognition/main.conduit");

pub fn install_pete_cognition_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    conduit_semantic_catalog::install_experience_evidence_catalogs(startup, profile)?;
    conduit_semantic_catalog::install_experience_integration_catalogs(startup, profile)?;
    conduit_semantic_catalog::install_experience_prompt_catalogs(startup, profile)?;
    conduit_ai::install_llm_semantic_catalog(startup, profile)?;
    startup.insert_value_kind_alias(
        "PeteCognitionResponse",
        conduit_core::kind_id(conduit_ai::GENERATED_RESULT_VALUE_KIND),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    };

    #[test]
    fn cognitive_plot_connects_explicit_experience_to_the_existing_bounded_model_fore() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_pete_cognition_catalogs(&mut startup, &mut profile).unwrap();
        let parsed = parse_syntax_document(PETE_COGNITION_PLOT_SOURCE);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let checked = check_syntax_document(&parsed, &startup).unwrap();
        let authored =
            expand_canonical_plot_for_authoring(&checked, "pete-cognition", &profile).unwrap();
        assert_eq!(authored.expanded.gears.len(), 4);
        assert_eq!(authored.input_bindings.len(), 1);
        assert_eq!(authored.output_bindings.len(), 1);
        let prompt = authored
            .expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::EXPERIENCE_PROMPT_KIND)
            .unwrap();
        let model = authored
            .expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == conduit_ai::LLM_GENERATE_FLOW_KIND)
            .unwrap();
        assert_eq!(prompt.outputs[0].value_kind, model.inputs[0].value_kind);
        assert_eq!(
            model.outputs[0].value_kind.as_str(),
            conduit_ai::GENERATED_RESULT_VALUE_KIND
        );
    }
}
