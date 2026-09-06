use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REPOSITORY: AtomicU64 = AtomicU64::new(0);

struct Repository {
    root: PathBuf,
}

impl Repository {
    fn new() -> Self {
        let sequence = NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "conduit-ci-impact-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"]);
        git(&root, &["config", "user.name", "Conduit CI test"]);
        git(&root, &["config", "user.email", "ci-test@conduit.invalid"]);
        Self { root }
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn commit(&self, message: &str) -> String {
        git(&self.root, &["add", "."]);
        git(&self.root, &["commit", "-q", "-m", message]);
        git_text(&self.root, &["rev-parse", "HEAD"])
    }

    fn branch(&self, name: &str, start: &str) {
        git(&self.root, &["checkout", "-q", "-B", name, start]);
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_text(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn moving_base_does_not_change_immutable_candidate_paths() {
    let repo = Repository::new();
    repo.write("README.md", "M0");
    let m0 = repo.commit("M0");
    repo.branch("candidate-a", &m0);
    repo.write("targets/esp32/a.rs", "A1");
    let a1 = repo.commit("A1");
    repo.branch("candidate-b", &m0);
    repo.write("products/tour/browser/tour.css", "B1");
    let b1 = repo.commit("B1");

    let before = candidate_changed_paths(&repo.root, &m0, &b1).unwrap();
    let after = candidate_changed_paths(&repo.root, &a1, &b1).unwrap();
    assert_eq!(before.comparison_base, m0);
    assert_eq!(after.comparison_base, m0);
    assert_eq!(before.paths, ["products/tour/browser/tour.css"]);
    assert_eq!(after.paths, before.paths);
}

#[test]
fn candidate_path_discovery_fails_closed_without_common_history() {
    let repo = Repository::new();
    repo.write("main.txt", "main");
    let main = repo.commit("main");
    git(&repo.root, &["checkout", "-q", "--orphan", "unrelated"]);
    git(&repo.root, &["rm", "-q", "-f", "main.txt"]);
    repo.write("candidate.txt", "candidate");
    let candidate = repo.commit("candidate");

    let error = candidate_changed_paths(&repo.root, &main, &candidate).unwrap_err();
    assert!(error.contains("merge-base"));
}

#[test]
fn exact_inline_test_relocation_is_recognized() {
    let repo = Repository::new();
    repo.write(
        "crate/src/widget.rs",
        "pub fn value() -> u8 { 7 }\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn value_is_seven() {\n        assert_eq!(value(), 7);\n    }\n}\n",
    );
    let base = repo.commit("inline tests");
    repo.write(
        "crate/src/widget.rs",
        "pub fn value() -> u8 { 7 }\n\n#[cfg(test)]\nmod tests;\n",
    );
    repo.write(
        "crate/src/widget/tests.rs",
        "use super::*;\n\n#[test]\nfn value_is_seven() {\n    assert_eq!(value(), 7);\n}\n",
    );
    let candidate = repo.commit("extract tests");

    let changes = candidate_changed_paths(&repo.root, &base, &candidate).unwrap();
    assert_eq!(changes.test_extraction_parents, ["crate/src/widget.rs"]);
}

#[test]
fn production_or_location_sensitive_changes_refuse_extraction_equivalence() {
    let before = "pub fn value() -> u8 { 7 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn value_is_seven() {\n        assert_eq!(value(), 7);\n    }\n}\n";
    let child = "#[test]\nfn value_is_seven() {\n    assert_eq!(value(), 7);\n}\n";
    assert!(!exact_test_module_extraction(
        before,
        "pub fn value() -> u8 { 8 }\n\n#[cfg(test)]\nmod tests;\n",
        child,
    ));
    assert!(!exact_test_module_extraction(
        "pub fn value() -> u8 { 7 }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn value_is_seven() {\n        assert_eq!(value(), line!());\n    }\n}\n",
        "pub fn value() -> u8 { 7 }\n\n#[cfg(test)]\nmod tests;\n",
        "#[test]\nfn value_is_seven() {\n    assert_eq!(value(), line!());\n}\n",
    ));
    assert!(!contains_raw_string("let word = \"Controller\";"));
    assert!(contains_raw_string("let word = r#\"raw\"#;"));
}
