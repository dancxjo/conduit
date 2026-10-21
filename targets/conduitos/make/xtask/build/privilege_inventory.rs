//! Build-bound dependency and linker evidence, never an isolation receipt.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Serialize;

use super::super::{
    profile::Paths,
    report::{sha256_file, ArtifactRole, BuildRecord},
    ConduitosError,
};

#[derive(Serialize)]
struct Inventory<'a> {
    schema: &'static str,
    proof_class: &'static str,
    build: &'a BuildRecord,
    requested_features: &'a [&'a str],
    resolved_dependencies: String,
    root_link_map: String,
    root_link_map_sha256: String,
    working_tree_status: String,
    service_isolation_established: bool,
    interpretation: &'static str,
}

/// Apply to the final binary only, rather than dependencies or the separately
/// built domain. Cargo passes this single argument without shell word splitting.
pub(super) fn configure_link_map(
    command: &mut Command,
    paths: &Paths,
    target: &str,
    binary: &str,
) -> PathBuf {
    // Keep the map beside Cargo's compiled binary. Cleaning/replacing the proof
    // output directory must not strand an otherwise cached successful build.
    let map = paths.built_binary(target, binary).with_extension("map");
    command.args(["--", "-C"]);
    command.arg(format!("link-arg=-Map={}", map.display()));
    map
}

pub(super) fn publish(
    paths: &Paths,
    record: &BuildRecord,
    features: &[&str],
    map: &Path,
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
    let map_name = inventory_name(record).replace(".json", ".map");
    fs::copy(map, paths.target.join(&map_name)).map_err(|error| {
        ConduitosError::refusal("privilege-inventory-unavailable", error.to_string())
    })?;
    let inventory = Inventory {
        schema: "conduit.conduitos.privilege-inventory/v1",
        proof_class: "build-dependency-and-link-observation",
        build: record,
        requested_features: features,
        resolved_dependencies,
        root_link_map: map_name,
        root_link_map_sha256: sha256_file(map)?,
        working_tree_status: output(status)?,
        service_isolation_established: false,
        interpretation: "Normal dependencies are potentially included code; build dependencies execute on the build host. The same linker invocation retains a Root symbol map. Dependency membership and linker symbols do not establish runtime call reachability. The embedded domain image is linked separately. No service-isolation claim follows from this inventory. Dirty files are not identified by base_commit.",
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
    fn final_link_map_survives_evidence_root_replacement_and_space_in_paths() {
        let mut paths = Paths::new(super::super::super::ConduitosArch::X86_64).unwrap();
        paths.cargo_target = PathBuf::from("/tmp/compiled artifacts");
        let mut first = Command::new("cargo");
        let map = configure_link_map(&mut first, &paths, "x86_64-unknown-none", "conduitos");
        let args: Vec<_> = first.get_args().collect();
        assert_eq!(args.len(), 3);
        assert_eq!(
            args[2],
            "link-arg=-Map=/tmp/compiled artifacts/x86_64-unknown-none/release/conduitos.map"
        );
        paths.target = PathBuf::from("/tmp/new proof output");
        let mut second = Command::new("cargo");
        assert_eq!(
            map,
            configure_link_map(&mut second, &paths, "x86_64-unknown-none", "conduitos")
        );
    }

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
