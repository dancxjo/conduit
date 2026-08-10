use std::process::Command;

use conduit_std_catalog::{supported_nucleus_contracts, supported_nucleus_offers};
use conduitos::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::cli::GlobalOpts;

use super::ConduitosError;

const SCHEMA: &str = "conduit.conduitos/std-gap@1";
const INVENTORY_SCHEMA: &str = "conduit.std/supported-nucleus-inventory@1";
const MAXIMUM_CATALOG_ENTRIES: usize = 64;

#[derive(Serialize)]
struct InventoryEntry {
    kind_id: String,
    contract_revision: String,
    contract: Value,
    canonical_offer: Value,
}

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
struct InventoryDigestBasis<'a> {
    schema: &'static str,
    entries: &'a [InventoryEntry],
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
    catalog_entries: Vec<InventoryEntry>,
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
    let contracts = supported_nucleus_contracts();
    let offers = supported_nucleus_offers();
    if contracts.len() != offers.len() || contracts.len() > MAXIMUM_CATALOG_ENTRIES {
        return Err(ConduitosError::refusal(
            "std-catalog-inventory-out-of-bounds",
            format!(
                "contracts={}, offers={}, maximum={MAXIMUM_CATALOG_ENTRIES}",
                contracts.len(),
                offers.len()
            ),
        ));
    }

    let mut inventory = Vec::with_capacity(contracts.len());
    for (contract, offer) in contracts.into_iter().zip(offers) {
        if contract.kind_id != offer.kind_id
            || contract.inputs != offer.inputs
            || contract.outputs != offer.outputs
            || contract.limits != offer.limits
        {
            return Err(ConduitosError::refusal(
                "std-catalog-contract-offer-mismatch",
                offer.kind_id.as_str(),
            ));
        }
        inventory.push(InventoryEntry {
            kind_id: offer.kind_id.as_str().to_owned(),
            contract_revision: offer.kind_contract_revision.as_str().to_owned(),
            contract: serde_json::to_value(contract).map_err(encoding_error)?,
            canonical_offer: serde_json::to_value(offer).map_err(encoding_error)?,
        });
    }

    let digest = inventory_digest(&inventory)?;

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
        catalog_inventory_schema: INVENTORY_SCHEMA,
        catalog_digest_algorithm: "sha256-canonical-json",
        catalog_digest: digest,
        catalog_entry_count: inventory.len(),
        maximum_catalog_entries: MAXIMUM_CATALOG_ENTRIES,
        catalog_entries: inventory,
        host_profile: host.profile,
        artifact_build: build,
        comparison_key: "exact-kind-id+kind-contract-revision",
        implemented_count,
        missing_count: entries.len() - implemented_count,
        entries,
    })
}

fn inventory_digest(entries: &[InventoryEntry]) -> Result<String, ConduitosError> {
    let digest_bytes = serde_json::to_vec(&InventoryDigestBasis {
        schema: INVENTORY_SCHEMA,
        entries,
    })
    .map_err(encoding_error)?;
    Ok(format!("{:x}", Sha256::digest(digest_bytes)))
}

fn encoding_error(error: serde_json::Error) -> ConduitosError {
    ConduitosError::refusal("std-gap-encoding-failed", error.to_string())
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

    #[test]
    fn current_gap_is_derived_and_exact() {
        let report = build_report("test-build").unwrap();
        assert_eq!(report.catalog_entry_count, 16);
        assert_eq!(report.implemented_count, 5);
        assert_eq!(report.missing_count, 11);
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
        assert_ne!(original, inventory_digest(&report.catalog_entries).unwrap());
    }

    #[test]
    fn digest_excludes_host_build_basis() {
        let first = build_report("first-build").unwrap();
        let second = build_report("second-build").unwrap();
        assert_eq!(first.catalog_digest, second.catalog_digest);
        assert_ne!(first.artifact_build, second.artifact_build);
    }
}
