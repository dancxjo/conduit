//! Schema-driven rendering for retained product truth.

use conduit_observatory::{build_report, render_text_report};
use std::path::Path;

const MAX_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;
const BODY_BIOGRAPHY_SCHEMA: &str = "conduit.body/biography-evidence@2";

pub(crate) fn inspect(path: &Path) -> Result<String, String> {
    if path.extension().and_then(std::ffi::OsStr::to_str) == Some("conduit") {
        return inspect_form(path);
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read inspection artifact {}: {error}", path.display()))?;
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err("inspection artifact exceeds the 8 MiB product bound".into());
    }
    inspect_json(path, &bytes)
}

fn inspect_form(path: &Path) -> Result<String, String> {
    let form = crate::form_source::load(path)?.expand_entry()?;
    Ok(format!(
        "Form {}\nsource {}\nchecked {}\nexpanded {}\ngears {}\nconnections {}\nshared pools {}\n",
        form.name,
        form.source_document_id.as_str(),
        form.checked_form_id.as_str(),
        form.expanded_form_id.as_str(),
        form.gears.len(),
        form.connections.len(),
        form.shared_pools.len(),
    ))
}

fn inspect_json(path: &Path, bytes: &[u8]) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode inspection artifact {}: {error}", path.display()))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or("inspection artifact does not identify its schema")?;
    match schema {
        conduit_observatory::SNAPSHOT_SCHEMA => inspect_report(bytes),
        BODY_BIOGRAPHY_SCHEMA => inspect_body(bytes),
        _ => Err(format!("inspection does not yet support schema {schema}")),
    }
}

fn inspect_report(bytes: &[u8]) -> Result<String, String> {
    let snapshot: conduit_observatory::ObservatorySnapshot = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode Observatory report: {error}"))?;
    conduit_observatory::validate_snapshot(&snapshot)?;
    let report = build_report(&snapshot)?;
    Ok(render_text_report(&report))
}

fn inspect_body(bytes: &[u8]) -> Result<String, String> {
    let biography: conduit_body::BodyBiographyEvidence =
        serde_json::from_slice(bytes).map_err(|error| format!("decode Body biography: {error}"))?;
    biography
        .validate()
        .map_err(|error| format!("Body biography refused: {error:?}"))?;
    let present_members = biography
        .membership
        .parts
        .iter()
        .filter(|part| part.current.is_some())
        .count();
    Ok(format!(
        "Body {}\nname {}\nstate {:?}\nmembers {}\npresent members {}\nrecords {}\nwakes {}\n",
        biography.body_id.as_str(),
        biography.friendly_name,
        biography.body.state,
        biography.membership.parts.len(),
        present_members,
        biography.records.len(),
        biography.wakes.len(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_schema_fails_explicitly() {
        let error =
            inspect_json(Path::new("artifact.json"), br#"{"schema":"elsewhere/v1"}"#).unwrap_err();
        assert_eq!(error, "inspection does not yet support schema elsewhere/v1");
    }

    #[test]
    fn malformed_known_schema_is_not_pretty_printed_as_truth() {
        let error = inspect_json(
            Path::new("body.json"),
            br#"{"schema":"conduit.body/biography-evidence@2"}"#,
        )
        .unwrap_err();
        assert!(error.starts_with("decode Body biography:"));
    }
}
