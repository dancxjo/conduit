//! Exact domain-owned Back coverage. All work is bounded preparation, not trial execution.
use crate::{LanguageCoverage, LanguageRequest, LanguageVarietyPolicy};
use conduit_core::{ConfigurationValue, KindId, StructuredConfigurationValue};
use conduit_plot::rust_binding::NativeRustBinding;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageCoverageRefusal {
    Undeclared,
    Language,
    Variety,
    MissingVariety,
    VarietyLanguage,
    MalformedDeclaration,
    Bound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageCoverageUse {
    LanguageSufficient,
    ExactVariety,
}

pub fn language_coverage_profile() -> KindId {
    LanguageCoverage::semantic_type()
        .expect("checked coverage Type")
        .profile()
        .expect("finite coverage profile")
        .value_kind()
        .clone()
}

pub fn language_request_profile() -> KindId {
    LanguageRequest::semantic_type()
        .expect("checked request Type")
        .profile()
        .expect("finite request profile")
        .value_kind()
        .clone()
}

pub fn validate_language_request(request: &LanguageRequest) -> Result<(), LanguageCoverageRefusal> {
    if request
        .variety()
        .as_ref()
        .is_some_and(|v| v.language() != request.language())
    {
        return Err(LanguageCoverageRefusal::VarietyLanguage);
    }
    if *request.variety_policy() == LanguageVarietyPolicy::ExactVariety
        && request.variety().is_none()
    {
        return Err(LanguageCoverageRefusal::MissingVariety);
    }
    Ok(())
}

pub fn validate_language_coverage(
    coverage: &LanguageCoverage,
) -> Result<(), LanguageCoverageRefusal> {
    let languages = coverage.languages().as_slice();
    let varieties = coverage.varieties().as_slice();
    for (i, language) in languages.iter().enumerate() {
        if languages[..i].contains(language) {
            return Err(LanguageCoverageRefusal::MalformedDeclaration);
        }
    }
    for (i, variety) in varieties.iter().enumerate() {
        if !languages.contains(variety.language())
            || varieties[..i]
                .iter()
                .any(|v| v.identity() == variety.identity())
        {
            return Err(LanguageCoverageRefusal::MalformedDeclaration);
        }
    }
    let mappings = coverage.mappings().as_slice();
    for (i, row) in mappings.iter().enumerate() {
        if mappings[..i].iter().any(|r| r.external() == row.external())
            || !languages.contains(row.language())
            || row
                .variety()
                .as_ref()
                .is_some_and(|v| v.language() != row.language() || !varieties.contains(v))
        {
            return Err(LanguageCoverageRefusal::MalformedDeclaration);
        }
    }
    Ok(())
}

pub fn admit_language_coverage(
    request: &LanguageRequest,
    coverage: Option<&LanguageCoverage>,
) -> Result<LanguageCoverageUse, LanguageCoverageRefusal> {
    validate_language_request(request)?;
    let coverage = coverage.ok_or(LanguageCoverageRefusal::Undeclared)?;
    validate_language_coverage(coverage)?;
    if coverage.languages().is_empty() {
        return Err(LanguageCoverageRefusal::Undeclared);
    }
    if !coverage.languages().as_slice().contains(request.language()) {
        return Err(LanguageCoverageRefusal::Language);
    }
    let exact = *request.variety_policy() == LanguageVarietyPolicy::ExactVariety
        || *coverage.variety_sensitive();
    if exact {
        let variety = request
            .variety()
            .as_ref()
            .ok_or(LanguageCoverageRefusal::MissingVariety)?;
        if !coverage.varieties().as_slice().contains(variety) {
            return Err(LanguageCoverageRefusal::Variety);
        }
        Ok(LanguageCoverageUse::ExactVariety)
    } else {
        Ok(LanguageCoverageUse::LanguageSufficient)
    }
}

pub fn language_coverage_property(
    coverage: LanguageCoverage,
) -> Result<StructuredConfigurationValue, LanguageCoverageRefusal> {
    validate_language_coverage(&coverage)?;
    let bytes = coverage
        .encode()
        .map_err(|_| LanguageCoverageRefusal::MalformedDeclaration)?;
    if bytes.len() > conduit_core::MAXIMUM_REALIZATION_PROPERTY_BYTES {
        return Err(LanguageCoverageRefusal::Bound);
    }
    StructuredConfigurationValue::new(language_coverage_profile(), bytes)
        .ok_or(LanguageCoverageRefusal::MalformedDeclaration)
}

pub fn language_request_configuration(
    request: LanguageRequest,
) -> Result<ConfigurationValue, LanguageCoverageRefusal> {
    validate_language_request(&request)?;
    let bytes = request
        .encode()
        .map_err(|_| LanguageCoverageRefusal::MalformedDeclaration)?;
    StructuredConfigurationValue::new(language_request_profile(), bytes)
        .map(ConfigurationValue::Structured)
        .ok_or(LanguageCoverageRefusal::MalformedDeclaration)
}

/// Native request and declaration schemas, separate from shared linguistic identities.
pub fn realization_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageRequest",
            LanguageRequest::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageCoverage",
            LanguageCoverage::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageVarietyPolicy",
            LanguageVarietyPolicy::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageMappingDeclaration",
            crate::LanguageMappingDeclaration::semantic_type().expect("checked Language Type")
        ),
    ]
}

/// Extract the finite explicit table of this declaration for #4952 projection.
/// Callers retain the selected Back/artifact and coverage revision as its basis.
pub fn language_coverage_mapping_rows(
    coverage: &LanguageCoverage,
) -> Result<alloc::vec::Vec<crate::LanguageMappingRow>, LanguageCoverageRefusal> {
    validate_language_coverage(coverage)?;
    coverage
        .mappings()
        .as_slice()
        .iter()
        .map(|row| {
            Ok(crate::LanguageMappingRow {
                external: row.external().clone(),
                target: crate::LanguageSelection::new(
                    row.language().clone(),
                    row.variety().clone(),
                )
                .map_err(|_| LanguageCoverageRefusal::MalformedDeclaration)?,
            })
        })
        .collect()
}

/// Domain-owned request extraction from exact semantic input schemas.
/// Material names its own Language; no operation or human-language name is matched.
pub fn language_configuration_request(
    value: &StructuredConfigurationValue,
) -> Result<LanguageRequest, LanguageCoverageRefusal> {
    if value.profile() == &language_request_profile() {
        LanguageRequest::decode(value.canonical_value())
            .map_err(|_| LanguageCoverageRefusal::MalformedDeclaration)
    } else if value.profile() == &crate::language_text_profile() {
        let material = crate::LanguageText::decode(value.canonical_value())
            .map_err(|_| LanguageCoverageRefusal::MalformedDeclaration)?;
        Ok(crate::language_material_request(&material))
    } else {
        Err(LanguageCoverageRefusal::MalformedDeclaration)
    }
}
