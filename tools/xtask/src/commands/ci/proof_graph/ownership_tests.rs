use super::*;

fn repository_with_required_domains() -> Repository {
    let repo = Repository::new();
    for proof in PROOFS {
        for path in proof.inputs {
            if Path::new(path).extension().is_some() {
                repo.write(path, "required source");
            } else {
                repo.write(&format!("{path}/fixture"), "required domain");
            }
        }
    }
    repo
}

#[test]
fn support_root_moves_invalidate_receipts_without_rejecting_the_new_owners() {
    let repo = repository_with_required_domains();
    let moves = [
        (
            "xtask/src/suites/workspace_shards.rs",
            "tools/xtask/src/suites/workspace_shards.rs",
            "workspace.foundation",
        ),
        (
            "scripts/ci/stage-tour-product.sh",
            "products/tour/tools/stage-tour-product.sh",
            "browser.tour",
        ),
        (
            "package.json",
            "proof/browser/package.json",
            "browser.patchbay-debugger",
        ),
        (
            "package-lock.json",
            "proof/browser/package-lock.json",
            "browser.patchbay-debugger",
        ),
        (
            "profiles/hosts/std.profile.json",
            "targets/std/profiles/std.profile.json",
            "products.pages-carrier",
        ),
        (
            "scripts/ci/conduitos-tools.sh",
            "targets/conduitos/tools/conduitos-tools.sh",
            "conduitos.tools",
        ),
    ];
    for (old, _, _) in moves {
        repo.write(old, "exact implementation bytes");
    }
    let mut previous = repo.commit("support at former roots");
    validate_registry_paths(&repo.root, &previous).unwrap();
    for (old, new, id) in moves {
        fs::create_dir_all(repo.root.join(new).parent().unwrap()).unwrap();
        fs::rename(repo.root.join(old), repo.root.join(new)).unwrap();
        let moved = repo.commit("move to architectural owner");
        validate_registry_paths(&repo.root, &moved).unwrap();
        assert_ne!(
            fingerprint(&repo.root, &previous, spec(id)).unwrap(),
            fingerprint(&repo.root, &moved, spec(id)).unwrap(),
            "rename must invalidate {id}"
        );
        repo.write(new, "changed implementation bytes");
        let changed = repo.commit("change owned implementation");
        assert_ne!(
            fingerprint(&repo.root, &moved, spec(id)).unwrap(),
            fingerprint(&repo.root, &changed, spec(id)).unwrap(),
            "new owner must remain covered for {id}"
        );
        previous = changed;
    }
    fs::remove_dir_all(repo.root.join("proof/browser")).unwrap();
    let missing = repo.commit("remove required proof domain");
    assert!(validate_registry_paths(&repo.root, &missing)
        .unwrap_err()
        .contains("proof/browser"));
}

#[test]
fn deleting_an_implementation_invalidates_evidence_instead_of_inheriting_success() {
    let repo = repository_with_required_domains();
    let path = "products/tour/tools/stage-tour-product.sh";
    repo.write(path, "implementation");
    let before = repo.commit("implementation present");
    let proof = spec("browser.tour");
    let before_digest = fingerprint(&repo.root, &before, proof).unwrap();
    fs::remove_file(repo.root.join(path)).unwrap();
    let after = repo.commit("implementation removed");
    validate_registry_paths(&repo.root, &after).unwrap();
    let after_digest = fingerprint(&repo.root, &after, proof).unwrap();
    assert_ne!(before_digest, after_digest);
    assert_ne!(
        proof_key(proof, &before_digest, &BTreeMap::new()),
        proof_key(proof, &after_digest, &BTreeMap::new())
    );
}
