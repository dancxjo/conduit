//! Explicit artifact-bound Language declarations and prepared provider mappings.
use conduit_core::{ConfigurationValue, PlannedGear, StructuredConfigurationValue};
use conduit_language::{
    admit_language_coverage, language_coverage_profile, language_coverage_property,
    language_request_profile, validate_language_coverage, LanguageCoverage,
    LanguageCoverageRefusal, LanguageCoverageUse, LanguageRequest,
};
use conduit_plot::rust_binding::NativeRustBinding;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedLanguageRefusal {
    Coverage(LanguageCoverageRefusal),
    Configuration,
    Mapping,
    Artifact,
    DeclarationRead,
    DeclarationBound,
}

impl std::fmt::Display for HostedLanguageRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Host Language refusal: {self:?}")
    }
}
impl std::error::Error for HostedLanguageRefusal {}

impl HostedLanguageRefusal {
    // The Host Call retains distinct Language refusal classes in finite kernel
    // detail codes. The portable planner refusal remains the Language enum.
    pub(crate) const fn host_detail(self) -> u16 {
        match self {
            Self::Coverage(LanguageCoverageRefusal::Undeclared) => 32,
            Self::Coverage(LanguageCoverageRefusal::Language) => 33,
            Self::Coverage(LanguageCoverageRefusal::Variety) => 34,
            Self::Coverage(LanguageCoverageRefusal::MissingVariety) => 35,
            Self::Coverage(LanguageCoverageRefusal::VarietyLanguage) => 36,
            Self::Coverage(LanguageCoverageRefusal::MalformedDeclaration) => 37,
            Self::Coverage(LanguageCoverageRefusal::Bound) => 38,
            Self::Configuration => 39,
            Self::Mapping => 40,
            Self::Artifact => 41,
            Self::DeclarationRead => 42,
            Self::DeclarationBound => 43,
        }
    }
}

pub(crate) fn request(placement: &PlannedGear) -> Result<LanguageRequest, HostedLanguageRefusal> {
    let entry = placement
        .configuration
        .iter()
        .find(|entry| entry.key == "language-request")
        .ok_or(HostedLanguageRefusal::Configuration)?;
    let ConfigurationValue::Structured(value) = &entry.value else {
        return Err(HostedLanguageRefusal::Configuration);
    };
    if value.profile() != &language_request_profile() {
        return Err(HostedLanguageRefusal::Configuration);
    }
    LanguageRequest::decode(value.canonical_value())
        .map_err(|_| HostedLanguageRefusal::Configuration)
}

pub(crate) fn properties(coverage: Option<&LanguageCoverage>) -> Vec<StructuredConfigurationValue> {
    coverage
        .into_iter()
        .map(|coverage| {
            language_coverage_property(coverage.clone()).expect("validated provider declaration")
        })
        .collect()
}

pub(crate) fn admit(placement: &PlannedGear) -> Result<LanguageRequest, HostedLanguageRefusal> {
    let request = request(placement)?;
    let coverage = placement
        .realization_properties
        .iter()
        .find(|value| value.profile() == &language_coverage_profile())
        .map(|value| LanguageCoverage::decode(value.canonical_value()))
        .transpose()
        .map_err(|_| HostedLanguageRefusal::Configuration)?;
    admit_language_coverage(&request, coverage.as_ref())
        .map_err(HostedLanguageRefusal::Coverage)?;
    Ok(request)
}

pub(crate) fn coverage(placement: &PlannedGear) -> Result<LanguageCoverage, HostedLanguageRefusal> {
    let [value] = placement.realization_properties.as_slice() else {
        return Err(HostedLanguageRefusal::Configuration);
    };
    if value.profile() != &language_coverage_profile() {
        return Err(HostedLanguageRefusal::Configuration);
    }
    LanguageCoverage::decode(value.canonical_value())
        .map_err(|_| HostedLanguageRefusal::Configuration)
}

pub(crate) fn declare(
    artifact: &str,
    coverage: &LanguageCoverage,
) -> Result<(), HostedLanguageRefusal> {
    validate_language_coverage(coverage).map_err(HostedLanguageRefusal::Coverage)?;
    language_coverage_property(coverage.clone()).map_err(HostedLanguageRefusal::Coverage)?;
    if coverage.evidence() != artifact {
        return Err(HostedLanguageRefusal::Artifact);
    }
    for row in coverage.mappings().as_slice() {
        if row.external().contract() != artifact {
            return Err(HostedLanguageRefusal::Artifact);
        }
        let external = row.external().name().as_str();
        if external.len() > 64
            || !external
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
            || !external
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(HostedLanguageRefusal::Mapping);
        }
    }
    // A provider argument must have one explicit inverse mapping. Synonyms are
    // permitted by Language projection, but cannot choose an execution option.
    for (index, row) in coverage.mappings().as_slice().iter().enumerate() {
        if coverage.mappings().as_slice()[..index]
            .iter()
            .any(|other| other.language() == row.language() && other.variety() == row.variety())
        {
            return Err(HostedLanguageRefusal::Mapping);
        }
    }
    for language in coverage.languages().as_slice() {
        if !*coverage.variety_sensitive()
            && !coverage
                .mappings()
                .as_slice()
                .iter()
                .any(|row| row.language() == language && row.variety().is_none())
        {
            return Err(HostedLanguageRefusal::Mapping);
        }
    }
    for variety in coverage.varieties().as_slice() {
        if !coverage
            .mappings()
            .as_slice()
            .iter()
            .any(|row| row.variety().as_ref() == Some(variety))
        {
            return Err(HostedLanguageRefusal::Mapping);
        }
    }
    Ok(())
}

pub(crate) fn provider_language<'a>(
    coverage: Option<&'a LanguageCoverage>,
    request: &LanguageRequest,
) -> Result<&'a str, HostedLanguageRefusal> {
    let use_coverage =
        admit_language_coverage(request, coverage).map_err(HostedLanguageRefusal::Coverage)?;
    let coverage = coverage.ok_or(HostedLanguageRefusal::Coverage(
        LanguageCoverageRefusal::Undeclared,
    ))?;
    let mut rows = coverage.mappings().as_slice().iter().filter(|row| {
        row.language() == request.language()
            && match use_coverage {
                LanguageCoverageUse::LanguageSufficient => row.variety().is_none(),
                LanguageCoverageUse::ExactVariety => row.variety() == request.variety(),
            }
    });
    let row = rows.next().ok_or(HostedLanguageRefusal::Mapping)?;
    if rows.next().is_some() {
        return Err(HostedLanguageRefusal::Mapping);
    }
    Ok(row.external().name().as_str())
}

/// Read finite native declaration bytes during Host preparation, never during play.
pub fn read_language_coverage(
    path: &std::path::Path,
) -> Result<LanguageCoverage, HostedLanguageRefusal> {
    let coverage = read_native::<LanguageCoverage>(path)?;
    validate_language_coverage(&coverage).map_err(HostedLanguageRefusal::Coverage)?;
    Ok(coverage)
}

pub fn read_language_request(
    path: &std::path::Path,
) -> Result<LanguageRequest, HostedLanguageRefusal> {
    let request = read_native::<LanguageRequest>(path)?;
    conduit_language::validate_language_request(&request)
        .map_err(HostedLanguageRefusal::Coverage)?;
    Ok(request)
}

fn read_native<T: NativeRustBinding>(path: &std::path::Path) -> Result<T, HostedLanguageRefusal> {
    use std::io::Read;
    let maximum = conduit_core::MAXIMUM_REALIZATION_PROPERTY_BYTES;
    let file = std::fs::File::open(path).map_err(|_| HostedLanguageRefusal::DeclarationRead)?;
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| HostedLanguageRefusal::DeclarationRead)?;
    if bytes.len() > maximum {
        return Err(HostedLanguageRefusal::DeclarationBound);
    }
    T::decode(&bytes).map_err(|_| HostedLanguageRefusal::Configuration)
}

/// Render only exact typed semantic startup facts. Host source paths and private
/// mapping names remain outside the authored Plot.
pub fn language_request_literal(request: &LanguageRequest) -> String {
    let quote = |value: &str| serde_json::to_string(value).expect("native bounded text");
    let variety = request.variety().as_ref().map_or_else(
        || "none(\"\")".into(),
        |variety| {
            format!(
                "some({{ identity: {}, language: {} }})",
                quote(variety.identity().get()),
                quote(variety.language().get())
            )
        },
    );
    let policy = match request.variety_policy() {
        conduit_language::LanguageVarietyPolicy::LanguageSufficient => "language_sufficient(\"\")",
        conduit_language::LanguageVarietyPolicy::ExactVariety => "exact_variety(\"\")",
    };
    format!(
        "{{ language: {}, variety: {variety}, variety_policy: {policy} }}",
        quote(request.language().get())
    )
}

#[cfg(test)]
pub(crate) mod tests;
