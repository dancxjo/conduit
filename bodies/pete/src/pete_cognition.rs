//! Authored continuing interpretation; no Pete-private execution or scheduler.
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{ProfileCatalog, StartupCatalog};

pub const PETE_COGNITION_PLOT_SOURCE: &str =
    include_str!("../../../plots/pete-cognition/main.conduit");

pub const PETE_AUTOBIOGRAPHICAL_COGNITION_PLOT_SOURCE: &str =
    include_str!("../../../plots/pete-cognition/autobiographical.conduit");

pub const PETE_RESILIENT_COGNITION_PLOT_SOURCE: &str =
    include_str!("../../../plots/pete-cognition/resilient.conduit");

pub const PETE_SITUATED_COGNITION_PLOT_SOURCE: &str =
    include_str!("../../../plots/pete-cognition/situated.conduit");

pub fn install_pete_situated_cognition_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    install_pete_resilient_cognition_catalogs(startup, profile)?;
    conduit_semantic_catalog::install_experience_situation_catalogs(startup, profile)
}

pub fn install_pete_resilient_cognition_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    install_pete_autobiographical_cognition_catalogs(startup, profile)?;
    conduit_semantic_catalog::install_experience_autobiography_recovery_catalogs(startup, profile)?;
    startup.insert_value_kind_alias(
        "PeteModelLoss",
        conduit_core::kind_id(conduit_core::TERMINAL_INFO_ID),
    )
}

pub fn install_pete_autobiographical_cognition_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    install_pete_cognition_catalogs(startup, profile)?;
    conduit_semantic_catalog::install_experience_autobiography_catalogs(startup, profile)?;
    conduit_semantic_catalog::install_experience_autobiography_recall_catalogs(startup, profile)?;
    if startup.signature("state/latest").is_none() {
        startup.insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })?;
    }
    let initial = conduit_semantic_catalog::initial_experience_autobiography_state()
        .into_structured()
        .map_err(|error| format!("initial autobiography: {error:?}"))?;
    conduit_semantic_catalog::state_value::install_state_value_kind(
        "ExperienceAutobiographyState",
        initial.value_type(),
        &initial,
        startup,
        profile,
    )
}

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
    #[test]
    fn autobiographical_plot_uses_the_ordinary_initialized_state_feedback_boundary() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_pete_autobiographical_cognition_catalogs(&mut startup, &mut profile).unwrap();
        let parsed = parse_syntax_document(PETE_AUTOBIOGRAPHICAL_COGNITION_PLOT_SOURCE);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let checked = check_syntax_document(&parsed, &startup).unwrap();
        let authored = expand_canonical_plot_for_authoring(
            &checked,
            "pete-autobiographical-cognition",
            &profile,
        )
        .unwrap();
        assert_eq!(authored.expanded.gears.len(), 7);
        assert_eq!(authored.input_bindings.len(), 1);
        assert_eq!(authored.output_bindings.len(), 2);
        assert!(authored
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == conduit_core::STATE_VALUE_KIND));
    }
    #[test]
    fn resilient_plot_routes_typed_model_loss_into_explicit_autobiography_feedback() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_pete_resilient_cognition_catalogs(&mut startup, &mut profile).unwrap();
        let parsed = parse_syntax_document(PETE_RESILIENT_COGNITION_PLOT_SOURCE);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let checked = check_syntax_document(&parsed, &startup).unwrap();
        let authored =
            expand_canonical_plot_for_authoring(&checked, "pete-resilient-cognition", &profile)
                .unwrap();
        assert_eq!(authored.expanded.gears.len(), 7);
        assert_eq!(authored.output_bindings.len(), 3);
        assert!(authored
            .expanded
            .connections
            .iter()
            .any(|c| c.track == conduit_core::ConnectionTrack::AbnormalTerminal));
    }
    #[test]
    fn situated_plot_selects_explicit_time_before_memory_and_model_work() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_pete_situated_cognition_catalogs(&mut startup, &mut profile).unwrap();
        let parsed = parse_syntax_document(PETE_SITUATED_COGNITION_PLOT_SOURCE);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let checked = check_syntax_document(&parsed, &startup).unwrap();
        let authored =
            expand_canonical_plot_for_authoring(&checked, "pete-situated-cognition", &profile)
                .unwrap();
        assert_eq!(authored.expanded.gears.len(), 8);
        assert_eq!(authored.output_bindings.len(), 4);
    }
}
