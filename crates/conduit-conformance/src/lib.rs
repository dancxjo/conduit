use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};

pub const SUPPORTED_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub fixture_version: String,
    pub manifest_revision: u32,
    pub protocol_version: u32,
    pub deterministic_environment: DeterministicEnvironment,
    pub property_seeds: PropertySeeds,
    pub suites: Vec<Suite>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeterministicEnvironment {
    pub clock: FixtureClock,
    pub seed: u64,
    pub host_observations: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureClock {
    pub basis: String,
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertySeeds {
    pub bytes: Vec<String>,
    pub recursion_depths: Vec<u32>,
    pub discovery_orders: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub id: String,
    pub profile: String,
    pub requirement_ids: Vec<String>,
    pub artifacts: Vec<Artifact>,
    pub coverage: Coverage,
    pub reference_tests: Vec<ReferenceTest>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub positive: Vec<String>,
    pub negative: Vec<String>,
    pub boundary: Vec<String>,
    pub migration: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceTest {
    pub package: String,
    pub test: String,
}

#[derive(Debug, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub operation: String,
    pub requirement_ids: Vec<String>,
    pub default_rule: String,
    #[serde(default)]
    pub case_rules: BTreeMap<String, String>,
    #[serde(flatten)]
    pub format: ArtifactFormat,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "format", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ArtifactFormat {
    Tsv {
        case_columns: Vec<String>,
        expected_columns: Vec<String>,
    },
    Ndjson {
        case_fields: Vec<String>,
        expected_fields: Vec<String>,
    },
    JsonVectors {
        collection: String,
        case_fields: Vec<String>,
        expected_fields: Vec<String>,
    },
}

#[derive(Clone, Debug)]
pub struct LoadedCase {
    pub request_id: String,
    pub fixture: String,
    pub suite: String,
    pub profile: String,
    pub operation: String,
    pub requirement_ids: Vec<String>,
    pub environment: DeterministicEnvironment,
    pub input: Value,
    pub expected: Value,
}

#[derive(Debug)]
pub struct LoadedManifest {
    pub path: PathBuf,
    pub manifest: Manifest,
    pub cases: Vec<LoadedCase>,
}

#[derive(Debug)]
pub enum HarnessError {
    Io(io::Error),
    Json(serde_json::Error),
    Invalid(String),
    ReferenceFailed { package: String, test: String },
}

impl fmt::Display for HarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Invalid(message) => formatter.write_str(message),
            Self::ReferenceFailed { package, test } => {
                write!(formatter, "Rust reference test failed: {package}/{test}")
            }
        }
    }
}

impl std::error::Error for HarnessError {}

impl From<io::Error> for HarnessError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for HarnessError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub fn load_manifest(path: &Path) -> Result<LoadedManifest, HarnessError> {
    let path = path.canonicalize()?;
    let bytes = fs::read(&path)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    validate_manifest_header(&manifest)?;
    let base = path.parent().ok_or_else(|| {
        HarnessError::Invalid(format!("manifest path has no parent: {}", path.display()))
    })?;

    let mut suite_ids = BTreeSet::new();
    let mut artifact_ids = BTreeSet::new();
    let mut request_ids = BTreeSet::new();
    let mut cases = Vec::new();

    for suite in &manifest.suites {
        require_identifier("suite", &suite.id)?;
        if !suite_ids.insert(suite.id.as_str()) {
            return invalid(format!("duplicate suite id: {}", suite.id));
        }
        if suite.requirement_ids.is_empty() {
            return invalid(format!("suite {} has no requirement IDs", suite.id));
        }
        validate_coverage_shape(suite)?;

        let mut suite_cases = BTreeSet::new();
        for artifact in &suite.artifacts {
            require_identifier("artifact", &artifact.id)?;
            let qualified_artifact = format!("{}/{}", suite.id, artifact.id);
            if !artifact_ids.insert(qualified_artifact.clone()) {
                return invalid(format!("duplicate artifact id: {qualified_artifact}"));
            }
            if artifact.requirement_ids.is_empty() {
                return invalid(format!(
                    "artifact {qualified_artifact} has no requirement IDs"
                ));
            }
            for requirement in &artifact.requirement_ids {
                if !suite.requirement_ids.contains(requirement) {
                    return invalid(format!(
                        "artifact {qualified_artifact} uses undeclared requirement {requirement}"
                    ));
                }
            }
            if !artifact.requirement_ids.contains(&artifact.default_rule) {
                return invalid(format!(
                    "artifact {qualified_artifact} default rule {} is undeclared",
                    artifact.default_rule
                ));
            }
            for rule in artifact.case_rules.values() {
                if !artifact.requirement_ids.contains(rule) {
                    return invalid(format!(
                        "artifact {qualified_artifact} case rule {rule} is undeclared"
                    ));
                }
            }

            let artifact_path = resolve_artifact(base, &artifact.path)?;
            verify_digest(&artifact_path, &artifact.sha256)?;
            let artifact_cases = load_artifact(
                &manifest,
                suite,
                artifact,
                &artifact_path,
                &qualified_artifact,
            )?;
            if artifact_cases.is_empty() {
                return invalid(format!("artifact {qualified_artifact} has no cases"));
            }
            for case in artifact_cases {
                if !request_ids.insert(case.request_id.clone()) {
                    return invalid(format!("duplicate request id: {}", case.request_id));
                }
                suite_cases.insert(case.fixture.clone());
                cases.push(case);
            }
        }
        validate_coverage(suite, &suite_cases)?;
    }

    Ok(LoadedManifest {
        path,
        manifest,
        cases,
    })
}

fn validate_manifest_header(manifest: &Manifest) -> Result<(), HarnessError> {
    if manifest.protocol_version != SUPPORTED_PROTOCOL_VERSION {
        return invalid(format!(
            "unsupported protocol version {}; supported version is {}",
            manifest.protocol_version, SUPPORTED_PROTOCOL_VERSION
        ));
    }
    if manifest.fixture_version.is_empty() || manifest.manifest_revision == 0 {
        return invalid("fixture_version must be non-empty and manifest_revision must be positive");
    }
    if manifest.deterministic_environment.clock.basis.is_empty()
        || !manifest
            .deterministic_environment
            .host_observations
            .is_empty()
    {
        return invalid(
            "deterministic environment needs a named fixture clock and no discovered host observations",
        );
    }
    if manifest.property_seeds.bytes.is_empty()
        || manifest.property_seeds.recursion_depths.is_empty()
        || manifest.property_seeds.discovery_orders.len() < 2
    {
        return invalid("property seeds must cover bytes, recursion, and discovery ordering");
    }
    Ok(())
}

fn validate_coverage_shape(suite: &Suite) -> Result<(), HarnessError> {
    for (name, values) in [
        ("positive", &suite.coverage.positive),
        ("negative", &suite.coverage.negative),
        ("boundary", &suite.coverage.boundary),
        ("migration", &suite.coverage.migration),
    ] {
        if values.is_empty() {
            return invalid(format!("suite {} has no {name} coverage", suite.id));
        }
    }
    Ok(())
}

fn validate_coverage(suite: &Suite, cases: &BTreeSet<String>) -> Result<(), HarnessError> {
    for fixture in suite
        .coverage
        .positive
        .iter()
        .chain(&suite.coverage.negative)
        .chain(&suite.coverage.boundary)
        .chain(&suite.coverage.migration)
    {
        if !cases.contains(fixture) {
            return invalid(format!(
                "suite {} classifies unknown fixture {fixture}",
                suite.id
            ));
        }
    }
    Ok(())
}

fn resolve_artifact(base: &Path, relative: &str) -> Result<PathBuf, HarnessError> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return invalid(format!(
            "artifact path must be relative to the manifest directory: {}",
            relative.display()
        ));
    }
    let path = base.join(relative);
    let resolved = path.canonicalize()?;
    let conformance_root = base
        .parent()
        .ok_or_else(|| HarnessError::Invalid("manifest has no conformance root".to_owned()))?
        .canonicalize()?;
    if !resolved.starts_with(&conformance_root) {
        return invalid(format!(
            "artifact path escapes the conformance tree: {}",
            relative.display()
        ));
    }
    Ok(resolved)
}

fn verify_digest(path: &Path, expected: &str) -> Result<(), HarnessError> {
    let bytes = fs::read(path)?;
    let actual = format!("sha256:{:x}", Sha256::digest(bytes));
    if actual != expected {
        return invalid(format!(
            "{}: artifact digest differs: expected {expected}, actual {actual}",
            path.display()
        ));
    }
    Ok(())
}

fn load_artifact(
    manifest: &Manifest,
    suite: &Suite,
    artifact: &Artifact,
    path: &Path,
    qualified_artifact: &str,
) -> Result<Vec<LoadedCase>, HarnessError> {
    let cases = match &artifact.format {
        ArtifactFormat::Tsv {
            case_columns,
            expected_columns,
        } => load_tsv(
            manifest,
            suite,
            artifact,
            path,
            qualified_artifact,
            case_columns,
            expected_columns,
        ),
        ArtifactFormat::Ndjson {
            case_fields,
            expected_fields,
        } => load_ndjson(
            manifest,
            suite,
            artifact,
            path,
            qualified_artifact,
            case_fields,
            expected_fields,
        ),
        ArtifactFormat::JsonVectors {
            collection,
            case_fields,
            expected_fields,
        } => load_json_vectors(
            manifest,
            suite,
            artifact,
            path,
            qualified_artifact,
            collection,
            case_fields,
            expected_fields,
        ),
    }?;
    let loaded_ids: BTreeSet<&str> = cases
        .iter()
        .filter_map(|case| case.fixture.split_once('#').map(|(_, case)| case))
        .collect();
    for case in artifact.case_rules.keys() {
        if !loaded_ids.contains(case.as_str()) {
            return invalid(format!(
                "{qualified_artifact}: case rule names unknown case {case}"
            ));
        }
    }
    Ok(cases)
}

#[allow(clippy::too_many_arguments)]
fn load_tsv(
    manifest: &Manifest,
    suite: &Suite,
    artifact: &Artifact,
    path: &Path,
    qualified_artifact: &str,
    case_columns: &[String],
    expected_columns: &[String],
) -> Result<Vec<LoadedCase>, HarnessError> {
    let text = fs::read_to_string(path)?;
    let header = text
        .lines()
        .filter_map(|line| line.strip_prefix("# "))
        .find(|line| {
            let first = line.split('\t').next().unwrap_or_default();
            case_columns.first().is_some_and(|column| column == first)
        })
        .ok_or_else(|| HarnessError::Invalid(format!("{}: no TSV case header", path.display())))?;
    let columns: Vec<&str> = header.split('\t').collect();
    validate_columns(path, &columns, case_columns, expected_columns)?;

    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .enumerate()
        .map(|(row, line)| {
            let values: Vec<&str> = line.split('\t').collect();
            if values.len() != columns.len() {
                return invalid(format!(
                    "{}:{}: expected {} columns, found {}",
                    path.display(),
                    row + 1,
                    columns.len(),
                    values.len()
                ));
            }
            let record: Map<String, Value> = columns
                .iter()
                .zip(values)
                .map(|(name, value)| ((*name).to_owned(), Value::String(value.to_owned())))
                .collect();
            make_case(
                manifest,
                suite,
                artifact,
                qualified_artifact,
                &record,
                case_columns,
                expected_columns,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn load_ndjson(
    manifest: &Manifest,
    suite: &Suite,
    artifact: &Artifact,
    path: &Path,
    qualified_artifact: &str,
    case_fields: &[String],
    expected_fields: &[String],
) -> Result<Vec<LoadedCase>, HarnessError> {
    let text = fs::read_to_string(path)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(line_number, line)| {
            let record: Map<String, Value> = serde_json::from_str(line).map_err(|error| {
                HarnessError::Invalid(format!(
                    "{}:{}: invalid NDJSON: {error}",
                    path.display(),
                    line_number + 1
                ))
            })?;
            make_case(
                manifest,
                suite,
                artifact,
                qualified_artifact,
                &record,
                case_fields,
                expected_fields,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn load_json_vectors(
    manifest: &Manifest,
    suite: &Suite,
    artifact: &Artifact,
    path: &Path,
    qualified_artifact: &str,
    collection: &str,
    case_fields: &[String],
    expected_fields: &[String],
) -> Result<Vec<LoadedCase>, HarnessError> {
    let value: Value = serde_json::from_slice(&fs::read(path)?)?;
    let records = value
        .get(collection)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            HarnessError::Invalid(format!("{}: /{collection} is not an array", path.display()))
        })?;
    records
        .iter()
        .map(|record| {
            let record = record.as_object().ok_or_else(|| {
                HarnessError::Invalid(format!(
                    "{}: /{collection} member is not an object",
                    path.display()
                ))
            })?;
            make_case(
                manifest,
                suite,
                artifact,
                qualified_artifact,
                record,
                case_fields,
                expected_fields,
            )
        })
        .collect()
}

fn validate_columns(
    path: &Path,
    columns: &[&str],
    case_columns: &[String],
    expected_columns: &[String],
) -> Result<(), HarnessError> {
    let available: BTreeSet<&str> = columns.iter().copied().collect();
    for column in case_columns.iter().chain(expected_columns) {
        if !available.contains(column.as_str()) {
            return invalid(format!(
                "{}: declared column {column} is absent",
                path.display()
            ));
        }
    }
    Ok(())
}

fn make_case(
    manifest: &Manifest,
    suite: &Suite,
    artifact: &Artifact,
    qualified_artifact: &str,
    record: &Map<String, Value>,
    case_fields: &[String],
    expected_fields: &[String],
) -> Result<LoadedCase, HarnessError> {
    let case_id = select_case_id(record, case_fields)?;
    let fixture = format!("{}#{case_id}", artifact.id);
    let request_id = format!("{qualified_artifact}/{case_id}");
    let expected_names: BTreeSet<&str> = expected_fields.iter().map(String::as_str).collect();
    let mut input = Map::new();
    let mut expected = Map::new();
    for (name, value) in record {
        if expected_names.contains(name.as_str()) {
            expected.insert(name.clone(), value.clone());
        } else if !case_fields.contains(name) {
            input.insert(name.clone(), value.clone());
        }
    }
    if expected.len() != expected_fields.len() {
        return invalid(format!(
            "{request_id}: one or more expected fields are absent"
        ));
    }
    Ok(LoadedCase {
        request_id,
        fixture,
        suite: suite.id.clone(),
        profile: suite.profile.clone(),
        operation: artifact.operation.clone(),
        requirement_ids: vec![
            artifact
                .case_rules
                .get(&case_id)
                .unwrap_or(&artifact.default_rule)
                .clone(),
        ],
        environment: manifest.deterministic_environment.clone(),
        input: Value::Object(input),
        expected: Value::Object(expected),
    })
}

fn select_case_id(
    record: &Map<String, Value>,
    case_fields: &[String],
) -> Result<String, HarnessError> {
    if case_fields.is_empty() {
        return invalid("case field list is empty");
    }
    let mut parts = Vec::new();
    for field in case_fields {
        let value = record
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| HarnessError::Invalid(format!("case field {field} is not a string")))?;
        parts.push(value);
    }
    let id = parts.join("~");
    require_identifier("case", &id)?;
    Ok(id)
}

fn require_identifier(kind: &str, value: &str) -> Result<(), HarnessError> {
    if value.is_empty()
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || b"-_./~".contains(&byte)))
    {
        return invalid(format!("{kind} identifier is not portable: {value:?}"));
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, HarnessError> {
    Err(HarnessError::Invalid(message.into()))
}

#[derive(Serialize)]
struct Request<'a> {
    protocol_version: u32,
    fixture_version: &'a str,
    request_id: &'a str,
    suite: &'a str,
    fixture: &'a str,
    profile: &'a str,
    operation: &'a str,
    requirement_ids: &'a [String],
    result_fields: Vec<&'a str>,
    environment: &'a DeterministicEnvironment,
    input: &'a Value,
}

pub fn write_requests(loaded: &LoadedManifest, mut output: impl Write) -> Result<(), HarnessError> {
    for case in &loaded.cases {
        serde_json::to_writer(
            &mut output,
            &Request {
                protocol_version: loaded.manifest.protocol_version,
                fixture_version: &loaded.manifest.fixture_version,
                request_id: &case.request_id,
                suite: &case.suite,
                fixture: &case.fixture,
                profile: &case.profile,
                operation: &case.operation,
                requirement_ids: &case.requirement_ids,
                result_fields: case
                    .expected
                    .as_object()
                    .expect("loaded expected result is an object")
                    .keys()
                    .map(String::as_str)
                    .collect(),
                environment: &case.environment,
                input: &case.input,
            },
        )?;
        writeln!(output)?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImplementationResult {
    protocol_version: u32,
    fixture_version: String,
    request_id: String,
    status: ImplementationStatus,
    #[serde(default)]
    actual: Option<Value>,
    #[serde(default)]
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ImplementationStatus {
    Completed,
    Unsupported,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct Comparison<'a> {
    protocol_version: u32,
    fixture_version: &'a str,
    request_id: &'a str,
    fixture: &'a str,
    requirement_ids: &'a [String],
    status: ComparisonStatus,
    differences: Vec<Difference>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ComparisonStatus {
    Passed,
    Failed,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Difference {
    rule: String,
    path: String,
    expected: Value,
    actual: Value,
}

pub fn check_results(
    loaded: &LoadedManifest,
    input: impl BufRead,
    mut output: impl Write,
) -> Result<bool, HarnessError> {
    let mut submitted = BTreeMap::new();
    for (line_number, line) in input.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let result: ImplementationResult = serde_json::from_str(&line).map_err(|error| {
            HarnessError::Invalid(format!("result line {}: {error}", line_number + 1))
        })?;
        if submitted
            .insert(result.request_id.clone(), result)
            .is_some()
        {
            return invalid(format!("duplicate result at line {}", line_number + 1));
        }
    }

    let known: BTreeSet<&str> = loaded
        .cases
        .iter()
        .map(|case| case.request_id.as_str())
        .collect();
    for request_id in submitted.keys() {
        if !known.contains(request_id.as_str()) {
            return invalid(format!("result names unknown request {request_id}"));
        }
    }

    let mut passed = true;
    for case in &loaded.cases {
        let result = submitted.remove(&case.request_id);
        let rule = case
            .requirement_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "CND-CON-001".to_owned());
        let (status, differences) = match result {
            None => (
                ComparisonStatus::Failed,
                vec![Difference {
                    rule,
                    path: "/result".to_owned(),
                    expected: Value::String("one completed result".to_owned()),
                    actual: Value::String("missing".to_owned()),
                }],
            ),
            Some(result)
                if result.protocol_version != loaded.manifest.protocol_version
                    || result.fixture_version != loaded.manifest.fixture_version =>
            {
                (
                    ComparisonStatus::Failed,
                    vec![Difference {
                        rule,
                        path: "/protocol".to_owned(),
                        expected: json!({
                            "protocol_version": loaded.manifest.protocol_version,
                            "fixture_version": loaded.manifest.fixture_version,
                        }),
                        actual: json!({
                            "protocol_version": result.protocol_version,
                            "fixture_version": result.fixture_version,
                        }),
                    }],
                )
            }
            Some(result) => match result.status {
                ImplementationStatus::Unsupported => (
                    ComparisonStatus::Unsupported,
                    vec![Difference {
                        rule,
                        path: "/status".to_owned(),
                        expected: Value::String("completed".to_owned()),
                        actual: serde_json::to_value(result.diagnostics)?,
                    }],
                ),
                ImplementationStatus::Completed => {
                    let actual = result.actual.unwrap_or(Value::Null);
                    let differences = differences(&case.expected, &actual, &rule);
                    let status = if differences.is_empty() {
                        ComparisonStatus::Passed
                    } else {
                        ComparisonStatus::Failed
                    };
                    (status, differences)
                }
            },
        };
        if !matches!(status, ComparisonStatus::Passed) {
            passed = false;
        }
        serde_json::to_writer(
            &mut output,
            &Comparison {
                protocol_version: loaded.manifest.protocol_version,
                fixture_version: &loaded.manifest.fixture_version,
                request_id: &case.request_id,
                fixture: &case.fixture,
                requirement_ids: &case.requirement_ids,
                status,
                differences,
            },
        )?;
        writeln!(output)?;
    }
    Ok(passed)
}

fn differences(expected: &Value, actual: &Value, rule: &str) -> Vec<Difference> {
    let mut found = Vec::new();
    compare_value(expected, actual, "", rule, &mut found);
    found
}

fn compare_value(
    expected: &Value,
    actual: &Value,
    path: &str,
    rule: &str,
    found: &mut Vec<Difference>,
) {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            let keys: BTreeSet<&str> = expected
                .keys()
                .chain(actual.keys())
                .map(String::as_str)
                .collect();
            for key in keys {
                let child = format!("{path}/{}", json_pointer_escape(key));
                match (expected.get(key), actual.get(key)) {
                    (Some(expected), Some(actual)) => {
                        compare_value(expected, actual, &child, rule, found);
                    }
                    (expected, actual) => found.push(Difference {
                        rule: rule.to_owned(),
                        path: child,
                        expected: expected.cloned().unwrap_or(Value::Null),
                        actual: actual.cloned().unwrap_or(Value::Null),
                    }),
                }
            }
        }
        _ if expected == actual => {}
        _ => found.push(Difference {
            rule: rule.to_owned(),
            path: if path.is_empty() {
                "/".to_owned()
            } else {
                path.to_owned()
            },
            expected: expected.clone(),
            actual: actual.clone(),
        }),
    }
}

fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

pub fn run_reference(loaded: &LoadedManifest) -> Result<(), HarnessError> {
    verify_canonical_vectors(loaded)?;
    let repository = loaded
        .path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| HarnessError::Invalid("cannot locate repository root".to_owned()))?;
    let mut tests = BTreeSet::new();
    for suite in &loaded.manifest.suites {
        for test in &suite.reference_tests {
            tests.insert((test.package.as_str(), test.test.as_str()));
        }
    }
    for (package, test) in tests {
        eprintln!("reference {package}/{test}");
        let status = Command::new("cargo")
            .args(["test", "-p", package, "--test", test])
            .current_dir(repository)
            .status()?;
        if !status.success() {
            return Err(HarnessError::ReferenceFailed {
                package: package.to_owned(),
                test: test.to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_canonical_vectors(loaded: &LoadedManifest) -> Result<(), HarnessError> {
    for case in loaded
        .cases
        .iter()
        .filter(|case| case.operation == "canonical-descriptor-v1")
    {
        let kind = string_field(&case.input, "kind", &case.request_id)?;
        let schema_version = case
            .input
            .get("schema_version")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                HarnessError::Invalid(format!("{}: schema_version is not a u32", case.request_id))
            })?;
        let body = case
            .input
            .get("body")
            .ok_or_else(|| HarnessError::Invalid(format!("{}: body is absent", case.request_id)))?;
        let canonical = canonical_descriptor(kind, schema_version, body, 0)?;
        let actual = json!({
            "canonical_hex": encode_hex(&canonical),
            "semantic_hash": format!(
                "sha256:{:x}",
                Sha256::digest([b"conduit.semantic-hash/v1\0".as_slice(), &canonical].concat())
            ),
        });
        let found = differences(&case.expected, &actual, &case.requirement_ids[0]);
        if !found.is_empty() {
            return invalid(format!(
                "{} [{}] differs at {}",
                case.fixture, found[0].rule, found[0].path
            ));
        }
        for field in ["equivalent_bodies", "different_bodies"] {
            let Some(values) = case.input.get(field).and_then(Value::as_array) else {
                continue;
            };
            for value in values {
                let alternative = canonical_descriptor(kind, schema_version, value, 0)?;
                let same = alternative == canonical;
                if (field == "equivalent_bodies") != same {
                    return invalid(format!(
                        "{} [{}] {field} violates canonical identity",
                        case.fixture, case.requirement_ids[0]
                    ));
                }
            }
        }
    }
    for case in loaded
        .cases
        .iter()
        .filter(|case| case.operation == "canonical-descriptor-rejection-v1")
    {
        let kind = string_field(&case.input, "kind", &case.request_id)?;
        let schema_version = case
            .input
            .get("schema_version")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                HarnessError::Invalid(format!("{}: schema_version is not a u32", case.request_id))
            })?;
        let body = case
            .input
            .get("body")
            .ok_or_else(|| HarnessError::Invalid(format!("{}: body is absent", case.request_id)))?;
        let expected = string_field(&case.expected, "expected_error", &case.request_id)?;
        let actual = match canonical_descriptor(kind, schema_version, body, 0) {
            Ok(_) => "accepted",
            Err(error) => canonical_error_code(&error),
        };
        if actual != expected {
            return invalid(format!(
                "{} [{}] expected {expected}, actual {actual}",
                case.fixture, case.requirement_ids[0]
            ));
        }
    }
    Ok(())
}

fn canonical_error_code(error: &HarnessError) -> &'static str {
    let HarnessError::Invalid(message) = error else {
        return "harness-error";
    };
    if message.starts_with("duplicate canonical map key") {
        "duplicate-map-key"
    } else if message.starts_with("invalid identifier") {
        "invalid-identifier"
    } else if message == "canonical value nesting exceeds 64" {
        "maximum-depth-exceeded"
    } else {
        "malformed-canonical-value"
    }
}

fn string_field<'a>(value: &'a Value, field: &str, fixture: &str) -> Result<&'a str, HarnessError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::Invalid(format!("{fixture}: {field} is not a string")))
}

fn canonical_descriptor(
    kind: &str,
    schema_version: u32,
    body: &Value,
    depth: u32,
) -> Result<Vec<u8>, HarnessError> {
    let mut output = b"CND\x01".to_vec();
    encode_identifier(kind, &mut output)?;
    output.extend_from_slice(&schema_version.to_be_bytes());
    encode_canonical(body, depth, &mut output)?;
    Ok(output)
}

fn encode_canonical(value: &Value, depth: u32, output: &mut Vec<u8>) -> Result<(), HarnessError> {
    if value.is_null() {
        output.push(0x00);
        return Ok(());
    }
    let object = value
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or_else(|| HarnessError::Invalid("canonical value needs one tag".to_owned()))?;
    let (tag, payload) = object.iter().next().expect("one member");
    match tag.as_str() {
        "boolean" => output.push(if payload.as_bool() == Some(true) {
            0x02
        } else if payload.as_bool() == Some(false) {
            0x01
        } else {
            return invalid("canonical boolean payload is not boolean");
        }),
        "integer" => {
            output.push(0x10);
            let integer = payload.as_i64().map(i128::from).ok_or_else(|| {
                HarnessError::Invalid("integer is outside fixture range".to_owned())
            })?;
            output.extend_from_slice(&integer.to_be_bytes());
        }
        "bytes" => {
            output.push(0x20);
            let bytes = decode_hex(payload.as_str().ok_or_else(|| {
                HarnessError::Invalid("bytes payload is not hexadecimal text".to_owned())
            })?)?;
            write_length(bytes.len(), output)?;
            output.extend_from_slice(&bytes);
        }
        "text" => {
            output.push(0x21);
            let bytes = payload
                .as_str()
                .ok_or_else(|| HarnessError::Invalid("text payload is not text".to_owned()))?
                .as_bytes();
            write_length(bytes.len(), output)?;
            output.extend_from_slice(bytes);
        }
        "identifier" => encode_identifier(
            payload.as_str().ok_or_else(|| {
                HarnessError::Invalid("identifier payload is not text".to_owned())
            })?,
            output,
        )?,
        "list" | "set" | "map" => {
            if depth >= 64 {
                return invalid("canonical value nesting exceeds 64");
            }
            encode_collection(tag, payload, depth + 1, output)?;
        }
        _ => return invalid(format!("unknown canonical value tag: {tag}")),
    }
    Ok(())
}

fn encode_collection(
    tag: &str,
    payload: &Value,
    depth: u32,
    output: &mut Vec<u8>,
) -> Result<(), HarnessError> {
    if tag == "map" {
        let fields = payload
            .as_array()
            .ok_or_else(|| HarnessError::Invalid("map payload is not an array".to_owned()))?;
        let mut members = Vec::new();
        let mut names = BTreeSet::new();
        for field in fields {
            let name = string_field(field, "name", "canonical map")?;
            if !names.insert(name) {
                return invalid(format!("duplicate canonical map key: {name}"));
            }
            let disposition = field
                .get("disposition")
                .and_then(Value::as_str)
                .unwrap_or("semantic");
            if disposition == "annotation" {
                continue;
            }
            let mut encoded_value = Vec::new();
            encode_canonical(
                field.get("value").ok_or_else(|| {
                    HarnessError::Invalid("canonical map value is absent".to_owned())
                })?,
                depth,
                &mut encoded_value,
            )?;
            if disposition == "defaulted" {
                let mut default = Vec::new();
                encode_canonical(
                    field.get("default").ok_or_else(|| {
                        HarnessError::Invalid("defaulted field has no default".to_owned())
                    })?,
                    depth,
                    &mut default,
                )?;
                if default == encoded_value {
                    continue;
                }
            } else if disposition != "semantic" {
                return invalid(format!("unknown field disposition: {disposition}"));
            }
            let mut key = Vec::new();
            encode_identifier(name, &mut key)?;
            members.push((key, encoded_value));
        }
        members.sort();
        output.push(0x31);
        write_length(members.len(), output)?;
        for (key, value) in members {
            output.extend_from_slice(&key);
            output.extend_from_slice(&value);
        }
        return Ok(());
    }

    let values = payload
        .as_array()
        .ok_or_else(|| HarnessError::Invalid(format!("{tag} payload is not an array")))?;
    let mut members = Vec::new();
    for value in values {
        let mut encoded = Vec::new();
        encode_canonical(value, depth, &mut encoded)?;
        members.push(encoded);
    }
    if tag == "set" {
        members.sort();
        if members.windows(2).any(|pair| pair[0] == pair[1]) {
            return invalid("duplicate canonical set value");
        }
    }
    output.push(if tag == "list" { 0x30 } else { 0x32 });
    write_length(members.len(), output)?;
    for member in members {
        output.extend_from_slice(&member);
    }
    Ok(())
}

fn encode_identifier(value: &str, output: &mut Vec<u8>) -> Result<(), HarnessError> {
    let bytes = value.as_bytes();
    let valid = bytes.first().is_some_and(u8::is_ascii_lowercase)
        && !value.ends_with(['.', '/'])
        && !value.contains("//")
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.' | b'/')
        });
    if !valid {
        return invalid(format!("invalid identifier: {value:?}"));
    }
    output.push(0x22);
    write_length(bytes.len(), output)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn write_length(length: usize, output: &mut Vec<u8>) -> Result<(), HarnessError> {
    let length = u64::try_from(length)
        .map_err(|_| HarnessError::Invalid("canonical collection is too long".to_owned()))?;
    output.extend_from_slice(&length.to_be_bytes());
    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, HarnessError> {
    if value.len() % 2 != 0 {
        return invalid("hexadecimal byte string has odd length");
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).map_err(|_| {
                HarnessError::Invalid("hexadecimal byte string is not ASCII".to_owned())
            })?;
            u8::from_str_radix(pair, 16)
                .map_err(|_| HarnessError::Invalid("invalid hexadecimal byte string".to_owned()))
        })
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    use fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("write to string");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_manifest() -> LoadedManifest {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        load_manifest(&root.join("conformance/v1/manifest.json")).unwrap()
    }

    #[test]
    fn manifest_covers_every_normative_case() {
        let loaded = fixture_manifest();
        assert!(loaded.cases.len() >= 130);
        assert_eq!(
            loaded
                .cases
                .iter()
                .map(|case| case.request_id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            loaded.cases.len()
        );
    }

    #[test]
    fn request_stream_is_deterministic_and_excludes_expected_results() {
        let loaded = fixture_manifest();
        let mut first = Vec::new();
        let mut second = Vec::new();
        write_requests(&loaded, &mut first).unwrap();
        write_requests(&loaded, &mut second).unwrap();
        assert_eq!(first, second);
        assert!(!String::from_utf8(first).unwrap().contains("\"expected\""));
    }

    #[test]
    fn comparison_names_the_fixture_rule_and_exact_difference() {
        let loaded = fixture_manifest();
        let case = &loaded.cases[0];
        let input = format!(
            "{}\n",
            json!({
                "protocol_version": loaded.manifest.protocol_version,
                "fixture_version": loaded.manifest.fixture_version,
                "request_id": case.request_id,
                "status": "completed",
                "actual": {},
            })
        );
        let mut output = Vec::new();
        assert!(!check_results(&loaded, input.as_bytes(), &mut output).unwrap());
        let line: Value =
            serde_json::from_slice(output.split(|byte| *byte == b'\n').next().unwrap()).unwrap();
        assert_eq!(line["fixture"], case.fixture);
        assert_eq!(line["requirement_ids"], json!(case.requirement_ids));
        assert!(
            line["differences"][0]["path"]
                .as_str()
                .unwrap()
                .starts_with('/')
        );
    }

    #[test]
    fn complete_exact_results_pass_as_one_batch() {
        let loaded = fixture_manifest();
        let mut input = Vec::new();
        for case in &loaded.cases {
            serde_json::to_writer(
                &mut input,
                &json!({
                    "protocol_version": loaded.manifest.protocol_version,
                    "fixture_version": loaded.manifest.fixture_version,
                    "request_id": case.request_id,
                    "status": "completed",
                    "actual": case.expected,
                }),
            )
            .unwrap();
            input.push(b'\n');
        }
        let mut output = Vec::new();
        assert!(check_results(&loaded, input.as_slice(), &mut output).unwrap());
        assert_eq!(
            output
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .count(),
            loaded.cases.len()
        );
    }

    #[test]
    fn property_seeds_cover_byte_and_recursion_boundaries() {
        let loaded = fixture_manifest();
        for seed in &loaded.manifest.property_seeds.bytes {
            assert_eq!(encode_hex(&decode_hex(seed).unwrap()), *seed);
        }
        assert!(
            loaded
                .manifest
                .property_seeds
                .recursion_depths
                .contains(&64)
        );
        assert!(
            loaded
                .manifest
                .property_seeds
                .recursion_depths
                .contains(&65)
        );
        let nested = |depth| {
            let mut value = Value::Null;
            for _ in 0..depth {
                value = json!({"list": [value]});
            }
            value
        };
        assert!(canonical_descriptor("fixture/depth", 1, &nested(64), 0).is_ok());
        let error = canonical_descriptor("fixture/depth", 1, &nested(65), 0).unwrap_err();
        assert_eq!(canonical_error_code(&error), "maximum-depth-exceeded");
        let mut orders = loaded.manifest.property_seeds.discovery_orders.clone();
        for order in &mut orders {
            order.sort();
        }
        assert!(orders.windows(2).all(|pair| pair[0] == pair[1]));
    }
}
