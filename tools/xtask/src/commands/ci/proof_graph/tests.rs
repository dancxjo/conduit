use super::*;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REPOSITORY: AtomicU64 = AtomicU64::new(0);

struct Repository {
    root: PathBuf,
}

impl Repository {
    fn new() -> Self {
        let sequence = NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "conduit-ci-proof-{}-{sequence}",
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
        git_text(&self.root, &["rev-parse", "HEAD"]).unwrap()
    }

    fn checkout(&self, branch: &str, start: Option<&str>) {
        let mut args = vec!["checkout", "-q", "-b", branch];
        if let Some(start) = start {
            args.push(start);
        }
        git(&self.root, &args);
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

fn spec(id: &str) -> &'static ProofSpec {
    PROOFS.iter().find(|proof| proof.id == id).unwrap()
}

#[test]
fn retained_impact_selects_only_proofs_the_candidate_was_required_to_run() {
    let selected = ImpactSelection {
        ci_controller_proofs: Vec::new(),
        workspace_shards: BTreeMap::from([
            ("test-products".to_owned(), true),
            ("lint".to_owned(), false),
        ]),
        full_fallback: false,
        shared_compile_packages: vec!["conduit-presentation".to_owned()],
        pages_products_required: true,
        pages_product_proofs: vec!["products.patchbay-debugger".to_owned()],
        esp32_required: false,
        esp32_targets: vec!["c3".to_owned()],
        conduitos_required: false,
        conduitos_x86_proofs: Vec::new(),
        conduitos_architectures: Vec::new(),
        conduitos_aarch64_product_required: false,
    };
    assert!(is_selected(spec("workspace.products"), Some(&selected)));
    assert!(is_selected(
        spec("workspace.shared-compile"),
        Some(&selected)
    ));
    assert!(is_selected(spec("browser.tour"), Some(&selected)));
    assert!(is_selected(
        spec("browser.patchbay-debugger"),
        Some(&selected)
    ));
    assert!(is_selected(spec("products.pages-carrier"), Some(&selected)));
    assert!(!is_selected(spec("machine.esp32-c3"), Some(&selected)));

    let fallback = ImpactSelection {
        full_fallback: true,
        ..selected
    };
    assert!(PROOFS
        .iter()
        .all(|proof| is_selected(proof, Some(&fallback))));
}

#[test]
fn malformed_retained_applicability_falls_back_to_the_complete_registry() {
    let repo = Repository::new();
    let path = repo.root.join("impact.json");
    fs::write(&path, b"{not-json").unwrap();
    assert!(load_selection(Some(&path)).unwrap().is_none());
    assert!(PROOFS.iter().all(|proof| is_selected(proof, None)));
}

fn receipt_for(root: &Path, tree: &str, spec: &ProofSpec, candidate: &str) -> ProofReceipt {
    let input_digest = fingerprint(root, tree, spec).unwrap();
    ProofReceipt {
        schema: RECEIPT_SCHEMA.to_owned(),
        proof_id: spec.id.to_owned(),
        proof_contract_version: spec.contract_version,
        candidate_sha: candidate.to_owned(),
        source_tree: tree.to_owned(),
        proof_key: proof_key(spec, &input_digest, &BTreeMap::new()),
        input_digest,
        result: "success".to_owned(),
        artifact_digests: BTreeMap::new(),
        evidence: vec!["deterministic-test".to_owned()],
    }
}

#[test]
fn candidate_workflows_pin_head_identity_and_do_not_cross_cancel() {
    let root = crate::workspace::workspace_root().unwrap();
    let check = fs::read_to_string(root.join(".github/workflows/check.yml")).unwrap();
    assert!(
        check.contains("CONDUIT_CANDIDATE_SHA: ${{ inputs.candidate_sha || github.event.pull_request.head.sha || '' }}")
    );
    assert!(
        check.contains("CONDUIT_INTEGRATION_SHA: ${{ github.event.merge_group.head_sha || '' }}")
    );
    assert!(check.contains(
        "github.event.pull_request.number || github.ref }}-${{ inputs.candidate_sha || github.event.pull_request.head.sha"
    ));
    assert!(check.contains("cancel-in-progress: false"));
    assert_eq!(
        check
            .matches("ref: ${{ env.CONDUIT_CHECKOUT_SHA }}")
            .count(),
        check.matches("uses: actions/checkout@v7").count()
    );
    assert!(check.contains("git worktree add --detach \"$RUNNER_TEMP/conduit-ci-controller\""));
    assert!(!check.contains("$GITHUB_SHA"));
    let workflow = ".github/workflows/pages-deploy-pr-proof.yml";
    let source = fs::read_to_string(root.join(workflow)).unwrap();
    assert!(
        source.contains("ref: ${{ github.event.pull_request.head.sha }}"),
        "{workflow}"
    );
    let products = fs::read_to_string(root.join(".github/workflows/tour-products.yml")).unwrap();
    assert!(products.contains("ref: ${{ env.CONDUIT_CANDIDATE_SHA }}"));
    assert!(!root
        .join(".github/workflows/patchbay-debugger-pr-proof.yml")
        .exists());
}

#[test]
fn proof_key_changes_only_for_relevant_git_or_contract_inputs() {
    let repo = Repository::new();
    repo.write("targets/browser/host/app.js", "one");
    repo.write("targets/esp32/readme.txt", "one");
    repo.write("proof/browser/executable-tour.spec.mjs", "proof one");
    let first = repo.commit("base");
    let first_tree = resolve_tree(&repo.root, &first).unwrap();
    let browser = spec("browser.tour");
    let initial = fingerprint(&repo.root, &first_tree, browser).unwrap();

    repo.write("targets/esp32/readme.txt", "two");
    let unrelated = repo.commit("unrelated");
    let unrelated_tree = resolve_tree(&repo.root, &unrelated).unwrap();
    assert_eq!(
        initial,
        fingerprint(&repo.root, &unrelated_tree, browser).unwrap()
    );

    repo.write("targets/browser/host/app.js", "two");
    let relevant = repo.commit("relevant");
    let relevant_tree = resolve_tree(&repo.root, &relevant).unwrap();
    assert_ne!(
        initial,
        fingerprint(&repo.root, &relevant_tree, browser).unwrap()
    );

    repo.write("proof/browser/executable-tour.spec.mjs", "proof two");
    let implementation = repo.commit("proof implementation");
    let implementation_tree = resolve_tree(&repo.root, &implementation).unwrap();
    assert_ne!(
        fingerprint(&repo.root, &relevant_tree, browser).unwrap(),
        fingerprint(&repo.root, &implementation_tree, browser).unwrap()
    );

    let mut bumped = *browser;
    bumped.contract_version += 1;
    assert_ne!(
        proof_key(browser, &initial, &BTreeMap::new()),
        proof_key(&bumped, &initial, &BTreeMap::new())
    );
    bumped.contract_version = browser.contract_version;
    bumped.environment = "another-toolchain";
    assert_ne!(
        proof_key(browser, &initial, &BTreeMap::new()),
        proof_key(&bumped, &initial, &BTreeMap::new())
    );
}

#[test]
fn registry_refuses_a_missing_required_input_root() {
    let repo = Repository::new();
    repo.write("targets/browser/host/app.js", "browser");
    let commit = repo.commit("incomplete tree");
    let tree = resolve_tree(&repo.root, &commit).unwrap();
    let error = validate_registry_paths(&repo.root, &tree).unwrap_err();
    assert!(error.contains("required input path"));
    assert!(error.contains("is absent from tree"));
}

#[test]
fn product_proof_renames_invalidate_receipts_without_rejecting_the_tree() {
    let repo = Repository::new();
    for proof in PROOFS {
        for path in proof.inputs.iter().chain(proof.implementation_inputs) {
            if Path::new(path).extension().is_some() {
                repo.write(path, "required input");
            } else {
                repo.write(&format!("{path}/fixture"), "required domain");
            }
        }
    }
    let migrations = [
        (
            "scripts/ci/stage-book-product.sh",
            "products/tour/tools/stage-tour-product.sh",
        ),
        (
            "proof/browser/executable-tour.spec.mjs",
            "proof/browser/tour.spec.mjs",
        ),
        (
            ".github/workflows/tour-products.yml",
            ".github/workflows/renamed-tour-products.yml",
        ),
    ];
    for (old, _) in migrations {
        repo.write(old, "unchanged proof bytes");
    }
    let before = repo.commit("old product paths");
    validate_registry_paths(&repo.root, &before).unwrap();
    let ids = [
        "browser.tour",
        "browser.patchbay-debugger",
        "products.pages-carrier",
    ];
    for (old, new) in migrations {
        let previous = git_text(&repo.root, &["rev-parse", "HEAD"]).unwrap();
        fs::create_dir_all(repo.root.join(new).parent().unwrap()).unwrap();
        fs::rename(repo.root.join(old), repo.root.join(new)).unwrap();
        let renamed = repo.commit("rename product proof implementation");
        validate_registry_paths(&repo.root, &renamed).unwrap();
        for id in ids {
            assert_ne!(
                fingerprint(&repo.root, &previous, spec(id)).unwrap(),
                fingerprint(&repo.root, &renamed, spec(id)).unwrap(),
                "{id} must invalidate evidence after {old} moves"
            );
        }
    }
    let previous = git_text(&repo.root, &["rev-parse", "HEAD"]).unwrap();
    repo.write(
        "products/tour/tools/stage-tour-product.sh",
        "changed proof bytes",
    );
    let changed = repo.commit("change renamed implementation");
    for id in ids {
        assert_ne!(
            fingerprint(&repo.root, &previous, spec(id)).unwrap(),
            fingerprint(&repo.root, &changed, spec(id)).unwrap()
        );
    }
    repo.write("products/creche/browser/creche.mjs", "moved product source");
    let moved = repo.commit("move Creche into its product owner");
    assert_ne!(
        fingerprint(&repo.root, &changed, spec("browser.tour")).unwrap(),
        fingerprint(&repo.root, &moved, spec("browser.tour")).unwrap()
    );
    fs::remove_dir_all(repo.root.join("proof/browser")).unwrap();
    let missing = repo.commit("remove required browser proof domain");
    assert!(validate_registry_paths(&repo.root, &missing)
        .unwrap_err()
        .contains("required input path proof/browser is absent"));
}

#[test]
fn x86_proofs_keep_distinct_keys_in_one_batch_environment() {
    let x86: Vec<_> = PROOFS
        .iter()
        .filter(|proof| proof.id.starts_with("conduitos.x86."))
        .collect();
    assert_eq!(x86.len(), 8);
    assert!(x86
        .iter()
        .all(|proof| proof.environment == "ubuntu-qemu-x86_64-batch-v1"));
    assert!(x86.iter().all(|proof| proof
        .command
        .starts_with("cargo xtask conduitos prove-many --proof ")));
    let ids: BTreeSet<_> = x86.iter().map(|proof| proof.id).collect();
    assert_eq!(ids.len(), x86.len());
}

#[test]
fn current_registry_uses_live_product_ownership_roots() {
    let root = crate::workspace::workspace_root().unwrap();
    let tree = resolve_tree(&root, "HEAD").unwrap();
    validate_registry_paths(&root, &tree).unwrap();
    let browser = spec("browser.tour");
    assert!(browser.inputs.contains(&"products/patchbay/html"));
    assert!(browser.inputs.contains(&"products/tour"));
    assert!(!browser.inputs.iter().any(|path| path.starts_with("apps/")));
}

#[test]
fn unrelated_merge_inherits_browser_evidence_while_candidate_remains_immutable() {
    let repo = Repository::new();
    repo.write("targets/browser/host/app.js", "browser");
    repo.write("targets/esp32/firmware/main.rs", "esp");
    repo.write("proof/browser/executable-tour.spec.mjs", "proof");
    let m0 = repo.commit("m0");
    repo.checkout("candidate-b", Some(&m0));
    repo.write("site/index.html", "candidate presentation");
    let b1 = repo.commit("b1");
    let candidate_tree = resolve_tree(&repo.root, &b1).unwrap();
    let receipt = receipt_for(&repo.root, &candidate_tree, spec("browser.tour"), &b1);

    git(&repo.root, &["checkout", "-q", "master"]);
    repo.write("targets/esp32/firmware/main.rs", "merged A");
    let m1 = repo.commit("a1");
    let MergeTree::Clean(integration_tree) = merge_tree(&repo.root, &m1, &b1).unwrap() else {
        panic!("unexpected conflict")
    };
    let integration_digest =
        fingerprint(&repo.root, &integration_tree, spec("browser.tour")).unwrap();
    assert!(receipt_matches(
        &receipt,
        spec("browser.tour"),
        &integration_digest,
        &proof_key(spec("browser.tour"), &integration_digest, &BTreeMap::new())
    ));
    assert_eq!(resolve_commit(&repo.root, &b1).unwrap(), b1);
}

#[test]
fn related_merge_invalidates_only_its_proof_domain() {
    let repo = Repository::new();
    repo.write("targets/browser/host/app.js", "browser");
    repo.write("targets/esp32/firmware/main.rs", "esp");
    repo.write("proof/browser/executable-tour.spec.mjs", "proof");
    let m0 = repo.commit("m0");
    repo.checkout("candidate-b", Some(&m0));
    repo.write("site/index.html", "candidate");
    let b1 = repo.commit("b1");
    let candidate_tree = resolve_tree(&repo.root, &b1).unwrap();
    let browser_receipt = receipt_for(&repo.root, &candidate_tree, spec("browser.tour"), &b1);
    let esp_receipt = receipt_for(&repo.root, &candidate_tree, spec("machine.esp32-c3"), &b1);

    git(&repo.root, &["checkout", "-q", "master"]);
    repo.write("targets/browser/host/runtime.js", "new semantic consumer");
    let m1 = repo.commit("related A");
    let MergeTree::Clean(tree) = merge_tree(&repo.root, &m1, &b1).unwrap() else {
        panic!("unexpected conflict")
    };
    let browser_digest = fingerprint(&repo.root, &tree, spec("browser.tour")).unwrap();
    let esp_digest = fingerprint(&repo.root, &tree, spec("machine.esp32-c3")).unwrap();
    assert!(!receipt_matches(
        &browser_receipt,
        spec("browser.tour"),
        &browser_digest,
        &proof_key(spec("browser.tour"), &browser_digest, &BTreeMap::new())
    ));
    assert!(receipt_matches(
        &esp_receipt,
        spec("machine.esp32-c3"),
        &esp_digest,
        &proof_key(spec("machine.esp32-c3"), &esp_digest, &BTreeMap::new())
    ));
}

#[test]
fn structural_conflict_precedes_all_proof_execution() {
    let repo = Repository::new();
    repo.write("same.txt", "base\n");
    let m0 = repo.commit("m0");
    repo.checkout("candidate", Some(&m0));
    repo.write("same.txt", "candidate\n");
    let head = repo.commit("candidate");
    let candidate_tree = resolve_tree(&repo.root, &head).unwrap();
    let receipts: Vec<_> = PROOFS
        .iter()
        .map(|proof| {
            ReceiptLoad::Valid(Box::new(receipt_for(
                &repo.root,
                &candidate_tree,
                proof,
                &head,
            )))
        })
        .collect();
    assert_eq!(
        evidence_status(&repo.root, &candidate_tree, &receipts, None).unwrap(),
        "pass"
    );
    git(&repo.root, &["checkout", "-q", "master"]);
    repo.write("same.txt", "main\n");
    let base = repo.commit("main");
    assert!(matches!(
        merge_tree(&repo.root, &base, &head).unwrap(),
        MergeTree::Conflict
    ));
}

#[test]
fn receipts_fail_closed_and_can_be_reused_across_candidate_heads() {
    let repo = Repository::new();
    repo.write("targets/browser/host/app.js", "same");
    repo.write("proof/browser/executable-tour.spec.mjs", "proof");
    let b1 = repo.commit("b1");
    let tree = resolve_tree(&repo.root, &b1).unwrap();
    let proof = spec("browser.tour");
    let receipt = receipt_for(&repo.root, &tree, proof, &b1);
    repo.write(
        "docs/note.md",
        "candidate advances without proof input changes",
    );
    let b2 = repo.commit("b2");
    let tree2 = resolve_tree(&repo.root, &b2).unwrap();
    let digest2 = fingerprint(&repo.root, &tree2, proof).unwrap();
    assert!(receipt_matches(
        &receipt,
        proof,
        &digest2,
        &proof_key(proof, &digest2, &BTreeMap::new())
    ));

    let mut corrupt = receipt.clone();
    corrupt.schema = "unknown".to_owned();
    let receipt_path = repo.root.join("receipt.json");
    fs::write(&receipt_path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
    assert!(matches!(
        load_receipts(std::slice::from_ref(&receipt_path))[0],
        ReceiptLoad::Invalid
    ));
    corrupt = receipt.clone();
    corrupt.result = "incomplete".to_owned();
    assert!(!receipt_matches(
        &corrupt,
        proof,
        &digest2,
        &proof_key(proof, &digest2, &BTreeMap::new())
    ));
    corrupt = receipt;
    corrupt.proof_key = "sha256:bad".to_owned();
    assert!(!receipt_matches(
        &corrupt,
        proof,
        &digest2,
        &proof_key(proof, &digest2, &BTreeMap::new())
    ));
    fs::write(&receipt_path, b"not json").unwrap();
    assert!(matches!(
        load_receipts(&[receipt_path])[0],
        ReceiptLoad::Invalid
    ));
}

#[test]
fn pr_lifecycle_namespaces_do_not_collide() {
    let key = |pr: u64, head: &str| format!("candidate-pr-{pr}-{head}");
    assert_ne!(key(10, "abc"), key(11, "abc"));
    assert_ne!(key(10, "abc"), key(10, "def"));
}

#[path = "ownership_tests.rs"]
mod ownership;
