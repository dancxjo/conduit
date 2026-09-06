use super::spec::ProofSpec;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(super) fn fingerprint(root: &Path, tree: &str, spec: &ProofSpec) -> Result<String, String> {
    let mut entries = BTreeSet::new();
    collect_entries(root, tree, "input", spec.inputs, &mut entries)?;
    collect_entries(
        root,
        tree,
        "implementation",
        spec.implementation_inputs,
        &mut entries,
    )?;
    let mut hash = Sha256::new();
    hash_field(&mut hash, "proof", spec.id.as_bytes());
    hash_field(&mut hash, "contract", &spec.contract_version.to_be_bytes());
    hash_field(&mut hash, "environment", spec.environment.as_bytes());
    for entry in entries {
        hash_field(&mut hash, "git", entry.as_bytes());
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn collect_entries(
    root: &Path,
    tree: &str,
    class: &str,
    paths: &[&str],
    entries: &mut BTreeSet<String>,
) -> Result<(), String> {
    let mut command = super::git_command(root);
    command.args(["ls-tree", "-r", "-z", tree, "--"]);
    command.args(paths);
    let output = command
        .output()
        .map_err(|error| format!("run git ls-tree: {error}"))?;
    if !output.status.success() {
        return Err(super::command_error("git ls-tree", &output));
    }
    for raw in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let text =
            std::str::from_utf8(raw).map_err(|_| "git ls-tree emitted non-UTF-8".to_owned())?;
        let (metadata, path) = text
            .split_once('\t')
            .ok_or_else(|| "malformed git ls-tree entry".to_owned())?;
        entries.insert(format!("{class}\0{path}\0{metadata}"));
    }
    Ok(())
}

pub(super) fn proof_key(
    spec: &ProofSpec,
    input_digest: &str,
    artifacts: &BTreeMap<String, String>,
) -> String {
    let mut hash = Sha256::new();
    hash_field(&mut hash, "proof", spec.id.as_bytes());
    hash_field(&mut hash, "contract", &spec.contract_version.to_be_bytes());
    hash_field(&mut hash, "inputs", input_digest.as_bytes());
    hash_field(&mut hash, "environment", spec.environment.as_bytes());
    for (name, digest) in artifacts {
        hash_field(&mut hash, "artifact-name", name.as_bytes());
        hash_field(&mut hash, "artifact-digest", digest.as_bytes());
    }
    format!("sha256:{:x}", hash.finalize())
}

fn hash_field(hash: &mut Sha256, label: &str, value: &[u8]) {
    hash.update((label.len() as u64).to_be_bytes());
    hash.update(label.as_bytes());
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}
