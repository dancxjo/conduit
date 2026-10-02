use conduit_host_make::*;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn manifest(
    id: &str,
    version: &str,
    dependencies: Vec<SourcePackageRequirement>,
) -> SourcePackageManifest {
    SourcePackageManifest {
        schema: SOURCE_PACKAGE_MANIFEST_SCHEMA.into(),
        package_id: id.into(),
        package_version: version.into(),
        language_compatibility: "conduit-language@1".into(),
        source_roots: vec!["src".into()],
        exports: vec![SourcePackageExport {
            public_name: format!("{id}/main"),
            source_path: "src/main.conduit".into(),
            kind: SourceExportKind::Plot,
        }],
        dependencies,
        content_files: vec!["src/main.conduit".into()],
    }
}

fn locked(
    manifest: &SourcePackageManifest,
    content: &[u8],
    dependencies: Vec<LockedSourceDependency>,
    carrier: SourcePackageCarrier,
) -> LockedSourcePackage {
    LockedSourcePackage {
        package_id: manifest.package_id.clone(),
        package_version: manifest.package_version.clone(),
        manifest_sha256: manifest.canonical_sha256().unwrap(),
        content_sha256: digest(content),
        content_bytes: content.len() as u64,
        dependencies,
        carrier,
    }
}

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-source-package-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn canonical_lock_keeps_package_semantics_content_and_carrier_distinct() {
    let leaf = manifest("example/math", "1.0.0", vec![]);
    let root = manifest(
        "example/lesson",
        "2.0.0",
        vec![SourcePackageRequirement {
            package_id: leaf.package_id.clone(),
            version_requirement: leaf.package_version.clone(),
        }],
    );
    let leaf_content = b"plot vector { }.";
    let root_content = b"plot lesson { }.";
    let lock = SourcePackageLock {
        schema: SOURCE_PACKAGE_LOCK_SCHEMA.into(),
        roots: vec![root.package_id.clone()],
        packages: vec![
            locked(
                &root,
                root_content,
                vec![LockedSourceDependency {
                    package_id: leaf.package_id.clone(),
                    package_version: leaf.package_version.clone(),
                }],
                SourcePackageCarrier::Archive {
                    registry_id: "reviewed-registry".into(),
                    relative_path: "example/lesson-2.0.0.conduit-package".into(),
                },
            ),
            locked(
                &leaf,
                leaf_content,
                vec![],
                SourcePackageCarrier::Git {
                    repository: "https://example.invalid/math.git".into(),
                    commit: "a".repeat(40),
                },
            ),
        ],
    };
    let bytes = lock.canonical_bytes(&[root.clone(), leaf.clone()]).unwrap();
    let decoded: SourcePackageLock = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, lock);
    assert_eq!(decoded.validate(&[root, leaf]), Ok(()));
    assert_ne!(lock.packages[0].package_id, lock.packages[0].content_sha256);
    assert_ne!(lock.packages[0].carrier, lock.packages[1].carrier);
}

#[test]
fn manifests_and_locks_refuse_ambient_or_ambiguous_package_truth() {
    let mut invalid = manifest("example/package", "1.0.0", vec![]);
    invalid.source_roots = vec!["/home/person/source".into()];
    assert_eq!(invalid.validate(), Err(SourcePackageRefusal::InvalidPath));
    invalid.source_roots = vec![r"C:\Users\person\source".into()];
    assert_eq!(invalid.validate(), Err(SourcePackageRefusal::InvalidPath));

    let mut duplicate_export = manifest("example/package", "1.0.0", vec![]);
    duplicate_export
        .exports
        .push(duplicate_export.exports[0].clone());
    assert_eq!(
        duplicate_export.validate(),
        Err(SourcePackageRefusal::DuplicateExport)
    );

    let dependency = SourcePackageRequirement {
        package_id: "example/dependency".into(),
        version_requirement: "1.0.0".into(),
    };
    let duplicate_dependency = manifest(
        "example/package",
        "1.0.0",
        vec![dependency.clone(), dependency],
    );
    assert_eq!(
        duplicate_dependency.validate(),
        Err(SourcePackageRefusal::DuplicateDependency)
    );

    let manifest = manifest("example/package", "1.0.0", vec![]);
    let content = b"plot package { }.";
    let mut package = locked(
        &manifest,
        content,
        vec![],
        SourcePackageCarrier::Git {
            repository: "https://token@example.invalid/package.git".into(),
            commit: "b".repeat(40),
        },
    );
    let lock = |package| SourcePackageLock {
        schema: SOURCE_PACKAGE_LOCK_SCHEMA.into(),
        roots: vec![manifest.package_id.clone()],
        packages: vec![package],
    };
    assert_eq!(
        lock(package.clone()).validate(std::slice::from_ref(&manifest)),
        Err(SourcePackageRefusal::InvalidPath)
    );
    package.carrier = SourcePackageCarrier::LocalWorkspace {
        relative_path: "packages/example".into(),
    };
    package.content_sha256 = digest(&vec![b'x'; content.len()]);
    let cache = temporary("mismatch");
    assert_eq!(
        acquire_source_package_with(&package, &cache, || Ok(content.to_vec())),
        Err(SourcePackageRefusal::ContentMismatch)
    );
    let _ = fs::remove_dir_all(cache);

    let with_script = serde_json::json!({
        "schema": SOURCE_PACKAGE_MANIFEST_SCHEMA,
        "package_id": "example/package",
        "package_version": "1.0.0",
        "language_compatibility": "conduit-language@1",
        "source_roots": ["src"],
        "exports": [],
        "dependencies": [],
        "content_files": ["src/main.conduit"],
        "install_scripts": ["do-anything"]
    });
    assert!(serde_json::from_value::<SourcePackageManifest>(with_script).is_err());
}

#[test]
fn cycles_and_inexact_dependency_resolutions_refuse() {
    let a = manifest(
        "a",
        "1",
        vec![SourcePackageRequirement {
            package_id: "b".into(),
            version_requirement: "1".into(),
        }],
    );
    let b = manifest(
        "b",
        "1",
        vec![SourcePackageRequirement {
            package_id: "a".into(),
            version_requirement: "1".into(),
        }],
    );
    let dependency = |id: &str| {
        vec![LockedSourceDependency {
            package_id: id.into(),
            package_version: "1".into(),
        }]
    };
    let lock = SourcePackageLock {
        schema: SOURCE_PACKAGE_LOCK_SCHEMA.into(),
        roots: vec!["a".into()],
        packages: vec![
            locked(
                &a,
                b"a",
                dependency("b"),
                SourcePackageCarrier::LocalWorkspace {
                    relative_path: "a".into(),
                },
            ),
            locked(
                &b,
                b"b",
                dependency("a"),
                SourcePackageCarrier::LocalWorkspace {
                    relative_path: "b".into(),
                },
            ),
        ],
    };
    assert_eq!(
        lock.validate(&[a, b]),
        Err(SourcePackageRefusal::DependencyCycle)
    );
}

#[test]
fn identical_content_crosses_carriers_and_locked_cache_validates_offline() {
    let manifest = manifest("portable/tone", "1.2.3", vec![]);
    let content = b"plot tone { }.";
    let local = locked(
        &manifest,
        content,
        vec![],
        SourcePackageCarrier::LocalWorkspace {
            relative_path: "packages/tone".into(),
        },
    );
    let archive = LockedSourcePackage {
        carrier: SourcePackageCarrier::Archive {
            registry_id: "mirror-a".into(),
            relative_path: "portable/tone-1.2.3.conduit-package".into(),
        },
        ..local.clone()
    };
    let local_cache = temporary("local-carrier");
    let archive_cache = temporary("archive-carrier");
    let local_path =
        acquire_source_package_with(&local, &local_cache, || Ok(content.to_vec())).unwrap();
    let archive_path =
        acquire_source_package_with(&archive, &archive_cache, || Ok(content.to_vec())).unwrap();
    assert_eq!(
        fs::read(local_path).unwrap(),
        fs::read(archive_path).unwrap()
    );
    assert_eq!(local.content_sha256, archive.content_sha256);

    let lock = SourcePackageLock {
        schema: SOURCE_PACKAGE_LOCK_SCHEMA.into(),
        roots: vec![local.package_id.clone()],
        packages: vec![local],
    };
    let paths = validate_locked_source_cache(&lock, &[manifest], &local_cache).unwrap();
    assert_eq!(paths.len(), 1);

    let _ = fs::remove_dir_all(local_cache);
    let _ = fs::remove_dir_all(archive_cache);
}
