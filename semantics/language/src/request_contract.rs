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
    match startup.structured_type("LanguageRequest") {
        Some(actual) if actual == &expected => Ok(()),
        Some(_) => Err("LanguageRequest differs from the Language-owned native schema".into()),
        None => startup.insert_structured_type("LanguageRequest", expected),
    }
}
