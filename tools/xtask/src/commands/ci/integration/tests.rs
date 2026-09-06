use super::*;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REPOSITORY: AtomicU64 = AtomicU64::new(0);

struct Repository {
    root: PathBuf,
}

impl Repository {
    fn new() -> Self {
        let sequence = NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "conduit-ci-integration-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.name", "CI Test"]);
        git(&root, &["config", "user.email", "ci@example.invalid"]);
        Self { root }
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.root.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn commit(&self, message: &str) -> String {
        git(&self.root, &["add", "."]);
        git(&self.root, &["commit", "-q", "-m", message]);
        text(&self.root, &["rev-parse", "HEAD"])
    }

    fn checkout(&self, branch: &str, start: &str) {
        git(&self.root, &["checkout", "-q", "-b", branch, start]);
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn squash_merged_parent_tree_is_an_exact_effective_base() {
    let repo = Repository::new();
    repo.write("shared", "base\n");
    let m0 = repo.commit("m0");

    repo.checkout("parent", &m0);
    repo.write("shared", "parent\n");
    let parent = repo.commit("parent candidate");
    repo.checkout("child", &parent);
    repo.write("shared", "parent plus child\n");
    let child = repo.commit("child candidate");

    git(&repo.root, &["checkout", "-q", "main"]);
    repo.write("shared", "parent\n");
    let squash = repo.commit("squash parent");
    assert_eq!(
        resolve_tree(&repo.root, &parent).unwrap(),
        resolve_tree(&repo.root, &squash).unwrap()
    );

    let integration = resolve(&repo.root, &squash, &child).unwrap();
    assert_eq!(integration.status, "clean");
    assert_eq!(
        integration.merge_base_method,
        "content-equivalent-candidate-parent"
    );
    assert_eq!(
        integration.effective_merge_base_sha.as_deref(),
        Some(squash.as_str())
    );
    let tree = integration.integration_tree.unwrap();
    assert_eq!(
        text(&repo.root, &["show", &format!("{tree}:shared")]),
        "parent plus child"
    );
}

#[test]
fn exact_equivalent_base_still_reports_a_real_conflict() {
    let repo = Repository::new();
    repo.write("shared", "base\n");
    let m0 = repo.commit("m0");

    repo.checkout("parent", &m0);
    repo.write("parent-only", "same tree anchor\n");
    let parent = repo.commit("parent candidate");
    repo.checkout("child", &parent);
    repo.write("shared", "child\n");
    let child = repo.commit("child candidate");

    git(&repo.root, &["checkout", "-q", "main"]);
    repo.write("parent-only", "same tree anchor\n");
    let squash = repo.commit("squash parent");
    repo.write("shared", "main\n");
    let current_main = repo.commit("conflicting main");
    assert_eq!(
        resolve_tree(&repo.root, &parent).unwrap(),
        resolve_tree(&repo.root, &squash).unwrap()
    );

    let integration = resolve(&repo.root, &current_main, &child).unwrap();
    assert_eq!(integration.status, "conflict");
    assert!(integration.integration_tree.is_none());
    assert_eq!(integration.merge_base_method, "none");
}

#[test]
fn no_exact_tree_match_fails_closed() {
    let repo = Repository::new();
    repo.write("shared", "base\n");
    let m0 = repo.commit("m0");
    repo.checkout("child", &m0);
    repo.write("shared", "child\n");
    let child = repo.commit("child");
    git(&repo.root, &["checkout", "-q", "main"]);
    repo.write("shared", "main\n");
    let main = repo.commit("main");

    let integration = resolve(&repo.root, &main, &child).unwrap();
    assert_eq!(integration.status, "conflict");
    assert_eq!(integration.merge_base_method, "none");
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {} failed", args.join(" "));
}

fn text(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {} failed", args.join(" "));
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
