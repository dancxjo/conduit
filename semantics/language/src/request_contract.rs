//! Shared native startup seam for semantic owners requiring Language realization.
use crate::LanguageRequest;
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{FrontStartupParameter, KindConfigurationField, KindConfigurationRule};
use conduit_plot::{StartupCatalog, StartupParameterSignature};

pub fn language_request_signature() -> StartupParameterSignature {
    StartupParameterSignature {
        name: "language-request".into(),
        value_type: "LanguageRequest".into(),
        default: None,
    }
}
pub fn language_request_parameter() -> FrontStartupParameter {
    FrontStartupParameter {
        name: "language-request".into(),
        value_type: crate::language_request_profile(),
        has_default: false,
    }
}
pub fn language_request_field() -> KindConfigurationField {
    // A finite checker placeholder, never an authored default or language fallback.
    let request = crate::LanguageRequest::new(
        crate::LanguageId::new("request/placeholder".into()).expect("finite placeholder"),
        None,
        crate::LanguageVarietyPolicy::LanguageSufficient,
    )
    .expect("finite request");
    KindConfigurationField {
        key: "language-request".into(),
        default_value: crate::language_request_configuration(request).expect("finite request"),
        rule: KindConfigurationRule::Structured {
            profile: crate::language_request_profile(),
        },
    }
}
pub fn language_requirement_laws() -> Vec<conduit_core::KindSemanticLaw> {
    vec![conduit_core::KindSemanticLaw::RealizationRequirement {
        property_profile: crate::language_coverage_profile(),
        configuration_key: "language-request".into(),
    }]
}

/// Install the exact owner schema; an incompatible namesake is not accepted.
pub fn install_language_request_type(startup: &mut StartupCatalog) -> Result<(), String> {
    let expected = LanguageRequest::semantic_type().expect("checked Language Type");
    startup.ensure_structured_type("LanguageRequest", expected)
}

/// Render only exact typed semantic startup facts. Host source paths and private
/// mapping names remain outside the authored Plot.
pub fn language_request_literal(request: &LanguageRequest) -> alloc::string::String {
    let quote = conduit_plot::text_startup_literal;
    let variety = request.variety().as_ref().map_or_else(
        || "none(empty)".into(),
        |variety| {
            alloc::format!(
                "some({{ identity: {}, language: {} }})",
                quote(variety.identity().get()),
                quote(variety.language().get())
            )
        },
    );
    let policy = match request.variety_policy() {
        crate::LanguageVarietyPolicy::LanguageSufficient => "language_sufficient(empty)",
        crate::LanguageVarietyPolicy::ExactVariety => "exact_variety(empty)",
    };
    alloc::format!(
        "{{ language: {}, variety: {variety}, variety_policy: {policy} }}",
        quote(request.language().get())
    )
}

#[cfg(test)]
mod literal_tests {
    use super::*;
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
        StartupCatalog,
    };

    #[test]
    fn request_literal_roundtrips_exact_escaped_language_and_variety_identities() {
        let language = crate::LanguageId::new("language/\"雪\\".into()).unwrap();
        let variety = crate::LanguageVariety::new(
            crate::VarietyId::new("variety/\"雪\\".into()).unwrap(),
            language.clone(),
        )
        .unwrap();
        let request = LanguageRequest::new(
            language,
            Some(variety),
            crate::LanguageVarietyPolicy::ExactVariety,
        )
        .unwrap();
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        crate::install_linguistics_catalogs(&mut startup, &mut profiles).unwrap();
        let source = alloc::format!(
            "plot proof {{\n    annotate: language/annotate-four(language-request = {})\n}}\n",
            language_request_literal(&request)
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded = expand_canonical_plot(&checked, "proof", &profiles).unwrap();
        assert_eq!(
            expanded.gears[0]
                .configuration
                .iter()
                .find(|entry| entry.key == "language-request")
                .unwrap()
                .value,
            crate::language_request_configuration(request).unwrap()
        );
    }
}
