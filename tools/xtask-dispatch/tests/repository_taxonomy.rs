use std::{collections::BTreeSet, fs, path::Path, process::Command};

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

// Reviewed resident data can have a form owner without pretending to be a
// canonical executable entry in the authored inventory.
const RESIDENT_FORM_ASSETS: &[&str] = &[
    "forms/birth/names/README.md",
    "forms/birth/names/catalog.mjs",
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

// Reviewed Rust owners of resident form meaning, independently of the authored
// executable inventory. A tracked manifest is required; only Rust package
// source/tests and package documentation are admitted by this rule.
const RESIDENT_FORM_PACKAGES: &[&str] = &["forms/birth"];

fn resident_form_package_owns(path: &str, paths: &BTreeSet<String>) -> bool {
    RESIDENT_FORM_PACKAGES.iter().any(|root| {
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

fn validate_form_paths(paths: &BTreeSet<String>, inventory: &str) -> Result<(), String> {
    let inventory: toml::Value = toml::from_str(inventory).map_err(|error| error.to_string())?;
    let forms = inventory["forms"]
        .as_array()
        .ok_or("missing reviewed forms")?;
    let mut declared = BTreeSet::new();
    for form in forms {
        let slug = form["slug"].as_str().ok_or("missing Form slug")?;
        if slug.is_empty() || slug.contains(['/', '\\']) || matches!(slug, "." | "..") {
            return Err(format!("noncanonical Form slug: {slug}"));
        }
        let path = format!("forms/{slug}/main.conduit");
        if !declared.insert(path.clone()) || !paths.contains(&path) {
            return Err(format!("missing or duplicate reviewed form source: {path}"));
        }
    }
    for path in paths.iter().filter(|path| path.starts_with("forms/")) {
        let parts: Vec<_> = path.split('/').collect();
        if parts.len() == 2 && !matches!(parts[1], "README.md" | "inventory.toml") {
            return Err(format!("loose source at canonical Forms root: {path}"));
        }
        if parts.len() >= 3
            && !declared.contains(&format!("forms/{}/main.conduit", parts[1]))
            && !RESIDENT_FORM_ASSETS.contains(&path.as_str())
            && !resident_form_package_owns(path, paths)
        {
            return Err(format!("unreviewed canonical Form owner: {path}"));
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
        "products/tour",
        "products/creche",
        "products/patchbay",
        "bodies/pete",
        "proof/fixtures/forms",
    ] {
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with(&format!("{owner}/"))),
            "current owner absent from tracked tree: {owner}"
        );
    }
    let inventory = fs::read_to_string(repository.join("forms/inventory.toml")).unwrap();
    validate_form_paths(&paths, &inventory).unwrap();
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
        "products/tour/browser/tour.mjs",
        "products/patchbay/html/assets/app.js",
    ]))
    .unwrap();
}

#[test]
fn canonical_paths_follow_the_existing_inventory_without_promoting_fixtures() {
    let inventory = "[[forms]]\nslug = 'hello'\nentry = 'hello'\n";
    let valid = paths(&[
        "forms/inventory.toml",
        "forms/hello/main.conduit",
        "forms/hello/assets/icon.svg",
        "proof/fixtures/forms/sample.conduit",
    ]);
    validate_form_paths(&valid, inventory).unwrap();
    let mut missing = valid.clone();
    missing.remove("forms/hello/main.conduit");
    assert!(validate_form_paths(&missing, inventory).is_err());
    for extra in ["forms/sample/main.conduit", "forms/loose.conduit"] {
        let mut invalid = valid.clone();
        invalid.insert(extra.into());
        assert!(validate_form_paths(&invalid, inventory).is_err());
    }
    assert!(validate_form_paths(&valid, "[[forms]]\nslug = '../proof'\n").is_err());
}

#[test]
fn resident_naming_data_does_not_promote_an_authored_form() {
    let inventory = "forms = []\n";
    let valid = paths(RESIDENT_FORM_ASSETS);
    validate_form_paths(&valid, inventory).unwrap();
    for extra in [
        "forms/birth/main.conduit",
        "forms/birth/extra.mjs",
        "forms/unreviewed/names/catalog.mjs",
    ] {
        let mut invalid = valid.clone();
        invalid.insert(extra.into());
        assert!(validate_form_paths(&invalid, inventory).is_err(), "{extra}");
    }
}

#[test]
fn reviewed_resident_rust_packages_require_a_manifest_without_authored_promotion() {
    let inventory = "forms = []\n";
    let valid = paths(&[
        "forms/birth/Cargo.toml",
        "forms/birth/README.md",
        "forms/birth/src/lib.rs",
        "forms/birth/src/draft.rs",
        "forms/birth/src/names.rs",
        "forms/birth/src/draft/tests.rs",
        "forms/birth/tests/draft.rs",
        "forms/birth/tests/names.rs",
        "forms/birth/names/catalog.mjs",
        "forms/birth/names/README.md",
    ]);
    validate_form_paths(&valid, inventory).unwrap();
    let mut missing_manifest = valid.clone();
    missing_manifest.remove("forms/birth/Cargo.toml");
    assert!(validate_form_paths(&missing_manifest, inventory).is_err());
    for extra in [
        "forms/birth/main.conduit",
        "forms/birth/src/main.conduit",
        "forms/birth/tests/unreviewed.conduit",
        "forms/birth/extra.mjs",
        "forms/birth/names/unreviewed.mjs",
        "forms/birth/other/Cargo.toml",
        "forms/unknown/Cargo.toml",
        "forms/unknown/src/lib.rs",
    ] {
        let mut invalid = valid.clone();
        invalid.insert(extra.into());
        assert!(validate_form_paths(&invalid, inventory).is_err(), "{extra}");
    }
    assert!(validate_form_paths(
        &paths(&["forms/unknown/Cargo.toml", "forms/unknown/src/lib.rs"]),
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
