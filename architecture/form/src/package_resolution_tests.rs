use super::*;

fn package(source: &str) -> CheckedPackageSource {
    let document = parse_syntax_document(source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    CheckedPackageSource::from_syntax(source, &document.packages[0]).unwrap()
}

fn source(path: &str, version: &str, requirements: &[(&str, &str)]) -> String {
    let mut value = format!("pack {path} (\n    version = {version}\n) {{\n    ship public\n");
    for (path, version) in requirements {
        value.push_str(&format!("    need {path} = {version}\n"));
    }
    value.push_str("}\n");
    value
}

#[test]
fn package_source_is_lossless_finite_and_canonicalizes_order() {
    let source = "# pack truth\npack house/sensors (\n    version = 1.4.0\n) {\n    ship temperature\n    ship humidity\n    need math/units = ^2.1\n}\n";
    let document = parse_syntax_document(source);
    assert_eq!(document.round_trip(), source);
    assert!(document.diagnostics.is_empty());
    assert!(document.forms.is_empty());
    assert!(document.constructions.is_empty());
    let package = &document.packages[0];
    assert_eq!(package.path.text, "house/sensors");
    assert_eq!(package.version.text, "1.4.0");
    assert_eq!(package.exports.len(), 2);
    assert_eq!(package.requirements[0].path.text, "math/units");

    let checked = CheckedPackageSource::from_syntax(source, package).unwrap();
    assert_eq!(checked.exports, ["humidity", "temperature"]);
    assert_eq!(checked.requirements[0].path, "math/units");
}

#[test]
fn resolver_selects_highest_matching_version_and_locks_exact_content() {
    let units_210 = source("math/units", "2.1.0", &[]);
    let units_230 = source("math/units", "2.3.0", &[]);
    let units_300 = source("math/units", "3.0.0", &[]);
    let sensors = source("house/sensors", "1.4.0", &[("math/units", "^2.1")]);
    let catalog = [
        package(&units_300),
        package(&sensors),
        package(&units_210),
        package(&units_230),
    ];
    let root = PackageVersion {
        major: 1,
        minor: 4,
        patch: 0,
    };
    let lock = resolve_package_lock(&catalog, &[("house/sensors", root)]).unwrap();
    let reversed = catalog.iter().cloned().rev().collect::<Vec<_>>();
    let reversed_lock = resolve_package_lock(&reversed, &[("house/sensors", root)]).unwrap();
    assert_eq!(lock, reversed_lock);
    assert_eq!(lock.schema, CONDUIT_LOCK_SCHEMA);
    assert_eq!(lock.packages.len(), 2);
    let sensors = lock
        .packages
        .iter()
        .find(|package| package.path == "house/sensors")
        .unwrap();
    assert_eq!(
        sensors.requirements[0].version,
        PackageVersion {
            major: 2,
            minor: 3,
            patch: 0
        }
    );
    assert_ne!(sensors.content_digest, [0; 32]);
    let encoded = serde_json::to_vec(&lock).unwrap();
    assert_eq!(
        serde_json::from_slice::<ConduitLock>(&encoded).unwrap(),
        lock
    );
    lock.validate_against(&catalog).unwrap();
    let mut stale = lock.clone();
    stale.packages[0].content_digest[0] ^= 1;
    assert_eq!(
        stale.validate_against(&catalog),
        Err(PackageResolutionError::InvalidLock)
    );
}

#[test]
fn cycles_conflicts_and_duplicate_content_refuse_distinctly() {
    let a_source = source("test/a", "1.0.0", &[("test/b", "^1.0")]);
    let b_source = source("test/b", "1.0.0", &[("test/a", "^1.0")]);
    let a = package(&a_source);
    let b = package(&b_source);
    assert_eq!(
        resolve_package_lock(&[a.clone(), b], &[("test/a", a.version)]),
        Err(PackageResolutionError::DependencyCycle(vec![
            "test/a".into(),
            "test/b".into(),
            "test/a".into(),
        ]))
    );
    assert_eq!(
        resolve_package_lock(&[a.clone(), a.clone()], &[("test/a", a.version)]),
        Err(PackageResolutionError::DuplicateContentIdentity)
    );

    let root_source = source(
        "test/root",
        "1.0.0",
        &[("test/left", "1.0.0"), ("test/right", "1.0.0")],
    );
    let left_source = source("test/left", "1.0.0", &[("test/shared", "1.0.0")]);
    let right_source = source("test/right", "1.0.0", &[("test/shared", "2.0.0")]);
    let shared_one_source = source("test/shared", "1.0.0", &[]);
    let shared_two_source = source("test/shared", "2.0.0", &[]);
    let root = package(&root_source);
    let catalog = [
        root.clone(),
        package(&left_source),
        package(&right_source),
        package(&shared_one_source),
        package(&shared_two_source),
    ];
    assert_eq!(
        resolve_package_lock(&catalog, &[("test/root", root.version)]),
        Err(PackageResolutionError::ConflictingRequirement(
            "test/shared".into()
        ))
    );
}

#[test]
fn later_constraints_backtrack_to_the_highest_jointly_compatible_version() {
    let root_source = source(
        "test/root",
        "1.0.0",
        &[("test/left", "1.0.0"), ("test/right", "1.0.0")],
    );
    let left_source = source("test/left", "1.0.0", &[("test/shared", "^1.0")]);
    let right_source = source("test/right", "1.0.0", &[("test/shared", "1.1.0")]);
    let shared_110_source = source("test/shared", "1.1.0", &[]);
    let shared_120_source = source("test/shared", "1.2.0", &[]);
    let root = package(&root_source);
    let catalog = [
        root.clone(),
        package(&left_source),
        package(&right_source),
        package(&shared_110_source),
        package(&shared_120_source),
    ];
    let lock = resolve_package_lock(&catalog, &[("test/root", root.version)]).unwrap();
    assert_eq!(
        lock.packages
            .iter()
            .find(|package| package.path == "test/shared")
            .unwrap()
            .version,
        PackageVersion {
            major: 1,
            minor: 1,
            patch: 0,
        }
    );
}

#[test]
fn malformed_package_source_never_falls_back_to_form_syntax() {
    for source in [
        "pack house/sensors (\n version = 01.0.0\n) {\n}\n",
        "pack house/sensors (\n version = 1.0.0\n) {\n ship x\n ship x\n}\n",
        "pack house/sensors (\n version = 1.0.0\n) {\n need ../ambient = ^1.0\n}\n",
        "pack house/sensors (\n version = 1.0.0\n) {\n execute now\n}\n",
    ] {
        let document = parse_syntax_document(source);
        assert_eq!(document.diagnostics.len(), 1, "{source}");
        assert!(document.packages.is_empty());
    }
}

#[test]
fn legacy_package_keyword_is_not_a_compatibility_spelling() {
    let document = parse_syntax_document(
        "package house/sensors (\n version = 1.0.0\n) {\n ship temperature\n}\n",
    );
    assert!(document.packages.is_empty());
    assert_eq!(document.diagnostics.len(), 1);
    assert!(document.diagnostics[0].message.contains("'pack PATH'"));
}

#[test]
fn legacy_export_keyword_is_not_a_compatibility_spelling() {
    let document = parse_syntax_document(
        "pack house/sensors (\n version = 1.0.0\n) {\n export temperature\n}\n",
    );
    assert!(document.packages.is_empty());
    assert_eq!(document.diagnostics.len(), 1);
    assert!(document.diagnostics[0].message.contains("ship NAME"));
}

#[test]
fn legacy_require_and_quoted_versions_are_not_compatibility_spellings() {
    for source in [
        "pack house/sensors (\n version = \"1.0.0\"\n) {\n ship temperature\n}\n",
        "pack house/sensors (\n version = 1.0.0\n) {\n require math/units = \"^1.0\"\n}\n",
        "pack house/sensors (\n version = 1.0.0\n) {\n need math/units = \"^1.0\"\n}\n",
    ] {
        let document = parse_syntax_document(source);
        assert!(document.packages.is_empty(), "{source}");
        assert_eq!(document.diagnostics.len(), 1, "{source}");
    }
}

#[test]
fn checked_package_content_cannot_be_detached_from_its_parsed_source() {
    let first = source("test/package", "1.0.0", &[]);
    let second = source("test/package", "1.0.1", &[]);
    let syntax = parse_syntax_document(&first).packages[0].clone();
    assert_eq!(
        CheckedPackageSource::from_syntax(&second, &syntax),
        Err(PackageResolutionError::SourceMismatch)
    );
}

#[test]
fn deserialized_or_manually_constructed_package_truth_revalidates_fail_closed() {
    let source = source("test/package", "1.0.0", &[]);
    let mut checked = package(&source);
    checked.content_digest = [0; 32];
    assert_eq!(
        resolve_package_lock(&[checked], &[]),
        Err(PackageResolutionError::InvalidCheckedPackage(
            "test/package".into()
        ))
    );
}

#[test]
fn package_catalog_and_root_work_have_explicit_finite_admission_bounds() {
    let source = source("test/package", "1.0.0", &[]);
    let checked = package(&source);
    let catalog = vec![checked.clone(); MAXIMUM_PACKAGE_CATALOG_ENTRIES + 1];
    assert_eq!(
        resolve_package_lock(&catalog, &[]),
        Err(PackageResolutionError::CatalogLimitExceeded)
    );
    let roots = vec![("test/package", checked.version); MAXIMUM_PACKAGE_CATALOG_ENTRIES + 1];
    assert_eq!(
        resolve_package_lock(&[checked], &roots),
        Err(PackageResolutionError::CatalogLimitExceeded)
    );
}
