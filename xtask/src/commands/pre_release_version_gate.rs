use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Inventory {
    schema_version: u32,
    workspace_version: String,
    policy: String,
    classifications: Vec<Classification>,
    released_obligations: Vec<Value>,
}

#[derive(Deserialize)]
struct Classification {
    family: String,
    classification: String,
    validator: String,
    locations: Vec<String>,
    #[serde(default)]
    markers: Vec<String>,
}

#[derive(Deserialize)]
struct ExceptionLedger {
    schema_version: u32,
    releases: Vec<Value>,
}

const REQUIRED_VALIDATORS: &[&str] = &[
    "workspace-version",
    "owned-corpus",
    "current-schema",
    "panel-source",
    "semantic-contract",
    "external-standard",
    "historical-record",
    "rejected-input",
    "release-ledger",
];

const KNOWN_VALIDATORS: &[&str] = &[
    "workspace-version",
    "owned-corpus",
    "current-schema",
    "panel-source",
    "semantic-contract",
    "external-standard",
    "historical-record",
    "rejected-input",
    "release-ledger",
    "generated-binary",
];

const SPECIAL_CLASSIFICATIONS: &[&str] = &[
    "external-protocol-standard-version",
    "historical-non-normative-record",
    "rejected-input-fixture",
];

pub fn run(workspace_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    validate_repository(workspace_root)?;
    println!(
        "pre-release version gate passed: complete owned corpus, one current draft, empty release ledger"
    );
    Ok(())
}

fn validate_repository(workspace_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let inventory_path = workspace_root.join("inventory/pre-release-versions.json");
    let inventory: Inventory = serde_json::from_slice(&fs::read(&inventory_path)?)?;
    validate_inventory_shape(&inventory)?;

    let ledger_path = workspace_root.join("inventory/release-compatibility-exceptions.json");
    let ledger: ExceptionLedger = serde_json::from_slice(&fs::read(&ledger_path)?)?;
    if ledger.schema_version != 0 || !ledger.releases.is_empty() {
        return Err(
            "pre-release compatibility exception ledger must be empty until a tagged release"
                .into(),
        );
    }

    let manifest = fs::read_to_string(workspace_root.join("Cargo.toml"))?;
    if !manifest.contains("version = \"0.0.0-dev\"") {
        return Err("workspace package version must be 0.0.0-dev".into());
    }

    let mut repository_files = Vec::new();
    collect_files(workspace_root, workspace_root, &mut repository_files)?;
    repository_files.sort();

    let mut inspected_locations = BTreeSet::new();
    for file in &repository_files {
        let relative = file.strip_prefix(workspace_root).unwrap_or(file);
        let claims = claims_for(relative, &inventory.classifications);
        if !is_relevant_file(relative, file)? {
            let generated_claims = claims
                .into_iter()
                .filter(|claim| claim.validator == "generated-binary")
                .collect::<Vec<_>>();
            mark_inspected_locations(relative, &generated_claims, &mut inspected_locations);
            continue;
        }
        if !claims.iter().any(|claim| claim.validator == "owned-corpus") {
            return Err(format!(
                "repository-owned persisted or textual artifact is omitted from the inventory: {}",
                relative.display()
            )
            .into());
        }
        validate_claim_conflicts(relative, &claims)?;
        validate_release_looking_path(relative)?;

        let bytes = fs::read(file)?;
        if let Ok(text) = std::str::from_utf8(&bytes) {
            validate_text(relative, text, &claims)?;
            validate_structured_file(relative, text, &claims)?;
        }
        mark_inspected_locations(relative, &claims, &mut inspected_locations);
    }

    for entry in &inventory.classifications {
        for location in &entry.locations {
            let location_path = workspace_root.join(location);
            if !location_path.exists() {
                return Err(format!(
                    "pre-release inventory location is missing for family {}: {location}",
                    entry.family
                )
                .into());
            }
            let key = (entry.family.clone(), location.clone());
            if !inspected_locations.contains(&key) {
                return Err(format!(
                    "pre-release inventory location has no file inspected by validator {}: {location}",
                    entry.validator
                )
                .into());
            }
        }
    }

    Ok(())
}

fn validate_inventory_shape(inventory: &Inventory) -> Result<(), Box<dyn std::error::Error>> {
    if inventory.schema_version != 0
        || inventory.workspace_version != "0.0.0-dev"
        || inventory.policy != "one-current-draft"
        || !inventory.released_obligations.is_empty()
    {
        return Err(
            "pre-release version inventory does not describe the one-current-draft policy".into(),
        );
    }
    if inventory.classifications.is_empty() {
        return Err("pre-release version inventory has no classifications".into());
    }

    let mut families = BTreeSet::new();
    let mut validators = BTreeSet::new();
    for entry in &inventory.classifications {
        if entry.family.trim().is_empty()
            || entry.classification.trim().is_empty()
            || entry.validator.trim().is_empty()
            || entry.locations.is_empty()
        {
            return Err("pre-release inventory entry is incomplete".into());
        }
        if !families.insert(entry.family.as_str()) {
            return Err(format!("pre-release inventory repeats family {}", entry.family).into());
        }
        if !KNOWN_VALIDATORS.contains(&entry.validator.as_str()) {
            return Err(format!(
                "pre-release inventory family {} names unknown validator {}",
                entry.family, entry.validator
            )
            .into());
        }
        validators.insert(entry.validator.as_str());
        if entry.validator == "current-schema" && entry.markers.is_empty() {
            return Err(format!(
                "pre-release inventory family {} has no owned schema markers",
                entry.family
            )
            .into());
        }
        for location in &entry.locations {
            validate_inventory_location(location)?;
        }
    }
    for required in REQUIRED_VALIDATORS {
        if !validators.contains(required) {
            return Err(format!(
                "pre-release inventory has no family assigned to validator {required}"
            )
            .into());
        }
    }
    Ok(())
}

fn validate_inventory_location(location: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(location);
    if location.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(format!("invalid pre-release inventory location: {location}").into());
    }
    Ok(())
}

fn claims_for<'a>(
    relative: &Path,
    classifications: &'a [Classification],
) -> Vec<&'a Classification> {
    classifications
        .iter()
        .filter(|entry| {
            entry.locations.iter().any(|location| {
                let location = Path::new(location);
                relative == location || relative.starts_with(location)
            })
        })
        .collect()
}

fn mark_inspected_locations(
    relative: &Path,
    claims: &[&Classification],
    inspected: &mut BTreeSet<(String, String)>,
) {
    for claim in claims {
        for location in &claim.locations {
            let location_path = Path::new(location);
            if relative == location_path || relative.starts_with(location_path) {
                inspected.insert((claim.family.clone(), location.clone()));
            }
        }
    }
}

fn validate_claim_conflicts(
    relative: &Path,
    claims: &[&Classification],
) -> Result<(), Box<dyn std::error::Error>> {
    let special = claims
        .iter()
        .filter(|claim| SPECIAL_CLASSIFICATIONS.contains(&claim.classification.as_str()))
        .map(|claim| claim.classification.as_str())
        .collect::<BTreeSet<_>>();
    if special.len() > 1 {
        return Err(format!(
            "conflicting pre-release classifications inspect {}: {special:?}",
            relative.display()
        )
        .into());
    }
    Ok(())
}

fn validate_release_looking_path(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for component in path.components() {
        let text = component.as_os_str().to_string_lossy();
        if has_release_looking_token(&text) {
            return Err(format!(
                "repository-owned path has a release-looking draft marker: {}",
                path.display()
            )
            .into());
        }
    }
    Ok(())
}

fn validate_text(
    relative: &Path,
    text: &str,
    claims: &[&Classification],
) -> Result<(), Box<dyn std::error::Error>> {
    let historical = has_classification(claims, "historical-non-normative-record");
    let rejected = has_classification(claims, "rejected-input-fixture");
    let semantic = has_validator(claims, "semantic-contract");
    let external = has_validator(claims, "external-standard");

    if historical {
        validate_historical_header(relative, text)?;
    } else {
        validate_normative_prose(relative, text)?;
    }

    if !rejected {
        validate_embedded_panel(relative, text)?;
        validate_textual_schema_markers(relative, text)?;
        validate_owned_identities(relative, text)?;
        if !semantic && !external {
            validate_current_code_prose(relative, text)?;
        }
    }

    if relative
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("rs")
        && !relative
            .components()
            .any(|component| component.as_os_str() == "tests")
        && !semantic
        && !rejected
    {
        validate_rust_compatibility_machinery(relative, text)?;
    }
    Ok(())
}

fn validate_structured_file(
    relative: &Path,
    text: &str,
    claims: &[&Classification],
) -> Result<(), Box<dyn std::error::Error>> {
    if has_classification(claims, "rejected-input-fixture") {
        return Ok(());
    }
    let owned_schema_markers = claims
        .iter()
        .filter(|claim| claim.validator == "current-schema")
        .flat_map(|claim| claim.markers.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    match relative
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("json") => {
            let value: Value = serde_json::from_str(text).map_err(|error| {
                format!(
                    "owned JSON artifact {} is malformed: {error}",
                    relative.display()
                )
            })?;
            validate_json_versions(relative, &value, "$", &owned_schema_markers)?;
        }
        Some("jsonl" | "ndjson") => {
            for (index, line) in text.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let value: Value = serde_json::from_str(line).map_err(|error| {
                    format!(
                        "owned JSON-lines artifact {}:{} is malformed: {error}",
                        relative.display(),
                        index + 1
                    )
                })?;
                validate_json_versions(
                    relative,
                    &value,
                    &format!("line {}", index + 1),
                    &owned_schema_markers,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_json_versions(
    relative: &Path,
    value: &Value,
    location: &str,
    owned_schema_markers: &BTreeSet<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_location = format!("{location}.{key}");
                if is_owned_version_key(key) && child.as_u64().is_some_and(|version| version != 0) {
                    return Err(format!(
                        "owned schema or revision is not 0 in {} at {child_location}",
                        relative.display()
                    )
                    .into());
                }
                if key == "schema"
                    && child.as_str().is_some_and(|schema| {
                        schema.starts_with("conduit") && !owned_schema_markers.contains(schema)
                    })
                {
                    return Err(format!(
                        "owned persisted schema marker is omitted from the inventory in {} at {child_location}",
                        relative.display()
                    )
                    .into());
                }
                validate_json_versions(relative, child, &child_location, owned_schema_markers)?;
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_json_versions(
                    relative,
                    child,
                    &format!("{location}[{index}]"),
                    owned_schema_markers,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_owned_version_key(key: &str) -> bool {
    key == "version"
        || key == "grammar"
        || key == "schema"
        || key.ends_with("_version")
        || key.ends_with("_schema")
        || key.ends_with("_schema_version")
}

fn validate_historical_header(
    relative: &Path,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let header = text.lines().take(18).collect::<Vec<_>>().join("\n");
    let lower = header.to_ascii_lowercase();
    if !lower.contains("historical record")
        || !lower.contains("non-normative")
        || !lower.contains("audit date")
        || !lower.contains("audited baseline")
    {
        return Err(format!(
            "historical record lacks explicit non-normative date and baseline quarantine: {}",
            relative.display()
        )
        .into());
    }
    Ok(())
}

fn validate_normative_prose(relative: &Path, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    if relative
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("md")
    {
        return Ok(());
    }
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        let obligation = [
            "remains readable",
            "remain readable",
            "continue to read",
            "continues to read",
            "preserve backwards compatibility",
            "preserve backward compatibility",
            "must retain the old",
            "must retain old",
            "frozen v1",
            "frozen at their stated v1",
        ]
        .iter()
        .any(|phrase| lower.contains(phrase));
        if obligation && !lower.contains("must not") && !lower.contains("reject") {
            return Err(format!(
                "normative prose preserves an unreleased draft in {}:{}",
                relative.display(),
                index + 1
            )
            .into());
        }
    }
    Ok(())
}

fn validate_embedded_panel(relative: &Path, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    if relative
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("panel")
    {
        let header = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty() && !line.starts_with('#'));
        if header != Some("panel 0") {
            return Err(format!(
                "canonical Panel source is not panel 0: {}",
                relative.display()
            )
            .into());
        }
        validate_current_panel_spelling(relative, text)?;
        return Ok(());
    }

    if relative
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("md")
    {
        let mut in_panel_fence = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                if in_panel_fence {
                    in_panel_fence = false;
                } else if trimmed == "```panel" {
                    in_panel_fence = true;
                }
                continue;
            }
            if in_panel_fence && trimmed.starts_with("panel ") && trimmed != "panel 0" {
                return Err(format!(
                    "embedded Panel source is not panel 0: {}",
                    relative.display()
                )
                .into());
            }
            if in_panel_fence {
                validate_current_panel_line(relative, trimmed)?;
            }
        }
        return Ok(());
    }

    let bytes = text.as_bytes();
    let needle = b"panel ";
    let mut offset = 0;
    while let Some(found) = find_bytes(&bytes[offset..], needle) {
        let start = offset + found;
        let preceding = start
            .checked_sub(1)
            .and_then(|index| bytes.get(index))
            .copied();
        let source_like = start == 0
            || matches!(preceding, Some(b'"' | b'\'' | b'`'))
            || start >= 2 && &bytes[start - 2..start] == br#"\""#;
        let digit_start = start + needle.len();
        let digit_end = bytes[digit_start..]
            .iter()
            .position(|byte| !byte.is_ascii_digit())
            .map_or(bytes.len(), |length| digit_start + length);
        if source_like
            && digit_end > digit_start
            && std::str::from_utf8(&bytes[digit_start..digit_end])? != "0"
        {
            return Err(format!(
                "embedded Panel source is not panel 0: {}",
                relative.display()
            )
            .into());
        }
        offset = digit_start;
    }
    Ok(())
}

fn validate_current_panel_spelling(
    relative: &Path,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for line in text.lines() {
        validate_current_panel_line(relative, line.trim())?;
    }
    Ok(())
}

fn validate_current_panel_line(
    relative: &Path,
    line: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if line.is_empty() || line.starts_with('#') {
        return Ok(());
    }
    if ["node ", "cord ", "composite "]
        .iter()
        .any(|prefix| line.starts_with(prefix))
        || line.contains(" -> ")
        || line.contains(" <- ")
    {
        return Err(format!(
            "displaced Panel declaration or connection spelling in {}",
            relative.display()
        )
        .into());
    }
    Ok(())
}

fn validate_textual_schema_markers(
    relative: &Path,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let extension = relative
        .extension()
        .and_then(|extension| extension.to_str());
    if !matches!(
        extension,
        Some("rs" | "js" | "mjs" | "ts" | "html" | "md" | "toml")
    ) {
        return Ok(());
    }
    if extension == Some("rs")
        && relative
            .components()
            .any(|component| component.as_os_str() == "tests")
    {
        return Ok(());
    }

    for (index, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        for marker in [
            "schema_version",
            "schemaversion",
            "schema version",
            "schema marker",
            "grammar version",
            "manifest revision",
            "contract_version",
            "protocol_version",
            "form_version",
        ] {
            let Some(position) = lower.find(marker) else {
                continue;
            };
            let tail = &lower[position + marker.len()..];
            if let Some(number) = first_number_after_assignment(tail)
                && number != 0
            {
                return Err(format!(
                    "owned textual schema or revision is not 0 in {}:{}",
                    relative.display(),
                    index + 1
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_current_code_prose(
    relative: &Path,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if relative
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("rs")
        || relative
            .components()
            .any(|component| component.as_os_str() == "tests")
    {
        return Ok(());
    }
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        for version in 1..=99 {
            let forbidden = [
                format!("schema version {version}"),
                format!("schema {version}"),
                format!("grammar version {version}"),
                format!("canonical form version {version}"),
                format!("plan schema {version}"),
                format!(" version-{version}"),
                format!(" v{version} "),
                format!(" v{version}."),
            ];
            if forbidden.iter().any(|marker| lower.contains(marker)) {
                return Err(format!(
                    "current implementation prose names a nonzero unreleased draft in {}:{}",
                    relative.display(),
                    index + 1
                )
                .into());
            }
        }
    }
    Ok(())
}

fn first_number_after_assignment(text: &str) -> Option<u64> {
    let mut saw_separator = false;
    let mut digits = String::new();
    for character in text.chars().take(48) {
        if !saw_separator {
            if matches!(character, ':' | '=' | '`') {
                saw_separator = true;
            } else if !character.is_whitespace() {
                return None;
            }
        } else if character.is_ascii_digit() {
            digits.push(character);
        } else if !digits.is_empty() {
            break;
        } else if !matches!(character, ' ' | '\t' | '"' | '\'' | '`') {
            return None;
        }
    }
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn validate_owned_identities(
    relative: &Path,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for token in text.split(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
    }) {
        let trimmed = token.trim_matches(|character: char| {
            matches!(character, ':' | '=' | '<' | '>' | '*' | '\\')
        });
        let lower = trimmed.to_ascii_lowercase();
        let conduit_magic = lower.len() == 4
            && lower.starts_with("cn")
            && lower
                .as_bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_digit() && *byte != b'0');
        if ((lower.starts_with("conduit")
            || lower.starts_with("std/")
            || lower.starts_with("flow/"))
            && has_release_looking_token(&lower))
            || conduit_magic
        {
            return Err(format!(
                "Conduit-owned identity has a release-looking draft marker in {}: {trimmed}",
                relative.display()
            )
            .into());
        }
    }
    Ok(())
}

fn validate_rust_compatibility_machinery(
    relative: &Path,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for identifier in rust_identifiers(text) {
        let lower = identifier.to_ascii_lowercase();
        let historical = [
            "legacy",
            "retired",
            "obsolete",
            "deprecated",
            "previous",
            "former",
            "displaced",
            "olddraft",
            "old_schema",
            "old_version",
        ]
        .iter()
        .any(|part| lower.contains(part));
        let compatibility_action = [
            "read", "reader", "write", "writer", "parse", "decode", "encode", "convert", "migrate",
            "upgrade", "alias", "shim", "fallback", "compat",
        ]
        .iter()
        .any(|part| lower.contains(part));
        let versioned_migrator = (lower.contains("migrate") || lower.contains("upgrade"))
            && contains_nonzero_version_token(&lower);
        let compatibility_alias = lower.contains("alias")
            && (lower.contains("compat") || lower.contains("schema") || historical);
        let silent_fallback = lower.contains("fallback")
            && ["schema", "version", "reader", "parser", "decode", "draft"]
                .iter()
                .any(|part| lower.contains(part));
        if (historical && compatibility_action)
            || versioned_migrator
            || compatibility_alias
            || silent_fallback
        {
            return Err(format!(
                "unreleased compatibility machinery `{identifier}` remains in {}",
                relative.display()
            )
            .into());
        }
    }
    Ok(())
}

fn rust_identifiers(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| !token.is_empty())
}

fn has_classification(claims: &[&Classification], classification: &str) -> bool {
    claims
        .iter()
        .any(|claim| claim.classification == classification)
}

fn has_validator(claims: &[&Classification], validator: &str) -> bool {
    claims.iter().any(|claim| claim.validator == validator)
}

fn has_release_looking_token(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] == b'v'
            && bytes
                .get(index + 1)
                .is_some_and(|byte| byte.is_ascii_digit() && *byte != b'0')
            && (index == 0 || matches!(bytes[index - 1], b'/' | b'-' | b'_' | b'.' | b'@'))
        {
            return true;
        }
        if bytes[index..].starts_with(b"schema")
            && bytes
                .get(index + "schema".len())
                .is_some_and(|byte| byte.is_ascii_digit() && *byte != b'0')
        {
            return true;
        }
    }
    false
}

fn contains_nonzero_version_token(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes
        .windows(2)
        .any(|pair| pair[0] == b'v' && pair[1].is_ascii_digit() && pair[1] != b'0')
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn collect_files(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if matches!(
            relative
                .components()
                .next()
                .and_then(|part| part.as_os_str().to_str()),
            Some(
                ".git"
                    | "target"
                    | "node_modules"
                    | "test-results"
                    | "playwright-report"
                    | "__pycache__"
            )
        ) {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, output)?;
        } else {
            output.push(path);
        }
    }
    Ok(())
}

fn is_relevant_file(relative: &Path, absolute: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let extension = relative
        .extension()
        .and_then(|extension| extension.to_str());
    if matches!(
        extension,
        Some(
            "rs" | "md"
                | "json"
                | "jsonl"
                | "ndjson"
                | "tsv"
                | "panel"
                | "js"
                | "mjs"
                | "ts"
                | "html"
                | "css"
                | "toml"
                | "lock"
                | "yml"
                | "yaml"
                | "sh"
                | "bash"
                | "txt"
                | "svg"
                | "ps1"
                | "fish"
                | "elv"
                | "x"
        )
    ) {
        return Ok(true);
    }
    if relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                "AGENTS.md"
                    | "Cargo.lock"
                    | "justfile"
                    | "LICENSE"
                    | "README.md"
                    | "SECURITY.md"
                    | "CONTRIBUTING.md"
                    | "CHANGELOG.md"
                    | ".gitignore"
            )
        })
    {
        return Ok(true);
    }
    let bytes = fs::read(absolute)?;
    Ok(bytes.len() <= 1_048_576 && std::str::from_utf8(&bytes).is_ok())
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

    struct MiniRepository {
        root: PathBuf,
    }

    impl MiniRepository {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = env::temp_dir().join(format!(
                "conduit-pre-release-gate-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("inventory")).unwrap();
            fs::create_dir_all(root.join("src")).unwrap();
            fs::create_dir_all(root.join("docs")).unwrap();
            fs::create_dir_all(root.join("external")).unwrap();
            fs::create_dir_all(root.join("semantic")).unwrap();
            fs::create_dir_all(root.join("fixtures")).unwrap();
            fs::write(
                root.join("Cargo.toml"),
                "[workspace]\n[workspace.package]\nversion = \"0.0.0-dev\"\n",
            )
            .unwrap();
            fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
            fs::write(
                root.join("src/lib.rs"),
                "pub const SCHEMA_VERSION: u32 = 0;\n",
            )
            .unwrap();
            fs::write(
                root.join("docs/history.md"),
                "# Historical record\n\n> Historical record -- non-normative.\n>\n> Audit date: 2026-07-30.\n> Audited baseline: `abc123`.\n",
            )
            .unwrap();
            fs::write(
                root.join("external/standards.md"),
                "HTTP/1.1, HTTP/2, TLS 1.3, DNSSEC and WASM remain external standards.\n",
            )
            .unwrap();
            fs::write(
                root.join("semantic/compatibility.rs"),
                "pub fn assess_semantic_compatibility() {}\n",
            )
            .unwrap();
            fs::write(
                root.join("fixtures/rejected.json"),
                r#"{"schema":"conduit.fixture","schema_version":9}"#,
            )
            .unwrap();
            fs::write(
                root.join("inventory/release-compatibility-exceptions.json"),
                r#"{"schema_version":0,"releases":[]}"#,
            )
            .unwrap();
            let repository = Self { root };
            repository.write_inventory(None);
            repository
        }

        fn write_inventory(&self, extra: Option<Value>) {
            let mut classifications = vec![
                json!({
                    "family": "workspace",
                    "classification": "current-draft-self-description",
                    "validator": "workspace-version",
                    "locations": ["Cargo.toml", "Cargo.lock"]
                }),
                json!({
                    "family": "corpus",
                    "classification": "repository-owned-corpus",
                    "validator": "owned-corpus",
                    "locations": [
                        "Cargo.toml", "Cargo.lock", "inventory", "src", "docs",
                        "external", "semantic", "fixtures"
                    ]
                }),
                json!({
                    "family": "schemas",
                    "classification": "current-draft-self-description",
                    "validator": "current-schema",
                    "markers": ["conduit.current"],
                    "locations": ["src"]
                }),
                json!({
                    "family": "panels",
                    "classification": "current-draft-self-description",
                    "validator": "panel-source",
                    "locations": ["src"]
                }),
                json!({
                    "family": "semantic",
                    "classification": "semantic-identity-substitution-fact",
                    "validator": "semantic-contract",
                    "locations": ["semantic"]
                }),
                json!({
                    "family": "external",
                    "classification": "external-protocol-standard-version",
                    "validator": "external-standard",
                    "locations": ["external"]
                }),
                json!({
                    "family": "history",
                    "classification": "historical-non-normative-record",
                    "validator": "historical-record",
                    "locations": ["docs/history.md"]
                }),
                json!({
                    "family": "rejections",
                    "classification": "rejected-input-fixture",
                    "validator": "rejected-input",
                    "locations": ["fixtures/rejected.json"]
                }),
                json!({
                    "family": "ledger",
                    "classification": "released-obligation",
                    "validator": "release-ledger",
                    "locations": ["inventory/release-compatibility-exceptions.json"]
                }),
            ];
            if let Some(extra) = extra {
                classifications.push(extra);
            }
            let inventory = json!({
                "schema_version": 0,
                "workspace_version": "0.0.0-dev",
                "policy": "one-current-draft",
                "classifications": classifications,
                "released_obligations": []
            });
            fs::write(
                self.root.join("inventory/pre-release-versions.json"),
                serde_json::to_vec_pretty(&inventory).unwrap(),
            )
            .unwrap();
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }

        fn error(&self) -> String {
            validate_repository(&self.root).unwrap_err().to_string()
        }
    }

    impl Drop for MiniRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn miniature_current_repository_passes_with_external_and_historical_versions() {
        let repository = MiniRepository::new();
        repository.write(".git", "gitdir: /tmp/conduit-worktrees/example\n");
        validate_repository(&repository.root).unwrap();
    }

    #[test]
    fn rejects_nonzero_json_schema_and_owned_versioned_identity_and_path() {
        let repository = MiniRepository::new();
        repository.write("src/artifact.json", r#"{"schema_version":1}"#);
        assert!(repository.error().contains("schema or revision is not 0"));

        repository.write("src/artifact.json", r#"{"schema_version":0}"#);
        repository.write(
            "src/identity.rs",
            "const ID: &str = \"conduit.example/v1\";\n",
        );
        assert!(repository.error().contains("release-looking draft marker"));

        repository.write("src/identity.rs", "const MAGIC: &[u8] = b\"CNH4\";\n");
        assert!(repository.error().contains("release-looking draft marker"));

        fs::remove_file(repository.root.join("src/identity.rs")).unwrap();
        repository.write("src/releases/v2/data.json", r#"{"schema_version":0}"#);
        assert!(repository.error().contains("release-looking draft marker"));
    }

    #[test]
    fn rejects_embedded_panel_in_json_javascript_and_rust() {
        for (path, contents) in [
            ("src/source.json", r#"{"source":"panel 3\na: x"}"#),
            ("src/source.js", "const source = \"panel 3\\na: x\";\n"),
            (
                "src/source.rs",
                "const SOURCE: &str = \"panel 3\\na: x\";\n",
            ),
        ] {
            let repository = MiniRepository::new();
            repository.write(path, contents);
            assert!(
                repository.error().contains("Panel source is not panel 0"),
                "{path}"
            );
        }
    }

    #[test]
    fn rejects_displaced_panel_source_spelling_in_owned_panels() {
        for source in [
            "panel 0\nnode value: fixture/source\n",
            "panel 0\ncord value.out -> sink.in\n",
            "panel 0\ncord sink.in <- value.out\n",
            "panel 0\ncomposite box { value: fixture/source }\n",
        ] {
            let repository = MiniRepository::new();
            repository.write("src/example.panel", source);
            assert!(
                repository
                    .error()
                    .contains("displaced Panel declaration or connection spelling"),
                "{source}"
            );
        }
    }

    #[test]
    fn rejects_renamed_reader_migrator_alias_and_schema_fallback() {
        for source in [
            "fn decode_retired_plan() {}\n",
            "fn convert_previous_schema() {}\n",
            "fn compatibility_schema_alias() {}\n",
            "fn schema_reader_fallback() {}\n",
            "fn upgrade_payload_v7() {}\n",
        ] {
            let repository = MiniRepository::new();
            repository.write("src/mechanism.rs", source);
            assert!(
                repository
                    .error()
                    .contains("unreleased compatibility machinery"),
                "{source}"
            );
        }
    }

    #[test]
    fn rejects_normative_unreleased_readability_claim() {
        let repository = MiniRepository::new();
        repository.write(
            "docs/policy.md",
            "The old unreleased schema remains readable by every current reader.\n",
        );
        assert!(repository.error().contains("normative prose preserves"));
    }

    #[test]
    fn rejects_new_uninventoried_family_and_unimplemented_validator() {
        let repository = MiniRepository::new();
        repository.write(
            "new-family/artifact.json",
            r#"{"schema":"conduit.new","schema_version":0}"#,
        );
        assert!(repository.error().contains("omitted from the inventory"));

        fs::remove_dir_all(repository.root.join("new-family")).unwrap();
        repository.write(
            "src/new-family.json",
            r#"{"schema":"conduit.new-family","schema_version":0}"#,
        );
        assert!(
            repository
                .error()
                .contains("persisted schema marker is omitted")
        );

        fs::remove_file(repository.root.join("src/new-family.json")).unwrap();
        repository.write_inventory(Some(json!({
            "family": "uninspected",
            "classification": "current-draft-self-description",
            "validator": "does-not-exist",
            "locations": ["src"]
        })));
        assert!(repository.error().contains("unknown validator"));
    }

    #[test]
    fn rejects_inventory_location_no_validator_inspects_and_fake_release() {
        let repository = MiniRepository::new();
        fs::create_dir_all(repository.root.join("empty")).unwrap();
        fs::write(
            repository.root.join("empty/ignored.wasm"),
            [0xff, 0xfe, 0xfd],
        )
        .unwrap();
        repository.write_inventory(Some(json!({
            "family": "uninspected",
            "classification": "current-draft-self-description",
            "validator": "current-schema",
            "markers": ["conduit.uninspected"],
            "locations": ["empty/ignored.wasm"]
        })));
        assert!(repository.error().contains("no file inspected"));

        fs::remove_dir_all(repository.root.join("empty")).unwrap();
        repository.write_inventory(None);
        repository.write(
            "inventory/release-compatibility-exceptions.json",
            r#"{"schema_version":0,"releases":[{"tag":"v0.1.0"}]}"#,
        );
        assert!(repository.error().contains("must be empty"));
    }
}
