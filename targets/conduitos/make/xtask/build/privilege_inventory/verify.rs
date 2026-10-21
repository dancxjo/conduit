//! A verifier reads exact sealed artifacts. It never builds, boots or claims isolation.
use super::super::super::{profile::Paths, ConduitosArch, ConduitosError};
use super::{digest, elf, invalid, source};
use crate::cli::GlobalOpts;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Sealed normal host build directory containing build-manifest.json and kernel.
    #[arg(long)]
    pub build_dir: PathBuf,
    /// Common builder evidence directory containing the exact inventory, map and build.json.
    #[arg(long)]
    pub inventory_dir: PathBuf,
    /// New verification receipt; never overwrites existing evidence.
    #[arg(long)]
    pub output: PathBuf,
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    if fs::metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len()
        > 512 * 1024 * 1024
    {
        return Err("inventory artifact exceeds 512 MiB observation bound".into());
    }
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}
fn document(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&read(path)?).map_err(|e| format!("{}: {e}", path.display()))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing {key}"))
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
fn basename(value: &str) -> bool {
    Path::new(value).file_name().and_then(|s| s.to_str()) == Some(value)
}
/// Deliberately separate from filesystem selection so replacement/tampering has
/// deterministic negative controls and never falls back to an older sidecar.
fn associate(
    manifest: &Value,
    build: &Value,
    inventory: &Value,
    kernel: &[u8],
    map: &[u8],
) -> Result<(), String> {
    let permitted = [
        "schema",
        "proof_class",
        "build",
        "requested_features",
        "resolved_dependencies",
        "root_link_map",
        "root_link_map_sha256",
        "working_tree_status",
        "service_isolation_established",
        "responsibilities",
        "linked_images",
        "linked_responsibilities",
        "interpretation",
    ];
    if inventory
        .as_object()
        .is_none_or(|fields| fields.keys().any(|key| !permitted.contains(&key.as_str())))
    {
        return Err(
            "unknown inventory claim; source/link evidence cannot acquire execution authority"
                .into(),
        );
    }
    if manifest["schema"] != "conduit.host/target-build-manifest@3"
        || manifest["artifact_role"] != "product-host"
        || manifest["target"] != "conduitos/x86_64/pc"
    {
        return Err("expected sealed x86_64 graphical product BUILD".into());
    }
    if inventory["schema"] != "conduit.conduitos.privilege-inventory/v2"
        || inventory["proof_class"] != "build-dependency-and-link-observation"
        || inventory["service_isolation_established"] != false
    {
        return Err("inventory cannot establish service isolation or another proof class".into());
    }
    if inventory["working_tree_status"] != ""
        || inventory["build"] != *build
        || build["schema"] != "conduit.conduitos.build/v2"
        || build["artifact_role"] != "product-host"
        || build["architecture"] != "x86_64"
        || build["rust_target"] != "x86_64-unknown-none"
    {
        return Err("dirty, replaced or wrong-role build record".into());
    }
    let source = text(manifest, "source_identity")?;
    let sha = text(&manifest["kernel"], "sha256")?;
    if !hex(source, 40)
        || !hex(sha, 64)
        || build["base_commit"] != source
        || build["elf_sha256"] != sha
        || digest(kernel) != sha
    {
        return Err("source or sealed kernel identity differs from inventory".into());
    }
    if inventory["requested_features"] != json!(["native-compositor"]) {
        return Err("unsupported requested graphical feature profile".into());
    }
    let dependencies = text(inventory, "resolved_dependencies")?;
    let root = dependencies
        .lines()
        .next()
        .ok_or("missing feature closure")?;
    if !root.starts_with("conduitos v")
        || !root.ends_with("features=[default,native-compositor,native-owner-network]")
    {
        return Err("unexpected resolved graphical feature closure; review inventory".into());
    }
    if digest(map) != text(inventory, "root_link_map_sha256")? || map.is_empty() {
        return Err("missing or changed same-invocation Root link map".into());
    }
    if serde_json::to_value(super::linked_responsibilities(map)).map_err(|e| e.to_string())?
        != inventory["linked_responsibilities"]
    {
        return Err("linked responsibility observations differ from map".into());
    }
    let expected_map = format!("privilege-inventory-{sha}-product-host.map");
    if inventory["root_link_map"] != expected_map {
        return Err("map belongs to another artifact or role".into());
    }
    if serde_json::to_value(elf::observe(kernel)?).map_err(|e| e.to_string())?
        != inventory["linked_images"]
    {
        return Err("Root/embedded ELF observations do not match exact kernel".into());
    }
    Ok(())
}
fn match_closure(inventory: &Value, observed: &str) -> Result<(), String> {
    if text(inventory, "resolved_dependencies")? != observed {
        return Err("complete locked target-specific dependency/feature closure changed".into());
    }
    Ok(())
}
pub fn execute(args: Args, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(invalid("verification needs actual artifacts".into()));
    }
    let result = verify(&args).map_err(invalid)?;
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)
        .map_err(|e| invalid(e.to_string()))?;
    serde_json::to_writer_pretty(output, &result).map_err(|e| invalid(e.to_string()))?;
    println!(
        "Verified graphical privilege inventory: {}",
        args.output.display()
    );
    Ok(())
}
fn verify(args: &Args) -> Result<Value, String> {
    let manifest = document(&args.build_dir.join("build-manifest.json"))?;
    let kernel_name = text(&manifest["kernel"], "file")?;
    if !basename(kernel_name) {
        return Err("kernel filename escapes sealed build".into());
    }
    let kernel = read(&args.build_dir.join(kernel_name))?;
    let sha = text(&manifest["kernel"], "sha256")?;
    if !hex(sha, 64) {
        return Err("invalid kernel digest".into());
    }
    let inventory_path = args
        .inventory_dir
        .join(format!("privilege-inventory-{sha}-product-host.json"));
    let inventory = document(&inventory_path)?;
    let map_name = text(&inventory, "root_link_map")?;
    if !basename(map_name) {
        return Err("map filename escapes inventory directory".into());
    }
    let map = read(&args.inventory_dir.join(map_name))?;
    let build = document(&args.inventory_dir.join("build.json"))?;
    associate(&manifest, &build, &inventory, &kernel, &map)?;
    let paths = Paths::new(ConduitosArch::X86_64).map_err(|e| format!("{e:?}"))?;
    let source_id = text(&manifest, "source_identity")?;
    for (arguments, expected) in [
        (["rev-parse", "HEAD", ""], source_id),
        (["status", "--porcelain=v1", "--untracked-files=all"], ""),
    ] {
        let mut command = std::process::Command::new("git");
        command
            .current_dir(&paths.root)
            .args(arguments.into_iter().filter(|s| !s.is_empty()));
        if super::output(command)
            .map_err(|e| format!("{e:?}"))?
            .trim_end()
            != expected
        {
            return Err(
                "full dependency verification requires the matching clean committed checkout"
                    .into(),
            );
        }
    }
    let observed =
        super::dependency_closure(&paths.root, "x86_64-unknown-none", &["native-compositor"])
            .map_err(|e| format!("{e:?}"))?;
    match_closure(&inventory, &observed)?;
    let responsibilities = source::observe(&paths.root, Some(text(&manifest, "source_identity")?))?;
    if serde_json::to_value(responsibilities).map_err(|e| e.to_string())?
        != inventory["responsibilities"]
    {
        return Err("reviewed responsibility inventory differs from exact committed source".into());
    }
    Ok(
        json!({"schema":"conduit.conduitos/verified-privilege-inventory@1", "source":manifest["source_identity"], "build_id":manifest["build_id"], "kernel_sha256":sha, "inventory_sha256":digest(&read(&inventory_path)?), "root_link_map_sha256":inventory["root_link_map_sha256"], "dependency_closure_sha256":digest(observed.as_bytes()), "dependency_closure_verification":"recomputed locked offline target-specific Cargo tree at matching clean Source", "linked_responsibilities":inventory["linked_responsibilities"], "linked_images":inventory["linked_images"], "responsibilities":inventory["responsibilities"], "proof_class":"exact-source-and-linked-image-inventory", "service_isolation_established":false, "execution_established":false, "physical_or_dma_isolation_established":false, "limits":"Source anchors are reviewed reachability/authority observations, not an exhaustive computed call graph. ELF segments are linked bytes, not installed mappings or executed service entries. No isolation claim follows from module boundaries, feature closure or image presence."}),
    )
}
#[cfg(test)]
mod tests;
