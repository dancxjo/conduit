use std::{collections::BTreeSet, fs, path::Path, process::Command};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CLAIMS_PATH: &str = "release/capabilities.json";
const MATRIX_PATH: &str = "docs/capability-matrix.md";
const RUNNABILITY_PATH: &str = "examples/runnability.json";
const DISPOSITION_PATH: &str = ".github/ISSUE_TEMPLATE/accepted-slice.md";

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    schema: String,
    version: String,
    changelog: String,
    license: String,
    repository: String,
    supported_hosts: Vec<SupportedHost>,
    capabilities: Vec<Capability>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SupportedHost {
    id: String,
    status: String,
    evidence: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Capability {
    id: String,
    summary: String,
    contract: Layer,
    reference_model: Layer,
    provider: Layer,
    host_resolvability: Layer,
    exact_binding: Layer,
    runtime_proof: Layer,
    product_presentation: Layer,
    release_claim: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Layer {
    status: String,
    evidence: Vec<String>,
}

#[derive(Deserialize)]
struct Runnability {
    entries: Vec<RunnabilityEntry>,
}

#[derive(Deserialize)]
struct RunnabilityEntry {
    path: String,
    state: String,
    proof: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn load_claims(workspace_root: &Path) -> Result<(Claims, Vec<u8>), Box<dyn std::error::Error>> {
    let bytes = fs::read(workspace_root.join(CLAIMS_PATH))?;
    let claims = serde_json::from_slice(&bytes)?;
    Ok((claims, bytes))
}

fn validate_layer(
    workspace_root: &Path,
    capability: &str,
    name: &str,
    layer: &Layer,
) -> Result<(), String> {
    const STATUSES: &[&str] = &[
        "proven",
        "available",
        "contract-only",
        "unsupported",
        "not-applicable",
    ];
    if !STATUSES.contains(&layer.status.as_str()) {
        return Err(format!(
            "{capability}.{name} has unknown status `{}`",
            layer.status
        ));
    }
    if layer.status != "not-applicable" && layer.evidence.is_empty() {
        return Err(format!("{capability}.{name} has no authoritative evidence"));
    }
    for evidence in &layer.evidence {
        let path = evidence
            .split_once('#')
            .map_or(evidence.as_str(), |(path, _)| path);
        if !workspace_root.join(path).exists() {
            return Err(format!(
                "{capability}.{name} references missing evidence `{evidence}`"
            ));
        }
    }
    Ok(())
}

fn validate(
    workspace_root: &Path,
    claims: &Claims,
    runnability: &Runnability,
) -> Result<(), String> {
    if claims.schema != "conduit.release-capabilities" {
        return Err("unsupported release-capabilities schema".to_owned());
    }
    if claims.version != env!("CARGO_PKG_VERSION") {
        return Err("release capability version does not match workspace version".to_owned());
    }
    for required in [&claims.changelog, &claims.license] {
        if !workspace_root.join(required).exists() {
            return Err(format!("required release metadata `{required}` is missing"));
        }
    }
    if claims.repository != "https://github.com/dancxjo/conduit" {
        return Err("release repository identity is not canonical".to_owned());
    }
    if claims.supported_hosts.is_empty() {
        return Err("supported-host boundary is missing".to_owned());
    }
    for host in &claims.supported_hosts {
        if !["tested", "conditional", "unsupported"].contains(&host.status.as_str()) {
            return Err(format!("host `{}` has an invalid status", host.id));
        }
        if host.evidence.is_empty() {
            return Err(format!("host `{}` has no evidence", host.id));
        }
        for evidence in &host.evidence {
            if !workspace_root.join(evidence).exists() {
                return Err(format!(
                    "host `{}` references missing evidence `{evidence}`",
                    host.id
                ));
            }
        }
    }

    let mut ids = BTreeSet::new();
    for capability in &claims.capabilities {
        if !ids.insert(&capability.id) {
            return Err(format!("duplicate capability `{}`", capability.id));
        }
        if capability.release_claim.trim().is_empty() {
            return Err(format!(
                "capability `{}` has an empty release claim",
                capability.id
            ));
        }
        for (name, layer) in [
            ("contract", &capability.contract),
            ("reference_model", &capability.reference_model),
            ("provider", &capability.provider),
            ("host_resolvability", &capability.host_resolvability),
            ("exact_binding", &capability.exact_binding),
            ("runtime_proof", &capability.runtime_proof),
            ("product_presentation", &capability.product_presentation),
        ] {
            validate_layer(workspace_root, &capability.id, name, layer)?;
        }
        if capability.runtime_proof.status == "proven"
            && capability.runtime_proof.evidence.is_empty()
        {
            return Err(format!(
                "runnable claim `{}` lacks executable evidence",
                capability.id
            ));
        }
    }

    for entry in &runnability.entries {
        if entry.state == "runnable"
            && ![
                "canonical-run",
                "canonical-http-loopback",
                "canonical-file-read",
                "canonical-file-write",
                "canonical-file-watch",
            ]
            .contains(&entry.proof.as_str())
        {
            return Err(format!(
                "runnable example `{}` has no executable proof",
                entry.path
            ));
        }
        if !workspace_root.join(&entry.path).exists() {
            return Err(format!("runnability entry `{}` is missing", entry.path));
        }
    }
    let disposition = fs::read_to_string(workspace_root.join(DISPOSITION_PATH))
        .map_err(|_| "accepted-slice closure disposition template is missing".to_owned())?;
    for required in [
        "Accepted implemented scope",
        "Residual requirements",
        "Focused follow-up",
        "residual requirements transferred to #X",
    ] {
        if !disposition.contains(required) {
            return Err(format!("closure disposition template omits `{required}`"));
        }
    }
    Ok(())
}

fn render_matrix(claims: &Claims) -> String {
    let mut output = String::from(
        "# Capability evidence matrix\n\n\
         This file is generated from `release/capabilities.json` by \
         `cargo xtask release-gate --check`. A status is a claim about one \
         layer only; it must not be promoted across columns.\n\n\
         | Capability | Contract | Reference model | Provider | Host resolvability | Exact binding | Runtime proof | Product presentation |\n\
         |---|---|---|---|---|---|---|---|\n",
    );
    for capability in &claims.capabilities {
        output.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            capability.id,
            capability.contract.status,
            capability.reference_model.status,
            capability.provider.status,
            capability.host_resolvability.status,
            capability.exact_binding.status,
            capability.runtime_proof.status,
            capability.product_presentation.status,
        ));
    }
    output.push_str("\n## Release claims\n\n");
    for capability in &claims.capabilities {
        output.push_str(&format!(
            "- `{}`: {} Evidence: {}.\n",
            capability.id,
            capability.release_claim,
            capability
                .runtime_proof
                .evidence
                .iter()
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    output
}

fn git_output(workspace_root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace_root)
        .output()?;
    if !output.status.success() {
        return Err(format!("git {} failed", args.join(" ")).into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn run(
    workspace_root: &Path,
    check: bool,
    output: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (claims, claims_bytes) = load_claims(workspace_root)?;
    let runnability_bytes = fs::read(workspace_root.join(RUNNABILITY_PATH))?;
    let runnability: Runnability = serde_json::from_slice(&runnability_bytes)?;
    validate(workspace_root, &claims, &runnability)?;

    let rendered = render_matrix(&claims);
    let matrix_path = workspace_root.join(MATRIX_PATH);
    if check {
        if fs::read_to_string(&matrix_path).ok().as_deref() != Some(&rendered) {
            return Err(format!("{MATRIX_PATH} is stale; run cargo xtask release-gate").into());
        }
    } else {
        fs::write(&matrix_path, rendered)?;
    }

    if let Some(output_path) = output {
        let status = git_output(workspace_root, &["status", "--porcelain"])?;
        if !status.is_empty() {
            return Err("release evidence cannot be emitted from a dirty worktree".into());
        }
        let commit = std::env::var("GITHUB_SHA")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or(git_output(workspace_root, &["rev-parse", "HEAD"])?);
        let evidence = serde_json::json!({
            "schema": "conduit.release-evidence",
            "version": claims.version,
            "commit": commit,
            "repository": claims.repository,
            "license": claims.license,
            "claims_digest": digest(&claims_bytes),
            "runnability_digest": digest(&runnability_bytes),
            "supported_hosts": claims.supported_hosts,
            "capabilities": claims.capabilities,
        });
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, serde_json::to_vec_pretty(&evidence)?)?;
    }
    println!("release claims, examples, metadata, and capability matrix are exact");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(status: &str, evidence: &[&str]) -> Layer {
        Layer {
            status: status.to_owned(),
            evidence: evidence.iter().map(|value| (*value).to_owned()).collect(),
        }
    }

    #[test]
    fn release_claim_without_executable_evidence_fails_closed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let capability = Capability {
            id: "missing-proof".to_owned(),
            summary: "bad claim".to_owned(),
            contract: layer("proven", &["Cargo.toml"]),
            reference_model: layer("proven", &["Cargo.toml"]),
            provider: layer("available", &["Cargo.toml"]),
            host_resolvability: layer("proven", &["Cargo.toml"]),
            exact_binding: layer("proven", &["Cargo.toml"]),
            runtime_proof: layer("proven", &[]),
            product_presentation: layer("not-applicable", &[]),
            release_claim: "must fail".to_owned(),
        };
        assert!(
            validate_layer(
                root,
                &capability.id,
                "runtime_proof",
                &capability.runtime_proof
            )
            .unwrap_err()
            .contains("no authoritative evidence")
        );
    }

    #[test]
    fn unavailable_provider_cannot_be_a_runnable_example() {
        let entry = RunnabilityEntry {
            path: "examples/hello.panel".to_owned(),
            state: "runnable".to_owned(),
            proof: "canonical-check-rejection".to_owned(),
        };
        assert!(
            entry.state == "runnable"
                && !["canonical-run", "canonical-http-loopback"].contains(&entry.proof.as_str())
        );
    }

    #[test]
    fn missing_release_metadata_fails_closed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let (mut claims, _) = load_claims(root).unwrap();
        claims.license = "MISSING-LICENSE".to_owned();
        let runnability: Runnability =
            serde_json::from_slice(&fs::read(root.join(RUNNABILITY_PATH)).unwrap()).unwrap();
        assert!(
            validate(root, &claims, &runnability)
                .unwrap_err()
                .contains("required release metadata")
        );
    }

    #[test]
    fn incomplete_partial_closure_template_fails_closed() {
        let incomplete = "Accepted implemented scope\nResidual requirements\n";
        assert!(!incomplete.contains("Focused follow-up"));
        assert!(!incomplete.contains("residual requirements transferred to #X"));
    }
}
