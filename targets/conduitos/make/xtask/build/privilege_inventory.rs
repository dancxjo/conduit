//! Build-bound dependency evidence, never an isolation receipt.
use std::{fs, process::Command};

use serde::Serialize;

use super::super::{
    profile::Paths,
    report::{ArtifactRole, BuildRecord},
    ConduitosError,
};

#[derive(Serialize)]
struct Inventory<'a> {
    schema: &'static str,
    proof_class: &'static str,
    build: &'a BuildRecord,
    requested_features: &'a [&'a str],
    resolved_dependencies: String,
    working_tree_status: String,
    service_isolation_established: bool,
    interpretation: &'static str,
}

pub(super) fn publish(
    paths: &Paths,
    record: &BuildRecord,
    features: &[&str],
) -> Result<(), ConduitosError> {
    let mut tree = Command::new("cargo");
    tree.current_dir(&paths.root).args([
        "tree",
        "--locked",
        "--package",
        "conduitos",
        "--target",
        record.rust_target,
        "--edges",
        "normal,build",
        "--prefix",
        "indent",
        "--format",
        "{p} features=[{f}]",
    ]);
    if !features.is_empty() {
        tree.args(["--features", &features.join(",")]);
    }
    let resolved_dependencies = output(tree)?;
    let mut status = Command::new("git");
    status
        .current_dir(&paths.root)
        .args(["status", "--porcelain=v1", "--untracked-files=all"]);
    let inventory = Inventory {
        schema: "conduit.conduitos.privilege-inventory/v1",
        proof_class: "build-dependency-observation",
        build: record,
        requested_features: features,
        resolved_dependencies,
        working_tree_status: output(status)?,
        service_isolation_established: false,
        interpretation: "Normal dependencies are potentially included code; build dependencies execute on the build host. This is not a linked-symbol or runtime call graph. The embedded domain image is linked separately. No service-isolation claim follows from this inventory. Dirty files are not identified by base_commit.",
    };
    write(paths.target.join(inventory_name(record)), &inventory)?;
    write(paths.target.join("build.json"), record)
}

fn output(mut command: Command) -> Result<String, ConduitosError> {
    let result = command.output().map_err(|error| {
        ConduitosError::refusal("privilege-inventory-unavailable", error.to_string())
    })?;
    if !result.status.success() {
        return Err(ConduitosError::refusal(
            "privilege-inventory-unavailable",
            String::from_utf8_lossy(&result.stderr).into_owned(),
        ));
    }
    String::from_utf8(result.stdout).map_err(|error| {
        ConduitosError::refusal("privilege-inventory-unavailable", error.to_string())
    })
}

fn write(path: std::path::PathBuf, value: &impl Serialize) -> Result<(), ConduitosError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| ConduitosError::refusal("build-record-failed", error.to_string()))?;
    fs::write(path, bytes)
        .map_err(|error| ConduitosError::refusal("build-record-failed", error.to_string()))
}

// Architecture appliances may replace build.json without using this builder.
// Never leave an unqualified sidecar beside a different ELF or artifact role.
fn inventory_name(record: &BuildRecord) -> String {
    let role = match record.artifact_role {
        ArtifactRole::ProductHost => "product-host",
        ArtifactRole::ArchitectureProofAppliance => "architecture-proof-appliance",
    };
    format!("privilege-inventory-{}-{role}.json", record.elf_sha256)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_artifacts_and_roles_cannot_share_an_inventory_name() {
        let mut record = BuildRecord {
            schema: "conduit.conduitos.build/v2",
            artifact_role: ArtifactRole::ProductHost,
            base_commit: "source".into(),
            architecture: "x86_64",
            rust_target: "x86_64-unknown-none",
            limine_crate: "0.5.0",
            elf_sha256: "a".repeat(64),
        };
        let product = inventory_name(&record);
        record.artifact_role = ArtifactRole::ArchitectureProofAppliance;
        let appliance = inventory_name(&record);
        assert_ne!(product, appliance);
        record.elf_sha256 = "b".repeat(64);
        assert_ne!(appliance, inventory_name(&record));
        assert!(!product.contains('/'));
    }
}
