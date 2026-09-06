use std::{fs, path::Path};

const PACKAGE_ROOTS: &[&str] = &[
    "architecture",
    "bodies",
    "fabrication",
    "forms",
    "mechanisms",
    "products",
    "proof",
    "semantics",
    "targets",
    "tools",
];

#[test]
fn packages_live_under_an_explicit_ownership_root() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");

    for retired in [
        "apps", "crates", "examples", "firmware", "fixtures", "hosts", "tour",
    ] {
        assert!(
            !repository.join(retired).exists(),
            "retired catch-all root returned: {retired}"
        );
    }
    assert!(
        !repository.join("products/book").exists(),
        "a future Book product must not be reserved before it exists"
    );
    for current in [
        "products/conduit",
        "products/tour",
        "products/creche",
        "products/patchbay",
        "bodies/pete",
    ] {
        assert!(
            repository.join(current).is_dir(),
            "current ownership root is absent: {current}"
        );
    }

    let mut manifests = Vec::new();
    collect_manifests(repository, repository, &mut manifests);
    manifests.sort();
    for manifest in manifests {
        if manifest == repository.join("Cargo.toml") {
            continue;
        }
        let relative = manifest
            .strip_prefix(repository)
            .expect("manifest beneath repository");
        let owner = relative
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_str())
            .expect("manifest has ownership root");
        assert!(
            PACKAGE_ROOTS.contains(&owner),
            "package has no explicit ownership class: {}",
            relative.display()
        );
    }
}

#[test]
fn canonical_forms_have_stable_owners_and_proof_fixtures_stay_separate() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");
    let forms = repository.join("forms");
    let mut canonical_count = 0;
    for entry in fs::read_dir(&forms)
        .expect("read canonical Forms")
        .collect::<Result<Vec<_>, _>>()
        .expect("read canonical Form entry")
    {
        let path = entry.path();
        if path.is_file() {
            assert!(
                matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("README.md" | "inventory.toml")
                ),
                "canonical Form source cannot be loose at forms root: {}",
                path.display()
            );
            continue;
        }
        assert!(path.is_dir(), "unexpected Forms entry: {}", path.display());
        assert!(
            path.join("main.conduit").is_file(),
            "canonical Form owner lacks main.conduit: {}",
            path.display()
        );
        canonical_count += 1;
    }
    assert!(forms.join("inventory.toml").is_file());
    assert!(
        canonical_count >= 32,
        "canonical Form inventory was truncated"
    );
    assert!(repository.join("proof/fixtures/forms").is_dir());
}

fn collect_manifests(repository: &Path, directory: &Path, manifests: &mut Vec<std::path::PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("read {} entry: {error}", directory.display()));
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let relative = path
                .strip_prefix(repository)
                .expect("path beneath repository");
            if relative.components().any(|component| {
                matches!(
                    component.as_os_str().to_str(),
                    Some(".git" | "node_modules" | "target" | "test-results")
                )
            }) {
                continue;
            }
            collect_manifests(repository, &path, manifests);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
            manifests.push(path);
        }
    }
}
