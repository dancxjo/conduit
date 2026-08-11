use std::process::Command;

use conduitos::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
};
use serde::Serialize;

use crate::{cli::GlobalOpts, commands::catalog::inventory};

use super::ConduitosError;

const SCHEMA: &str = "conduit.conduitos/std-gap@1";
#[derive(Serialize)]
struct HostCapability<'a> {
    kind_id: &'static str,
    contract_revision: &'static str,
    implementation: &'static str,
    artifact_build: &'a str,
}

#[derive(Serialize)]
struct GapEntry<'a> {
    kind_id: String,
    contract_revision: String,
    classification: &'static str,
    host_capability: Option<HostCapability<'a>>,
}

#[derive(Serialize)]
struct StdGapReport<'a> {
    schema: &'static str,
    catalog_basis: &'static str,
    catalog_inventory_schema: &'static str,
    catalog_digest_algorithm: &'static str,
    catalog_digest: String,
    catalog_entry_count: usize,
    maximum_catalog_entries: usize,
    catalog_entries: Vec<inventory::InventoryEntry>,
    host_profile: &'static str,
    artifact_build: &'a str,
    comparison_key: &'static str,
    implemented_count: usize,
    missing_count: usize,
    entries: Vec<GapEntry<'a>>,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        println!("derive supported_nucleus_contracts + supported_nucleus_offers; compare exact kind_id + contract_revision with HostOffer");
        return Ok(());
    }

    let build = git_head()?;
    let report = build_report(&build)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| {
            ConduitosError::refusal("std-gap-encoding-failed", error.to_string())
        })?
    );
    Ok(())
}

fn build_report(build: &str) -> Result<StdGapReport<'_>, ConduitosError> {
    let inventory = inventory::derive().map_err(|error| {
        ConduitosError::refusal("std-catalog-inventory-invalid", error.to_string())
    })?;

    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let host = HostOffer::new(
        &ids,
        build,
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    host.validate()
        .map_err(|error| ConduitosError::refusal("conduitos-offer-invalid", error.as_str()))?;

    let entries: Vec<_> = inventory
        .entries
        .iter()
        .map(|entry| {
            let host_capability = host.capabilities.iter().find(|capability| {
                capability.kind == entry.kind_id
                    && capability.contract_revision == entry.contract_revision
            });
            GapEntry {
                kind_id: entry.kind_id.clone(),
                contract_revision: entry.contract_revision.clone(),
                classification: if host_capability.is_some() {
                    "implemented"
                } else {
                    "missing"
                },
                host_capability: host_capability.map(|capability| HostCapability {
                    kind_id: capability.kind,
                    contract_revision: capability.contract_revision,
                    implementation: capability.implementation,
                    artifact_build: capability.artifact_build,
                }),
            }
        })
        .collect();
    let implemented_count = entries
        .iter()
        .filter(|entry| entry.classification == "implemented")
        .count();

    Ok(StdGapReport {
        schema: SCHEMA,
        catalog_basis:
            "conduit_std_catalog::supported_nucleus_contracts()+supported_nucleus_offers()",
        catalog_inventory_schema: inventory::SCHEMA,
        catalog_digest_algorithm: "sha256-canonical-json",
        catalog_digest: inventory.digest,
        catalog_entry_count: inventory.entries.len(),
        maximum_catalog_entries: inventory::MAXIMUM_ENTRIES,
        catalog_entries: inventory.entries,
        host_profile: host.profile,
        artifact_build: build,
        comparison_key: "exact-kind-id+kind-contract-revision",
        implemented_count,
        missing_count: entries.len() - implemented_count,
        entries,
    })
}

fn git_head() -> Result<String, ConduitosError> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| ConduitosError::refusal("git-head-unavailable", error.to_string()))?;
    if !output.status.success() {
        return Err(ConduitosError::refusal(
            "git-head-unavailable",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn current_gap_is_derived_and_exact() {
        let report = build_report("test-build").unwrap();
        assert_eq!(report.catalog_entry_count, 28);
        assert_eq!(report.implemented_count, 5);
        assert_eq!(report.missing_count, 23);
        assert!(report.entries.iter().any(|entry| {
            entry.kind_id == "time/tick"
                && entry.contract_revision == "conduit.std/time-tick@2"
                && entry.classification == "implemented"
        }));
        assert!(report.entries.iter().any(|entry| {
            entry.kind_id == "text/upper" && entry.classification == "implemented"
        }));
        assert!(report
            .entries
            .iter()
            .any(|entry| entry.kind_id == "logic/select" && entry.classification == "missing"));
        for kind in ["time/debounce", "time/timeout"] {
            assert!(report
                .entries
                .iter()
                .any(|entry| entry.kind_id == kind && entry.classification == "missing"));
        }
        for kind in ["text/literal", "presentation/text"] {
            assert!(report
                .entries
                .iter()
                .any(|entry| { entry.kind_id == kind && entry.classification == "implemented" }));
        }
    }

    #[test]
    fn digest_changes_when_semantic_inventory_changes() {
        let mut report = build_report("test-build").unwrap();
        let original = report.catalog_digest;
        report.catalog_entries[0].contract["summary"] = Value::String("changed".into());
        assert_ne!(
            original,
            inventory::digest(&report.catalog_entries).unwrap()
        );
    }

    #[test]
    fn digest_excludes_host_build_basis() {
        let first = build_report("first-build").unwrap();
        let second = build_report("second-build").unwrap();
        assert_eq!(first.catalog_digest, second.catalog_digest);
        assert_ne!(first.artifact_build, second.artifact_build);
    }
}
