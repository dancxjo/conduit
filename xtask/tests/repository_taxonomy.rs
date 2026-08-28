use std::{fs, path::Path};

const PACKAGE_ROOTS: &[&str] = &[
    "apps",
    "architecture",
    "fabrication",
    "mechanisms",
    "proof",
    "semantics",
    "targets",
    "tools",
    "xtask",
];

#[test]
fn packages_live_under_an_explicit_ownership_root() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has repository parent");

    for retired in ["crates", "hosts", "firmware", "fixtures"] {
        assert!(
            !repository.join(retired).exists(),
            "retired catch-all root returned: {retired}"
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
