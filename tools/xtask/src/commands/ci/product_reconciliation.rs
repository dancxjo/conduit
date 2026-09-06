use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::impact::product_registry;

const SCHEMA: &str = "conduit.ci.product-reconciliation/v1";

#[derive(Debug, Serialize)]
struct ProductReconciliation {
    schema: &'static str,
    product_id: String,
    candidate_sha: String,
    candidate_tree: String,
    integration_sha: String,
    integration_tree: String,
    changed_paths: Vec<String>,
    disposition: &'static str,
}

pub(super) fn run(
    product_id: &str,
    candidate: &str,
    integration: &str,
    json_out: Option<&Path>,
    allow_execute: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !product_registry::contains(product_id) {
        return Err(format!("unknown product proof {product_id}").into());
    }

    let root = crate::workspace::workspace_root()?;
    let candidate_sha = resolve(&root, candidate, "commit")?;
    let candidate_tree = resolve(&root, candidate, "tree")?;
    let integration_sha = resolve(&root, integration, "commit")?;
    let integration_tree = resolve(&root, integration, "tree")?;
    let changed_paths = changed_paths(&root, &candidate_tree, &integration_tree)?;
    let changed = product_registry::proofs_for_paths(&changed_paths).contains(&product_id);
    let reconciliation = ProductReconciliation {
        schema: SCHEMA,
        product_id: product_id.to_owned(),
        candidate_sha,
        candidate_tree,
        integration_sha,
        integration_tree,
        changed_paths,
        disposition: if changed { "execute" } else { "inherited" },
    };

    if let Some(path) = json_out {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&reconciliation)?),
        )?;
    }
    println!("product_id={}", reconciliation.product_id);
    println!("candidate_tree={}", reconciliation.candidate_tree);
    println!("integration_tree={}", reconciliation.integration_tree);
    println!("disposition={}", reconciliation.disposition);

    if changed && !allow_execute {
        return Err(format!(
            "product proof {product_id} has changed integration inputs; exact integration execution is required"
        )
        .into());
    }
    Ok(())
}

fn resolve(root: &Path, revision: &str, kind: &str) -> Result<String, String> {
    let suffix = match kind {
        "commit" => "^{commit}",
        "tree" => "^{tree}",
        _ => return Err("unknown Git object kind".to_owned()),
    };
    let output = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--verify", &format!("{revision}{suffix}")])
        .output()
        .map_err(|error| format!("cannot execute git rev-parse: {error}"))?;
    if !output.status.success() {
        return Err(format!("cannot resolve {revision} as a Git {kind}"));
    }
    let identity = String::from_utf8(output.stdout)
        .map_err(|_| "git rev-parse returned non-UTF-8 output".to_owned())?
        .trim()
        .to_owned();
    if identity.len() != 40 || !identity.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("git rev-parse returned malformed {kind} identity"));
    }
    Ok(identity)
}

fn changed_paths(root: &Path, left_tree: &str, right_tree: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-z",
            left_tree,
            right_tree,
        ])
        .output()
        .map_err(|error| format!("cannot execute git diff-tree: {error}"))?;
    if !output.status.success() {
        return Err("cannot compare candidate and integration trees".to_owned());
    }
    let mut paths = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map_err(|_| "git diff-tree returned a non-UTF-8 path".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    paths.dedup();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::{changed_paths, product_registry, resolve};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn unrelated_main_movement_inherits_but_related_product_input_executes() {
        let repo = Repository::new();
        repo.write("site/index.html", "base\n");
        repo.write("docs/note.md", "base\n");
        let base = repo.commit("base");

        repo.branch("candidate", &base);
        repo.write("site/index.html", "candidate\n");
        let candidate = repo.commit("candidate");

        repo.checkout("master");
        repo.write("docs/note.md", "unrelated main\n");
        let unrelated_main = repo.commit("unrelated main");
        repo.merge("candidate");
        let unrelated_integration = repo.commit("integrate candidate");

        let candidate_tree = resolve(&repo.root, &candidate, "tree").unwrap();
        let integration_tree = resolve(&repo.root, &unrelated_integration, "tree").unwrap();
        let paths = changed_paths(&repo.root, &candidate_tree, &integration_tree).unwrap();
        assert_eq!(paths, ["docs/note.md"]);
        assert!(product_registry::proofs_for_paths(&paths).is_empty());

        repo.branch("related-main", &unrelated_main);
        repo.write("site/footer.html", "related main\n");
        repo.commit("related main");
        repo.merge("candidate");
        let related_integration = repo.commit("integrate related candidate");
        let related_tree = resolve(&repo.root, &related_integration, "tree").unwrap();
        let paths = changed_paths(&repo.root, &candidate_tree, &related_tree).unwrap();
        assert!(product_registry::proofs_for_paths(&paths).contains(&"products.pages-carrier"));
    }

    struct Repository {
        root: PathBuf,
    }

    impl Repository {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "conduit-product-reconciliation-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            git(&root, &["init", "-q", "-b", "master"]);
            git(&root, &["config", "user.email", "ci@example.invalid"]);
            git(&root, &["config", "user.name", "Conduit CI"]);
            Self { root }
        }

        fn write(&self, path: &str, contents: &str) {
            let path = self.root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }

        fn commit(&self, message: &str) -> String {
            git(&self.root, &["add", "."]);
            git(
                &self.root,
                &["commit", "-q", "--allow-empty", "-m", message],
            );
            output(&self.root, &["rev-parse", "HEAD"])
        }

        fn branch(&self, name: &str, start: &str) {
            git(&self.root, &["checkout", "-q", "-b", name, start]);
        }

        fn checkout(&self, name: &str) {
            git(&self.root, &["checkout", "-q", name]);
        }

        fn merge(&self, name: &str) {
            git(&self.root, &["merge", "-q", "--no-commit", "--no-ff", name]);
        }
    }

    impl Drop for Repository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn git(root: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .current_dir(root)
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success(), "git {arguments:?}");
    }

    fn output(root: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(arguments)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {arguments:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
}
