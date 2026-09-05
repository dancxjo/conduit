//! Compatibility entrance for the authoritative ConduitOS catalog profile gap.
//!
//! This command deliberately consumes the same exact profile advertisement as
//! `cargo xtask catalog gap --host conduitos`; it must never reconstruct a
//! second, smaller implementation inventory from the legacy fixed HostOffer.

use std::process::Command;

use serde::Serialize;

use crate::{cli::GlobalOpts, commands::catalog};

use super::ConduitosError;

const SCHEMA: &str = "conduit.conduitos/std-gap@3";

#[derive(Serialize)]
struct HostCapability {
    capability_id: String,
    kind_id: String,
    contract_revision: String,
    implementation: String,
    execution_profile: String,
    artifact: String,
    host_operations: Vec<String>,
    resources: Vec<String>,
}

#[derive(Serialize)]
struct GapEntry {
    kind_id: String,
    contract_revision: String,
    classification: catalog::GapClassification,
    realization_mode: &'static str,
    reason_code: Option<&'static str>,
    required_host_operations: Vec<String>,
    required_resources: Vec<String>,
    required_bases: Vec<String>,
    unsatisfied_prerequisites: Vec<String>,
    machine_specific: bool,
    host_capability: Option<HostCapability>,
}

#[derive(Serialize)]
struct StdGapReport {
    schema: &'static str,
    catalog_basis: &'static str,
    catalog_inventory_schema: &'static str,
    catalog_digest_algorithm: &'static str,
    catalog_digest: String,
    catalog_entry_count: usize,
    maximum_catalog_entries: usize,
    catalog_entries: Vec<catalog::inventory::InventoryEntry>,
    host_profile: String,
    artifact_build: String,
    comparison_key: &'static str,
    profile_basis: &'static str,
    classification_vocabulary: [catalog::GapClassification; 7],
    implemented_count: usize,
    missing_count: usize,
    entries: Vec<GapEntry>,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        println!("derive the canonical inventory and authoritative ConduitOS profile advertisement; compare exact kind_id + contract_revision");
        return Ok(());
    }
    let report = build_report()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| {
            ConduitosError::refusal("std-gap-encoding-failed", error.to_string())
        })?
    );
    Ok(())
}

fn build_report() -> Result<StdGapReport, ConduitosError> {
    let inventory = catalog::inventory::derive().map_err(|error| {
        ConduitosError::refusal("semantic-catalog-inventory-invalid", error.to_string())
    })?;
    let host = catalog::profiles::conduitos_advertisement()
        .map_err(|error| ConduitosError::refusal("conduitos-profile-invalid", error.to_string()))?;
    let recursive = catalog::recursive::derive().map_err(|error| {
        ConduitosError::refusal("conduitos-recursive-profile-invalid", error.to_string())
    })?;
    let entries = inventory
        .entries
        .iter()
        .map(|entry| {
            let capability = host.capabilities.iter().find(|candidate| {
                candidate.kind_id.as_str() == entry.kind_id
                    && candidate.kind_contract_revision.as_str() == entry.contract_revision
            });
            let recursive_coverage = recursive.iter().any(|coverage| {
                coverage.host_profile == host.profile.as_str()
                    && coverage.kind_id == entry.kind_id
                    && coverage.contract_revision == entry.contract_revision
            });
            let prerequisites = catalog::prerequisites::classify(
                &host,
                entry,
                capability.is_some() || recursive_coverage,
            );
            GapEntry {
                kind_id: entry.kind_id.clone(),
                contract_revision: entry.contract_revision.clone(),
                classification: prerequisites.classification,
                realization_mode: if capability.is_some() {
                    "direct"
                } else if recursive_coverage {
                    "recursive"
                } else {
                    "none"
                },
                reason_code: prerequisites.reason_code,
                required_host_operations: prerequisites.required_host_operations,
                required_resources: prerequisites.required_resources,
                required_bases: prerequisites.required_bases,
                unsatisfied_prerequisites: prerequisites.unsatisfied,
                machine_specific: prerequisites.machine_specific,
                host_capability: capability.map(|capability| HostCapability {
                    capability_id: capability.capability_id.as_str().to_owned(),
                    kind_id: capability.kind_id.as_str().to_owned(),
                    contract_revision: capability.kind_contract_revision.as_str().to_owned(),
                    implementation: capability
                        .implementation
                        .implementation_id
                        .as_str()
                        .to_owned(),
                    execution_profile: capability
                        .implementation
                        .execution_profile_id
                        .as_str()
                        .to_owned(),
                    artifact: capability.implementation.artifact_id.as_str().to_owned(),
                    host_operations: capability
                        .host_operations
                        .iter()
                        .map(|requirement| requirement.contract_id.as_str().to_owned())
                        .collect(),
                    resources: capability
                        .resource_requirements
                        .iter()
                        .map(|requirement| requirement.class_id.as_str().to_owned())
                        .collect(),
                }),
            }
        })
        .collect::<Vec<_>>();
    let implemented_count = entries
        .iter()
        .filter(|entry| entry.classification == catalog::GapClassification::Implemented)
        .count();
    Ok(StdGapReport {
        schema: SCHEMA,
        catalog_basis:
            "conduit_semantic_catalog::supported_nucleus_contracts()+conduit_std_host::supported_nucleus_offers()",
        catalog_inventory_schema: catalog::inventory::SCHEMA,
        catalog_digest_algorithm: "sha256-canonical-json",
        catalog_digest: inventory.digest,
        catalog_entry_count: inventory.entries.len(),
        maximum_catalog_entries: catalog::inventory::MAXIMUM_ENTRIES,
        catalog_entries: inventory.entries,
        host_profile: host.profile.as_str().to_owned(),
        artifact_build: git_head()?,
        comparison_key: "exact-kind-id+kind-contract-revision",
        profile_basis: "xtask::commands::catalog::profiles::conduitos_advertisement",
        classification_vocabulary: [
            catalog::GapClassification::Implemented,
            catalog::GapClassification::PortableImplementationMissing,
            catalog::GapClassification::MissingHostOperation,
            catalog::GapClassification::MissingResource,
            catalog::GapClassification::MissingBase,
            catalog::GapClassification::UnsupportedOnThisMachine,
            catalog::GapClassification::DeliberatelyNotApplicable,
        ],
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

    #[test]
    fn compatibility_report_uses_the_authoritative_profile_inventory() {
        let report = build_report().unwrap();
        assert_eq!(report.catalog_entry_count, 60);
        assert_eq!(
            report.implemented_count + report.missing_count,
            report.catalog_entry_count
        );
        assert_eq!(report.implemented_count, 57);
        assert_eq!(report.missing_count, 3);
        let state_select = report
            .entries
            .iter()
            .find(|entry| entry.kind_id == "state/select")
            .unwrap();
        assert_eq!(
            state_select.classification,
            catalog::GapClassification::Implemented
        );
        assert!(state_select.unsatisfied_prerequisites.is_empty());
        let file_copy = report
            .entries
            .iter()
            .find(|entry| entry.kind_id == "file/copy")
            .unwrap();
        assert_eq!(
            file_copy.classification,
            catalog::GapClassification::MissingBase
        );
        assert_eq!(file_copy.unsatisfied_prerequisites, ["base:storage"]);
        for kind in [
            "layout/inset",
            "presentation/bool",
            "logic/not",
            "logic/compare",
            "logic/select",
            "math/clamp",
            "robotics/observe-bump",
            "robotics/observe-imu",
            "robotics/observe-range",
            "robotics/observe-odometry",
            "robotics/observe-battery",
            "robotics/velocity-intent",
            "robotics/drive-differential",
        ] {
            assert!(report.entries.iter().any(|entry| {
                entry.kind_id == kind
                    && entry.classification == catalog::GapClassification::Implemented
                    && entry.host_capability.is_some()
            }));
        }
        assert!(report.entries.iter().any(|entry| {
            entry.kind_id == "patchbay/gear-face"
                && entry.classification == catalog::GapClassification::Implemented
                && entry.realization_mode == "recursive"
                && entry.host_capability.is_none()
        }));
    }
}
