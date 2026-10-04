//! Checked Source packaging, independent of native resource admission.
use super::ConduitosError;
use crate::cli::GlobalOpts;
use clap::Args;
use conduitos::protocol_source::{
    PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSourceRefusal,
    ProtocolSpecializationRequest, MAXIMUM_PACKAGE_BYTES, MAXIMUM_SOURCE_BYTES,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::PathBuf};

#[derive(Args, Debug)]
pub(super) struct PackageArgs {
    /// Bounded JSON Source package with generic typed Back specializations.
    #[arg(long, required_unless_present = "source", conflicts_with = "source")]
    package: Option<PathBuf>,
    /// Combined ordinary Source; schemas are derived from its checked declarations.
    #[arg(
        long,
        required_unless_present = "package",
        conflicts_with = "package",
        requires = "specializations"
    )]
    source: Option<PathBuf>,
    /// JSON array of generic Back requests with named Source types and byte ceilings.
    #[arg(long, requires = "source")]
    specializations: Option<PathBuf>,
    /// Checked plot to expand as the protocol entrance.
    #[arg(long)]
    entry: String,
    /// New directory for the exact package and its checking receipt.
    #[arg(long)]
    output_dir: PathBuf,
}

pub(super) fn execute(args: PackageArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        println!(
            "Check {} entry {} and package into {}",
            args.package
                .as_ref()
                .or(args.source.as_ref())
                .unwrap()
                .display(),
            args.entry,
            args.output_dir.display()
        );
        return Ok(());
    }
    let bytes = if let Some(package) = &args.package {
        read(package, MAXIMUM_PACKAGE_BYTES)?
    } else {
        let source_bytes = read(args.source.as_ref().unwrap(), MAXIMUM_SOURCE_BYTES)?;
        let source = String::from_utf8(source_bytes).map_err(|error| {
            ConduitosError::refusal("protocol-source-encoding", error.to_string())
        })?;
        let requests: Vec<ProtocolSpecializationRequest> =
            serde_json::from_slice(&read(args.specializations.as_ref().unwrap(), 16384)?).map_err(
                |error| {
                    ConduitosError::refusal("protocol-source-specializations", error.to_string())
                },
            )?;
        let package = ProtocolSourcePackage::compile(source, &requests).map_err(|error| {
            ConduitosError::refusal("protocol-source-compilation", format!("{error:?}"))
        })?;
        serde_json::to_vec(&package).map_err(|error| {
            ConduitosError::refusal("protocol-source-package-encoding", error.to_string())
        })?
    };
    let receipt = check(&bytes, &args.entry)?;
    let encoded_receipt = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| ConduitosError::refusal("protocol-source-receipt", error.to_string()))?;
    // A new directory preserves any existing package or evidence. No target
    // build or controller operation occurs during this preparation entrance.
    fs::create_dir(&args.output_dir).map_err(io_error)?;
    fs::write(args.output_dir.join("protocol-source.json"), &bytes).map_err(io_error)?;
    fs::write(args.output_dir.join("receipt.json"), encoded_receipt).map_err(io_error)?;
    println!("Checked Source package: {}", args.output_dir.display());
    Ok(())
}

fn check(bytes: &[u8], entry: &str) -> Result<Value, ConduitosError> {
    let prepared = PreparedProtocolEntry::prepare(bytes, entry).map_err(|error| {
        let code = match &error {
            ProtocolSourceRefusal::Bounds
            | ProtocolSourceRefusal::Encoding
            | ProtocolSourceRefusal::UnsupportedSchema => "protocol-source-package-refused",
            ProtocolSourceRefusal::Expansion(_) => "protocol-source-entry-refused",
            _ => "protocol-source-check-refused",
        };
        ConduitosError::refusal(code, format!("{error:?}"))
    })?;
    let expanded = &prepared.expanded().expanded;
    Ok(json!({
        "schema": "conduit.conduitos/protocol-source-receipt@1",
        "proof_class": "source-check-and-expansion",
        "package_sha256": format!("sha256:{:x}", Sha256::digest(bytes)),
        "package_bytes": bytes.len(),
        "entry": entry,
        "artifact_id": prepared.artifact_id().as_str(),
        "source_document_id": expanded.source_document_id.as_str(),
        "checked_plot_id": expanded.checked_plot_id.as_str(),
        "expanded_plot_id": expanded.expanded_plot_id.as_str(),
        "gears": expanded.gears.len(),
    }))
}

fn read(path: &std::path::Path, maximum: usize) -> Result<Vec<u8>, ConduitosError> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(io_error)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(ConduitosError::refusal(
            "protocol-source-input-bounds",
            path.display().to_string(),
        ));
    }
    Ok(bytes)
}

fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("protocol-source-package-io", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduitos::protocol_source::ProtocolSourcePackage;

    fn package() -> Vec<u8> {
        serde_json::to_vec(&ProtocolSourcePackage {
            schema: conduitos::protocol_source::PACKAGE_SCHEMA.into(),
            source: "plot identity (\n >> input: Boolean...|\n output: Boolean...| >>\n) = (.)"
                .into(),
            specializations: vec![],
        })
        .unwrap()
    }

    #[test]
    fn receipt_binds_exact_package_bytes_and_distinct_checked_identities() {
        let bytes = package();
        let receipt = check(&bytes, "identity").unwrap();
        assert_eq!(
            receipt["package_sha256"],
            format!("sha256:{:x}", Sha256::digest(&bytes))
        );
        assert_eq!(receipt["package_bytes"], bytes.len());
        assert_eq!(receipt["proof_class"], "source-check-and-expansion");
        assert_eq!(
            receipt["artifact_id"],
            PreparedProtocolEntry::prepare(&bytes, "identity")
                .unwrap()
                .artifact_id()
                .as_str()
        );
        for key in ["source_document_id", "checked_plot_id", "expanded_plot_id"] {
            assert!(!receipt[key].as_str().unwrap().is_empty());
        }
        assert!(receipt.get("plan_id").is_none());
        assert!(receipt.get("authority").is_none());
    }

    #[test]
    fn malformed_or_oversized_packages_and_unknown_entries_are_refused() {
        assert!(check(b"{}", "identity").is_err());
        assert!(check(&vec![0; MAXIMUM_PACKAGE_BYTES + 1], "identity").is_err());
        assert!(check(&package(), "missing").is_err());
    }
}
