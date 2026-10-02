use std::{collections::BTreeSet, fs, path::Path, process::Command};

const PACKAGE_ROOTS: &[&str] = &[
    "architecture",
    "bodies",
    "make",
    "plots",
    "mechanisms",
    "products",
    "proof",
    "semantics",
    "targets",
    "tools",
];
const RETIRED_ROOTS: &[&str] = &[
    "apps",
    "crates",
    "examples",
    "firmware",
    "fixtures",
    "hosts",
    "tour",
    "profiles",
    "scripts",
    "assets",
    "tests",
    "xtask",
    "seeds",
    "components",
    "modules",
    "subforms",
    "shared",
    "common",
    "utils",
    "misc",
    "libraries",
];

// Reviewed resident data can have a plot owner without pretending to be a
// canonical executable entry in the authored inventory.
const RESIDENT_PLOT_ASSETS: &[&str] = &[
    "plots/birth/names/README.md",
    "plots/birth/names/catalog.mjs",
];

fn tracked_paths(repository: &Path) -> BTreeSet<String> {
    let output = Command::new("git")
        .current_dir(repository)
        .args(["ls-files", "-z"])
        .output()
        .expect("read tracked repository paths");
    assert!(output.status.success(), "git ls-files failed");
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8(entry.to_vec()).expect("tracked path must be UTF-8"))
        .collect()
}

fn validate_owners(paths: &BTreeSet<String>) -> Result<(), String> {
    for path in paths {
        let root = path.split('/').next().unwrap();
        if RETIRED_ROOTS.contains(&root)
            || matches!(path.as_str(), "package.json" | "package-lock.json")
            || path.starts_with("products/book/")
        {
            return Err(format!("retired ownership bucket: {path}"));
        }
        if path.ends_with("/Cargo.toml") && !PACKAGE_ROOTS.contains(&root) {
            return Err(format!("package has no architectural owner: {path}"));
        }
        if let Some(relative) = path.strip_prefix("targets/browser/host/") {
            for part in relative.split('/') {
                if ["book", "tour", "creche", "patchbay"]
                    .iter()
                    .any(|product| {
                        part == *product
                            || part
                                .strip_prefix(product)
                                .is_some_and(|tail| tail.starts_with(['.', '-', '_']))
                    })
                {
                    return Err(format!(
                        "product source beneath generic browser Host: {path}"
                    ));
                }
            }
        }
    }
    Ok(())
}

// Reviewed Rust owners of resident plot meaning, independently of the authored
// executable inventory. A tracked manifest is required; only Rust package
// source/tests and package documentation are admitted by this rule.
const RESIDENT_PLOT_PACKAGES: &[&str] = &["plots/birth"];

fn resident_plot_package_owns(path: &str, paths: &BTreeSet<String>) -> bool {
    RESIDENT_PLOT_PACKAGES.iter().any(|root| {
        let Some(relative) = path
            .strip_prefix(*root)
            .and_then(|relative| relative.strip_prefix('/'))
        else {
            return false;
        };
        paths.contains(&format!("{root}/Cargo.toml"))
            && (matches!(relative, "Cargo.toml" | "README.md")
                || ((relative.starts_with("src/") || relative.starts_with("tests/"))
                    && relative.ends_with(".rs")))
    })
}

fn validate_plot_paths(paths: &BTreeSet<String>, inventory: &str) -> Result<(), String> {
    let inventory: toml::Value = toml::from_str(inventory).map_err(|error| error.to_string())?;
    let plots = inventory["plots"]
        .as_array()
        .ok_or("missing reviewed plots")?;
    let mut declared = BTreeSet::new();
    for plot in plots {
        let slug = plot["slug"].as_str().ok_or("missing Plot slug")?;
        if slug.is_empty() || slug.contains(['/', '\\']) || matches!(slug, "." | "..") {
            return Err(format!("noncanonical Plot slug: {slug}"));
        }
        let path = format!("plots/{slug}/main.conduit");
        if !declared.insert(path.clone()) || !paths.contains(&path) {
            return Err(format!("missing or duplicate reviewed plot source: {path}"));
        }
    }
    for path in paths.iter().filter(|path| path.starts_with("plots/")) {
        let parts: Vec<_> = path.split('/').collect();
        if parts.len() == 2 && !matches!(parts[1], "README.md" | "inventory.toml") {
            return Err(format!("loose source at canonical Plots root: {path}"));
        }
        if parts.len() >= 3
            && !declared.contains(&format!("plots/{}/main.conduit", parts[1]))
            && !RESIDENT_PLOT_ASSETS.contains(&path.as_str())
            && !resident_plot_package_owns(path, paths)
        {
            return Err(format!("unreviewed canonical Plot owner: {path}"));
        }
    }
    Ok(())
}

#[test]
fn tracked_repository_structure_has_explicit_owners() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("dispatcher is beneath repository tools");
    let paths = tracked_paths(repository);
    validate_owners(&paths).unwrap();
    for owner in [
        "products/conduit",
        "targets/browser/workspace",
        "targets/browser/patchbay-workbench",
        "plots/patchbay/workbench",
        "bodies/pete",
        "proof/fixtures/plots",
    ] {
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with(&format!("{owner}/"))),
            "current owner absent from tracked tree: {owner}"
        );
    }
    let inventory = fs::read_to_string(repository.join("plots/inventory.toml")).unwrap();
    validate_plot_paths(&paths, &inventory).unwrap();
}

fn paths(entries: &[&str]) -> BTreeSet<String> {
    entries.iter().map(|entry| (*entry).to_owned()).collect()
}

#[test]
fn retired_roots_and_unowned_packages_are_rejected() {
    for root in RETIRED_ROOTS {
        assert!(validate_owners(&paths(&[&format!("{root}/fixture.txt")])).is_err());
    }
    for path in [
        "package.json",
        "package-lock.json",
        "products/book/index.html",
        "unknown/Cargo.toml",
    ] {
        assert!(validate_owners(&paths(&[path])).is_err(), "accepted {path}");
    }
    validate_owners(&paths(&[
        "Cargo.toml",
        "tools/xtask/Cargo.toml",
        "proof/browser/package.json",
        "products/conduit/tests/hello.rs",
        "docs/assets/diagram.svg",
    ]))
    .unwrap();
}

#[test]
fn product_source_cannot_return_to_the_generic_browser_host() {
    for product in ["book", "tour", "creche", "patchbay"] {
        for relative in [
            format!("assets/{product}.mjs"),
            format!("src/{product}/state.rs"),
        ] {
            assert!(
                validate_owners(&paths(&[&format!("targets/browser/host/{relative}")])).is_err()
            );
        }
    }
    validate_owners(&paths(&[
        "targets/browser/host/assets/application-presentation.mjs",
        "docs/journeys/tour/chapter-1.md",
        "targets/browser/patchbay-workbench/assets/app.js",
    ]))
    .unwrap();
}

#[test]
fn canonical_paths_follow_the_existing_inventory_without_promoting_fixtures() {
    let inventory = "[[plots]]\nslug = 'hello'\nentry = 'hello'\n";
    let valid = paths(&[
        "plots/inventory.toml",
        "plots/hello/main.conduit",
        "plots/hello/assets/icon.svg",
        "proof/fixtures/plots/sample.conduit",
    ]);
    validate_plot_paths(&valid, inventory).unwrap();
    let mut missing = valid.clone();
    missing.remove("plots/hello/main.conduit");
    assert!(validate_plot_paths(&missing, inventory).is_err());
    for extra in ["plots/sample/main.conduit", "plots/loose.conduit"] {
        let mut invalid = valid.clone();
        invalid.insert(extra.into());
        assert!(validate_plot_paths(&invalid, inventory).is_err());
    }
    assert!(validate_plot_paths(&valid, "[[plots]]\nslug = '../proof'\n").is_err());
}

#[test]
fn resident_naming_data_does_not_promote_an_authored_plot() {
    let inventory = "plots = []\n";
    let valid = paths(RESIDENT_PLOT_ASSETS);
    validate_plot_paths(&valid, inventory).unwrap();
    for extra in [
        "plots/birth/main.conduit",
        "plots/birth/extra.mjs",
        "plots/unreviewed/names/catalog.mjs",
    ] {
        let mut invalid = valid.clone();
        invalid.insert(extra.into());
        assert!(validate_plot_paths(&invalid, inventory).is_err(), "{extra}");
    }
}

#[test]
fn reviewed_resident_rust_packages_require_a_manifest_without_authored_promotion() {
    let inventory = "plots = []\n";
    let valid = paths(&[
        "plots/birth/Cargo.toml",
        "plots/birth/README.md",
        "plots/birth/src/lib.rs",
        "plots/birth/src/draft.rs",
        "plots/birth/src/names.rs",
        "plots/birth/src/draft/tests.rs",
        "plots/birth/tests/draft.rs",
        "plots/birth/tests/names.rs",
        "plots/birth/names/catalog.mjs",
        "plots/birth/names/README.md",
    ]);
    validate_plot_paths(&valid, inventory).unwrap();
    let mut missing_manifest = valid.clone();
    missing_manifest.remove("plots/birth/Cargo.toml");
    assert!(validate_plot_paths(&missing_manifest, inventory).is_err());
    for extra in [
        "plots/birth/main.conduit",
        "plots/birth/src/main.conduit",
        "plots/birth/tests/unreviewed.conduit",
        "plots/birth/extra.mjs",
        "plots/birth/names/unreviewed.mjs",
        "plots/birth/other/Cargo.toml",
        "plots/unknown/Cargo.toml",
        "plots/unknown/src/lib.rs",
    ] {
        let mut invalid = valid.clone();
        invalid.insert(extra.into());
        assert!(validate_plot_paths(&invalid, inventory).is_err(), "{extra}");
    }
    assert!(validate_plot_paths(
        &paths(&["plots/unknown/Cargo.toml", "plots/unknown/src/lib.rs"]),
        inventory,
    )
    .is_err());
}

#[test]
fn untracked_local_directories_are_outside_the_guard_input() {
    let temporary = std::env::temp_dir().join(format!("conduit-taxonomy-{}", std::process::id()));
    fs::create_dir_all(&temporary).unwrap();
    let initialized = Command::new("git")
        .args(["init", "-q"])
        .arg(&temporary)
        .status()
        .unwrap();
    assert!(initialized.success());
    fs::create_dir_all(temporary.join("apps/local-build")).unwrap();
    fs::write(temporary.join("apps/local-build/Cargo.toml"), "untracked").unwrap();
    let tracked = tracked_paths(&temporary);
    assert!(tracked.is_empty());
    validate_owners(&tracked).unwrap();
    fs::remove_dir_all(temporary).unwrap();
}
