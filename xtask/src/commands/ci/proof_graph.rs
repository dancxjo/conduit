use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PLAN_SCHEMA: &str = "conduit.ci.reconciliation-plan/v1";

#[path = "proof_graph/receipt.rs"]
mod receipt;
#[path = "proof_graph/spec.rs"]
mod spec;
use receipt::{load_receipts, ProofReceipt, ReceiptLoad, RECEIPT_SCHEMA};
use spec::{Applicability, ProofKind, ProofSpec, PROOFS};

#[derive(Debug, Serialize)]
struct ProofPlan {
    proof_id: String,
    proof_contract_version: u32,
    kind: ProofKind,
    applicability: Applicability,
    input_digest: String,
    proof_key: String,
    disposition: &'static str,
    reason: String,
    consumed_artifacts: Vec<String>,
    environment: String,
    command: String,
}

#[derive(Debug, Serialize)]
struct Plan {
    schema: &'static str,
    mode: &'static str,
    candidate_sha: String,
    candidate_tree: String,
    base_sha: Option<String>,
    integration_tree: Option<String>,
    effective_merge_base_sha: Option<String>,
    effective_merge_base_tree: Option<String>,
    merge_base_method: Option<&'static str>,
    integration_status: &'static str,
    candidate_evidence_status: &'static str,
    proofs: Vec<ProofPlan>,
    inherited: usize,
    execute: usize,
}

pub(super) fn candidate(
    head: &str,
    receipt_paths: &[PathBuf],
    json_out: Option<&Path>,
    summary_out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let candidate_sha = resolve_commit(&root, head)?;
    let tree = resolve_tree(&root, &candidate_sha)?;
    validate_registry_paths(&root, &tree)?;
    let receipts = load_receipts(receipt_paths);
    let plan = build_plan(
        &root,
        PlanContext {
            mode: "candidate",
            candidate_sha,
            candidate_tree: tree.clone(),
            base_sha: None,
            integration_tree: Some(tree),
            effective_merge_base_sha: None,
            effective_merge_base_tree: None,
            merge_base_method: None,
            integration_status: "not-requested",
        },
        &receipts,
    )?;
    emit(&plan, json_out, summary_out)
}

pub(super) fn reconcile(
    base: &str,
    head: &str,
    receipt_paths: &[PathBuf],
    json_out: Option<&Path>,
    summary_out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let base_sha = resolve_commit(&root, base)?;
    let candidate_sha = resolve_commit(&root, head)?;
    let candidate_tree = resolve_tree(&root, &candidate_sha)?;
    validate_registry_paths(&root, &candidate_tree)?;
    let receipts = load_receipts(receipt_paths);
    let candidate_evidence_status = evidence_status(&root, &candidate_tree, &receipts)?;
    let integration = super::integration::resolve(&root, &base_sha, &candidate_sha)?;
    let plan = if integration.status == "conflict" {
        Plan {
            schema: PLAN_SCHEMA,
            mode: "integration",
            candidate_sha,
            candidate_tree,
            base_sha: Some(base_sha),
            integration_tree: None,
            effective_merge_base_sha: None,
            effective_merge_base_tree: None,
            merge_base_method: Some("none"),
            integration_status: "conflict",
            candidate_evidence_status,
            proofs: Vec::new(),
            inherited: 0,
            execute: 0,
        }
    } else {
        build_plan(
            &root,
            PlanContext {
                mode: "integration",
                candidate_sha,
                candidate_tree,
                base_sha: Some(base_sha),
                integration_tree: integration.integration_tree,
                effective_merge_base_sha: integration.effective_merge_base_sha,
                effective_merge_base_tree: integration.effective_merge_base_tree,
                merge_base_method: Some(integration.merge_base_method),
                integration_status: "clean",
            },
            &receipts,
        )?
    };
    emit(&plan, json_out, summary_out)
}

pub(super) fn attest_success(
    head: &str,
    proof_id: &str,
    evidence: &[String],
    out: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if evidence.is_empty() || evidence.iter().any(|item| item.is_empty()) {
        return Err("a successful proof receipt requires non-empty evidence".into());
    }
    let spec = PROOFS
        .iter()
        .find(|spec| spec.id == proof_id)
        .ok_or_else(|| format!("unknown proof id {proof_id}"))?;
    if !spec.consumed_artifacts.is_empty() {
        return Err(format!(
            "proof {proof_id} consumes artifacts; this receipt command requires their exact digests"
        )
        .into());
    }
    let root = crate::workspace::workspace_root()?;
    let candidate_sha = resolve_commit(&root, head)?;
    let source_tree = resolve_tree(&root, &candidate_sha)?;
    validate_registry_paths(&root, &source_tree)?;
    let input_digest = fingerprint(&root, &source_tree, spec)?;
    let receipt = ProofReceipt {
        schema: RECEIPT_SCHEMA.to_owned(),
        proof_id: proof_id.to_owned(),
        proof_contract_version: spec.contract_version,
        candidate_sha,
        source_tree,
        proof_key: proof_key(spec, &input_digest, &BTreeMap::new()),
        input_digest,
        result: "success".to_owned(),
        artifact_digests: BTreeMap::new(),
        evidence: evidence.to_vec(),
    };
    write_parent(out)?;
    fs::write(
        out,
        format!("{}\n", serde_json::to_string_pretty(&receipt)?),
    )?;
    Ok(())
}

struct PlanContext {
    mode: &'static str,
    candidate_sha: String,
    candidate_tree: String,
    base_sha: Option<String>,
    integration_tree: Option<String>,
    effective_merge_base_sha: Option<String>,
    effective_merge_base_tree: Option<String>,
    merge_base_method: Option<&'static str>,
    integration_status: &'static str,
}

fn build_plan(
    root: &Path,
    context: PlanContext,
    receipts: &[ReceiptLoad],
) -> Result<Plan, Box<dyn std::error::Error>> {
    let proof_tree = context
        .integration_tree
        .as_deref()
        .unwrap_or(&context.candidate_tree);
    validate_registry_paths(root, proof_tree)?;
    let mut proofs = Vec::new();
    for spec in PROOFS {
        let input_digest = fingerprint(root, proof_tree, spec)?;
        let proof_key = proof_key(spec, &input_digest, &BTreeMap::new());
        let inherited = receipts.iter().any(|loaded| {
            matches!(loaded, ReceiptLoad::Valid(receipt) if receipt_matches(receipt, spec, &input_digest, &proof_key))
        });
        proofs.push(ProofPlan {
            proof_id: spec.id.to_owned(),
            proof_contract_version: spec.contract_version,
            kind: spec.kind,
            applicability: spec.applicability,
            input_digest,
            proof_key,
            disposition: if inherited { "inherited" } else { "execute" },
            reason: if inherited {
                "exact successful proof key is unchanged".to_owned()
            } else if receipts.is_empty() {
                "no prior receipt was supplied".to_owned()
            } else {
                "no complete recognized successful receipt has this exact proof key".to_owned()
            },
            consumed_artifacts: spec
                .consumed_artifacts
                .iter()
                .map(|v| (*v).to_owned())
                .collect(),
            environment: spec.environment.to_owned(),
            command: spec.command.to_owned(),
        });
    }
    let inherited = proofs
        .iter()
        .filter(|proof| proof.disposition == "inherited")
        .count();
    let execute = proofs.len() - inherited;
    let candidate_evidence_status = evidence_status(root, &context.candidate_tree, receipts)?;
    Ok(Plan {
        schema: PLAN_SCHEMA,
        mode: context.mode,
        candidate_sha: context.candidate_sha,
        candidate_tree: context.candidate_tree,
        base_sha: context.base_sha,
        integration_tree: context.integration_tree,
        effective_merge_base_sha: context.effective_merge_base_sha,
        effective_merge_base_tree: context.effective_merge_base_tree,
        merge_base_method: context.merge_base_method,
        integration_status: context.integration_status,
        candidate_evidence_status,
        proofs,
        inherited,
        execute,
    })
}

#[cfg(test)]
enum MergeTree {
    Clean(String),
    Conflict,
}

#[cfg(test)]
fn merge_tree(root: &Path, base: &str, head: &str) -> Result<MergeTree, String> {
    let integration = super::integration::resolve(root, base, head)?;
    Ok(match integration.integration_tree {
        Some(tree) => MergeTree::Clean(tree),
        None => MergeTree::Conflict,
    })
}

fn fingerprint(root: &Path, tree: &str, spec: &ProofSpec) -> Result<String, String> {
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

fn validate_registry_paths(root: &Path, tree: &str) -> Result<(), String> {
    for spec in PROOFS {
        for (class, paths) in [
            ("input", spec.inputs),
            ("implementation", spec.implementation_inputs),
        ] {
            for path in paths {
                let object = format!("{tree}:{path}");
                let output = git_command(root)
                    .args(["cat-file", "-e", &object])
                    .output()
                    .map_err(|error| format!("run git cat-file: {error}"))?;
                if !output.status.success() {
                    return Err(format!(
                        "proof {} required {class} path {path} is absent from tree {tree}",
                        spec.id
                    ));
                }
            }
        }
    }
    Ok(())
}

fn collect_entries(
    root: &Path,
    tree: &str,
    class: &str,
    paths: &[&str],
    entries: &mut BTreeSet<String>,
) -> Result<(), String> {
    let mut command = git_command(root);
    command.args(["ls-tree", "-r", "-z", tree, "--"]);
    command.args(paths);
    let output = command
        .output()
        .map_err(|error| format!("run git ls-tree: {error}"))?;
    if !output.status.success() {
        return Err(command_error("git ls-tree", &output));
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

fn proof_key(spec: &ProofSpec, input_digest: &str, artifacts: &BTreeMap<String, String>) -> String {
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

fn receipt_matches(
    receipt: &ProofReceipt,
    spec: &ProofSpec,
    input_digest: &str,
    key: &str,
) -> bool {
    receipt::receipt_matches(
        receipt,
        spec.id,
        spec.contract_version,
        spec.consumed_artifacts,
        input_digest,
        key,
    )
}

fn evidence_status(
    root: &Path,
    tree: &str,
    receipts: &[ReceiptLoad],
) -> Result<&'static str, String> {
    let mut inherited = 0;
    for spec in PROOFS {
        let input_digest = fingerprint(root, tree, spec)?;
        let key = proof_key(spec, &input_digest, &BTreeMap::new());
        if receipts.iter().any(|loaded| {
            matches!(loaded, ReceiptLoad::Valid(receipt) if receipt_matches(receipt, spec, &input_digest, &key))
        }) {
            inherited += 1;
        }
    }
    Ok(if inherited == PROOFS.len() {
        "pass"
    } else if inherited > 0 {
        "partial"
    } else {
        "unproven"
    })
}

fn hash_field(hash: &mut Sha256, label: &str, value: &[u8]) {
    hash.update((label.len() as u64).to_be_bytes());
    hash.update(label.as_bytes());
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

fn resolve_commit(root: &Path, revision: &str) -> Result<String, String> {
    git_text(
        root,
        &["rev-parse", "--verify", &format!("{revision}^{{commit}}")],
    )
}

fn resolve_tree(root: &Path, commit: &str) -> Result<String, String> {
    git_text(
        root,
        &["rev-parse", "--verify", &format!("{commit}^{{tree}}")],
    )
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_command(root)
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(command_error(&format!("git {}", args.join(" ")), &output));
    }
    Ok(String::from_utf8(output.stdout)
        .map_err(|_| "git emitted non-UTF-8".to_owned())?
        .trim()
        .to_owned())
}

fn git_command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .arg("-c")
        .arg(format!("safe.directory={}", root.display()));
    command
}

fn command_error(label: &str, output: &Output) -> String {
    format!(
        "{label} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn emit(
    plan: &Plan,
    json_out: Option<&Path>,
    summary_out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = format!("{}\n", serde_json::to_string_pretty(plan)?);
    if let Some(path) = json_out {
        write_parent(path)?;
        fs::write(path, &json)?;
    } else {
        print!("{json}");
    }
    if let Some(path) = summary_out {
        write_parent(path)?;
        fs::write(path, summary(plan))?;
    }
    Ok(())
}

fn summary(plan: &Plan) -> String {
    let mut text = format!(
        "## CI {} plan\n\n- candidate: `{}`\n- candidate tree: `{}`\n- candidate evidence: **{}**\n- integration: **{}**\n- inherited: **{}**\n- execute: **{}**\n",
        plan.mode,
        plan.candidate_sha,
        plan.candidate_tree,
        plan.candidate_evidence_status,
        plan.integration_status,
        plan.inherited,
        plan.execute
    );
    if let Some(base) = &plan.base_sha {
        text.push_str(&format!("- base: `{base}`\n"));
    }
    if let Some(tree) = &plan.integration_tree {
        text.push_str(&format!("- integration tree: `{tree}`\n"));
    }
    if let Some(base) = &plan.effective_merge_base_sha {
        text.push_str(&format!("- effective merge base: `{base}`\n"));
    }
    if let Some(tree) = &plan.effective_merge_base_tree {
        text.push_str(&format!("- effective merge-base tree: `{tree}`\n"));
    }
    if let Some(method) = plan.merge_base_method {
        text.push_str(&format!("- merge-base method: **{method}**\n"));
    }
    text.push_str("\n| Proof | Disposition | Reason |\n|---|---|---|\n");
    for proof in &plan.proofs {
        text.push_str(&format!(
            "| `{}` | **{}** | {} |\n",
            proof.proof_id, proof.disposition, proof.reason
        ));
    }
    text
}

fn write_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "proof_graph/tests.rs"]
mod tests;
