use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const AUDIT_PATH: &str = "docs/proof-dependency-audit.toml";
const AUDIT_SCHEMA: &str = "conduit.proof-dependency-audit/v1";

#[derive(Deserialize)]
struct Audit {
    schema: String,
    maximum_edges: usize,
    edges: Vec<AuditEdge>,
}

#[derive(Deserialize)]
struct AuditEdge {
    manifest: String,
    dependency: String,
    kind: String,
    classification: String,
    resolution: String,
}

#[test]
fn every_non_dev_dependency_into_proof_is_explicitly_classified() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let audit: Audit = toml::from_str(
        &std::fs::read_to_string(root.join(AUDIT_PATH)).expect("read proof dependency audit"),
    )
    .expect("parse proof dependency audit");
    assert_eq!(audit.schema, AUDIT_SCHEMA);
    assert!(!audit.edges.is_empty());
    assert!(audit.edges.len() <= audit.maximum_edges);

    let declared = audit
        .edges
        .iter()
        .map(|edge| {
            assert!(!edge.classification.trim().is_empty());
            assert!(!edge.resolution.trim().is_empty());
            assert!(matches!(
                edge.kind.as_str(),
                "dependencies" | "build-dependencies"
            ));
            (
                edge.manifest.clone(),
                edge.kind.clone(),
                edge.dependency.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(declared.len(), audit.edges.len(), "duplicate audited edge");
    assert_eq!(declared, actual_edges(&root));
}

fn actual_edges(root: &Path) -> BTreeSet<(String, String, String)> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "*Cargo.toml"])
        .output()
        .expect("list tracked Cargo manifests");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("tracked paths are UTF-8")
        .lines()
        .filter(|manifest| !manifest.starts_with("proof/"))
        .flat_map(|manifest| manifest_edges(root, manifest))
        .collect()
}

fn manifest_edges(root: &Path, manifest: &str) -> Vec<(String, String, String)> {
    let document: toml::Value = toml::from_str(
        &std::fs::read_to_string(root.join(manifest)).expect("read tracked Cargo manifest"),
    )
    .expect("parse tracked Cargo manifest");
    let mut edges = Vec::new();
    collect_dependency_tables(&document, manifest, &mut edges);
    edges
}

fn collect_dependency_tables(
    value: &toml::Value,
    manifest: &str,
    edges: &mut Vec<(String, String, String)>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, child) in table {
        if matches!(key.as_str(), "dependencies" | "build-dependencies") {
            if let Some(dependencies) = child.as_table() {
                for (dependency, declaration) in dependencies {
                    if dependency_path(declaration).is_some_and(|path| path.contains("proof/")) {
                        edges.push((manifest.into(), key.clone(), dependency.clone()));
                    }
                }
            }
        } else if key != "dev-dependencies" {
            collect_dependency_tables(child, manifest, edges);
        }
    }
}

fn dependency_path(value: &toml::Value) -> Option<&str> {
    value
        .as_table()
        .and_then(|declaration| declaration.get("path"))
        .and_then(toml::Value::as_str)
}

#[test]
fn target_and_build_edges_count_but_dev_edges_do_not() {
    let document: toml::Value = toml::from_str(
        r#"
[dependencies]
ordinary = { path = "../ordinary" }

[build-dependencies]
build-proof = { path = "../../proof/build-proof" }

[dev-dependencies]
test-proof = { path = "../../proof/test-proof" }

[target.'cfg(unix)'.dependencies]
target-proof = { path = "../../../proof/target-proof" }
"#,
    )
    .expect("parse fixture manifest");
    let mut edges = Vec::new();
    collect_dependency_tables(&document, "fixture/Cargo.toml", &mut edges);

    assert_eq!(
        edges.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            (
                "fixture/Cargo.toml".into(),
                "build-dependencies".into(),
                "build-proof".into(),
            ),
            (
                "fixture/Cargo.toml".into(),
                "dependencies".into(),
                "target-proof".into(),
            ),
        ])
    );
}
