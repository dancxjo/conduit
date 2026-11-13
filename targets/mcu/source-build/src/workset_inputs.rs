//! Internal finite Make subprocess receipts for existing fabrication inputs.
//! These are not human-authored canonical configuration: typed selections only
//! register exact identities authored/imported by the authoritative Source.
use super::{CapturedTypedSelections, MaskBounds, Workset, MAXIMUM_SOURCE_BYTES};
use crate::composite::{Specialization, ValueReference};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{io::Read, path::PathBuf};
const MAXIMUM_SPECIALIZATION_BYTES: usize = 64 * 1024;
const SOURCE: [&str; 3] = [
    "CONDUIT_WORKSET_SOURCE",
    "CONDUIT_WORKSET_SOURCE_SHA256",
    "CONDUIT_WORKSET_ENTRIES",
];
const SPECIALIZATIONS: [&str; 2] = [
    "CONDUIT_WORKSET_SPECIALIZATIONS",
    "CONDUIT_WORKSET_SPECIALIZATIONS_SHA256",
];

pub(super) fn from_environment(mask: Option<MaskBounds>) -> Result<Option<Workset>, String> {
    from_environment_with_installed(mask, &[], |_| Ok(Default::default()))
}
pub(super) fn from_environment_with_installed(
    mask: Option<MaskBounds>,
    installed: &[ValueReference<'_>],
    prepare: impl FnOnce(
        &conduit_plot::CheckedSyntaxDocument,
    ) -> Result<crate::installed_values::InstalledValueBindings, String>,
) -> Result<Option<Workset>, String> {
    for name in SOURCE.into_iter().chain(SPECIALIZATIONS) {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let Some(path) = std::env::var_os(SOURCE[0]) else {
        if SOURCE[1..]
            .iter()
            .chain(SPECIALIZATIONS.iter())
            .any(|name| std::env::var_os(name).is_some())
        {
            return Err("captured workset Source required with metadata".into());
        }
        return Ok(None);
    };
    let expected = std::env::var(SOURCE[1]).map_err(|_| "captured Source SHA256 required")?;
    let entries: Vec<String> = serde_json::from_str(
        &std::env::var(SOURCE[2]).map_err(|_| "captured ordered entries required")?,
    )
    .map_err(|e| format!("workset entry list: {e}"))?;
    let bytes = read(PathBuf::from(path), MAXIMUM_SOURCE_BYTES)?;
    match (
        std::env::var_os(SPECIALIZATIONS[0]),
        std::env::var(SPECIALIZATIONS[1]),
    ) {
        (None, Err(std::env::VarError::NotPresent)) => super::compile_specialized_with_installed(
            &bytes,
            &expected,
            &entries,
            mask,
            &[],
            installed,
            prepare,
        )
        .map(Some),
        (Some(path), Ok(digest)) => {
            let capture = read(PathBuf::from(path), MAXIMUM_SPECIALIZATION_BYTES)?;
            compile_captured_with_installed(
                &bytes,
                &expected,
                &entries,
                mask,
                CapturedTypedSelections {
                    bytes: &capture,
                    sha256: &digest,
                },
                installed,
                prepare,
            )
            .map(Some)
        }
        _ => Err("specialization receipt and SHA256 required together".into()),
    }
}
pub(super) fn compile_captured(
    source: &[u8],
    source_sha256: &str,
    entries: &[String],
    mask: Option<MaskBounds>,
    capture: CapturedTypedSelections<'_>,
) -> Result<Workset, String> {
    compile_captured_with_installed(source, source_sha256, entries, mask, capture, &[], |_| {
        Ok(Default::default())
    })
}
pub(super) fn compile_captured_with_installed(
    source: &[u8],
    source_sha256: &str,
    entries: &[String],
    mask: Option<MaskBounds>,
    capture: CapturedTypedSelections<'_>,
    installed: &[ValueReference<'_>],
    prepare: impl FnOnce(
        &conduit_plot::CheckedSyntaxDocument,
    ) -> Result<crate::installed_values::InstalledValueBindings, String>,
) -> Result<Workset, String> {
    if capture.bytes.is_empty() || capture.bytes.len() > MAXIMUM_SPECIALIZATION_BYTES {
        return Err("captured specialization receipt extent".into());
    }
    if format!("{:x}", Sha256::digest(capture.bytes)) != capture.sha256 {
        return Err("captured specialization SHA256 differs".into());
    }
    let description: Value = serde_json::from_slice(capture.bytes)
        .map_err(|e| format!("captured specializations: {e}"))?;
    let selected = selections(&description, source_sha256)?;
    super::compile_specialized_with_installed(
        source,
        source_sha256,
        entries,
        mask,
        &selected,
        installed,
        prepare,
    )
}

fn read(path: PathBuf, maximum: usize) -> Result<Vec<u8>, String> {
    println!("cargo:rerun-if-changed={}", path.display());
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| format!("captured workset input: {e}"))?
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("captured workset input: {e}"))?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err("captured workset input extent".into());
    }
    Ok(bytes)
}
fn object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, String> {
    let object = value.as_object().ok_or("specialization object required")?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err("unexpected specialization fields".into());
    }
    Ok(object)
}
fn reference(value: &Value) -> Result<ValueReference<'_>, String> {
    let object = object(value, &["type_name", "maximum_bytes"])?;
    let name = object["type_name"]
        .as_str()
        .filter(|name| !name.is_empty() && name.len() <= 128)
        .ok_or("bounded specialization Type name required")?;
    let maximum = object["maximum_bytes"]
        .as_u64()
        .filter(|n| (1..=4096).contains(n))
        .ok_or("specialization envelope outside finite MCU profile")?;
    Ok(ValueReference {
        type_name: name,
        maximum_bytes: maximum as u32,
    })
}
fn selections<'a>(
    description: &'a Value,
    source_sha256: &str,
) -> Result<Vec<Specialization<'a>>, String> {
    let description = object(description, &["schema", "source_sha256", "specializations"])?;
    if description["schema"].as_u64() != Some(1)
        || description["source_sha256"].as_str() != Some(source_sha256)
    {
        return Err("specializations differ from captured Source/schema".into());
    }
    let values = description["specializations"]
        .as_array()
        .filter(|values| values.len() <= 16)
        .ok_or("bounded specialization array required")?;
    values
        .iter()
        .map(|value| match value.get("kind").and_then(Value::as_str) {
            Some("seeded-until") => {
                let item = object(value, &["kind", "value"])?;
                Ok(Specialization::SeededUntilInstance(reference(
                    &item["value"],
                )?))
            }
            Some("merge") => {
                let item = object(value, &["kind", "value"])?;
                Ok(Specialization::MergeInstance(reference(&item["value"])?))
            }
            Some("zip") => {
                let item = object(value, &["kind", "left", "right", "feedback"])?;
                Ok(Specialization::ZipInstance {
                    left: reference(&item["left"])?,
                    right: reference(&item["right"])?,
                    feedback: item["feedback"]
                        .as_bool()
                        .ok_or("explicit zip feedback mode required")?,
                })
            }
            _ => Err("only exact typed family specializations are supported".into()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn description() -> Value {
        serde_json::json!({"schema":1,"source_sha256":"source-pin","specializations":[
            {"kind":"seeded-until","value":{"type_name":"State","maximum_bytes":4096}},
            {"kind":"merge","value":{"type_name":"Request","maximum_bytes":1024}},
            {"kind":"zip","left":{"type_name":"Left","maximum_bytes":512},"right":{"type_name":"Right","maximum_bytes":512},"feedback":false}
        ]})
    }
    #[test]
    fn explicit_capture_preserves_types_envelopes_and_mode_without_source_rewriting() {
        let description = description();
        let selected = selections(&description, "source-pin").unwrap();
        assert!(
            matches!(&selected[0], Specialization::SeededUntilInstance(v) if v.type_name == "State" && v.maximum_bytes == 4096)
        );
        assert!(
            matches!(&selected[1], Specialization::MergeInstance(v) if v.type_name == "Request" && v.maximum_bytes == 1024)
        );
        assert!(matches!(
            &selected[2],
            Specialization::ZipInstance {
                feedback: false,
                ..
            }
        ));
        assert!(selections(&description, "other-source").is_err());
    }
    #[test]
    fn capture_refuses_unknown_fields_modes_and_unbounded_requests() {
        let mut wrong = description();
        wrong["hidden"] = Value::Bool(true);
        assert!(selections(&wrong, "source-pin").is_err());
        wrong = description();
        wrong["specializations"][0]["kind"] = "legacy-merge".into();
        assert!(selections(&wrong, "source-pin").is_err());
        wrong = description();
        wrong["specializations"][0]["value"]["maximum_bytes"] = 4097.into();
        assert!(selections(&wrong, "source-pin").is_err());
        wrong = description();
        wrong["specializations"][2]["feedback"] = "false".into();
        assert!(selections(&wrong, "source-pin").is_err());
    }
    #[test]
    fn internal_receipt_is_pinned_to_exact_source_and_capture_bytes() {
        let source = b"plot value (\n input: U64...| >> output: U64...|\n) = (.)\n";
        let source_pin = format!("{:x}", Sha256::digest(source));
        let receipt = serde_json::to_vec(
            &serde_json::json!({"schema":1,"source_sha256":source_pin,"specializations":[]}),
        )
        .unwrap();
        let pin = format!("{:x}", Sha256::digest(&receipt));
        let entries = ["value".into()];
        assert!(compile_captured(
            source,
            &source_pin,
            &entries,
            None,
            CapturedTypedSelections {
                bytes: &receipt,
                sha256: &pin
            }
        )
        .is_ok());
        let mut changed = receipt.clone();
        changed.push(b' ');
        assert!(compile_captured(
            source,
            &source_pin,
            &entries,
            None,
            CapturedTypedSelections {
                bytes: &changed,
                sha256: &pin
            }
        )
        .is_err());
        assert!(compile_captured(
            source,
            "other-source",
            &entries,
            None,
            CapturedTypedSelections {
                bytes: &receipt,
                sha256: &pin
            }
        )
        .is_err());
    }
}
