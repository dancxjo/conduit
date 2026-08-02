use conduit_panel::{
    ContractExportKind, ContractPackageArtifact, ContractPackageDependency, ContractPackageExport,
    ContractPackageLock, ContractPackageManifest, LoadedModule, LockedContractPackage,
    LockedExport, MAXIMUM_CONTRACT_PACKAGE_BYTES, MAXIMUM_CONTRACT_PACKAGE_EXPORTS, ModuleLoader,
    PackageImportSelection, parse, resolve_module_package_imports, resolve_modules,
    resolve_package_imports,
};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;

const HASH: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_HASH: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn export(name: &str, kind: ContractExportKind, public: bool) -> ContractPackageExport {
    ContractPackageExport {
        name: name.to_owned(),
        canonical_id: format!("conduit.dev/std/{name}"),
        kind,
        descriptor_hash: HASH.to_owned(),
        descriptor: serde_json::json!({
            "id": format!("conduit.dev/std/{name}"),
            "kind": match kind {
                ContractExportKind::Type => "type",
                ContractExportKind::Node => "node",
                ContractExportKind::Composite => "composite",
                ContractExportKind::Interface => "interface",
                ContractExportKind::Adapter => "adapter",
            },
            "ports": []
        }),
        public,
        structural_facets: vec!["conduit.dev/facets/stream".to_owned()],
        directional_obligations: vec!["input:value -> output:value".to_owned()],
        conformance_fixtures: vec!["conformance/standard-flow.json".to_owned()],
        lessons: vec!["tour/imports".to_owned()],
        successor: None,
        deprecated: false,
    }
}

fn artifact(dependencies: Vec<ContractPackageDependency>) -> (Vec<u8>, ContractPackageLock) {
    let manifest = ContractPackageManifest {
        schema: "conduit.contract-package".to_owned(),
        draft: 0,
        package_id: "conduit.dev/std".to_owned(),
        owner: "Conduit project".to_owned(),
        provenance: "repository:fixtures/contracts".to_owned(),
        license: "MIT OR Apache-2.0".to_owned(),
        dependencies,
        exports: vec![
            export("tee", ContractExportKind::Node, true),
            export("gate", ContractExportKind::Node, true),
            export("stream", ContractExportKind::Interface, true),
            export("reading", ContractExportKind::Type, true),
            export("internal", ContractExportKind::Node, false),
        ],
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    let exports = manifest
        .exports
        .iter()
        .map(|item| LockedExport {
            name: item.name.clone(),
            canonical_id: item.canonical_id.clone(),
            descriptor_hash: item.descriptor_hash.clone(),
        })
        .collect();
    (
        bytes,
        ContractPackageLock {
            schema: "conduit.contract-package-lock".to_owned(),
            draft: 0,
            packages: vec![LockedContractPackage {
                package_id: manifest.package_id,
                artifact_digest: digest,
                source: "repository fixture".to_owned(),
                provenance_policy: "repository-owned".to_owned(),
                exports,
            }],
        },
    )
}

fn seal_manifest(manifest: ContractPackageManifest) -> (Vec<u8>, LockedContractPackage) {
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let artifact_digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    let exports = manifest
        .exports
        .iter()
        .map(|item| LockedExport {
            name: item.name.clone(),
            canonical_id: item.canonical_id.clone(),
            descriptor_hash: item.descriptor_hash.clone(),
        })
        .collect();
    (
        bytes,
        LockedContractPackage {
            package_id: manifest.package_id,
            artifact_digest,
            source: "test fixture".to_owned(),
            provenance_policy: "repository-owned".to_owned(),
            exports,
        },
    )
}

#[test]
fn parser_preserves_named_aliases_and_exact_source_spans() {
    let panel = parse(
        "panel 0\n\
         import conduit.dev/std/{tee, gate as valve}\n\
         split: tee\n\
         check: valve\n",
    )
    .unwrap();
    assert_eq!(panel.package_imports.len(), 1);
    let import = &panel.package_imports[0];
    assert_eq!(import.target, "conduit.dev/std");
    let PackageImportSelection::Named(names) = &import.selection else {
        panic!("named selection");
    };
    assert_eq!(
        (names[0].export.as_str(), names[0].local.as_str()),
        ("tee", "tee")
    );
    assert_eq!(
        (names[1].export.as_str(), names[1].local.as_str()),
        ("gate", "valve")
    );
    assert_eq!((import.source_span.line, names[1].source_span.line), (2, 2));
}

#[test]
fn exact_lock_resolution_rewrites_only_semantic_references() {
    let panel = parse(
        "panel 0\n\
         import conduit.dev/std/{tee as split, stream}\n\
         source: split implements stream\n",
    )
    .unwrap();
    let (bytes, lock) = artifact(Vec::new());
    let resolution = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: Some("mirror-b"),
        }],
    )
    .unwrap();
    assert_eq!(resolution.panel().nodes[0].kind, "conduit.dev/std/tee");
    assert_eq!(
        resolution.panel().nodes[0].implements[0].interface,
        "conduit.dev/std/stream"
    );
    assert_eq!(resolution.bindings()[0].descriptor_hash, HASH);
    assert_eq!(resolution.packages()[0].mirror.as_deref(), Some("mirror-b"));
    assert_eq!(resolution.panel().package_imports, panel.package_imports);
}

#[test]
fn imported_types_lower_in_typed_source_positions_and_local_declarations_win_no_ambiguity() {
    let panel = parse(
        "panel 0\n\
         import conduit.dev/std/{reading as sample}\n\
         Envelope(value: sample) {}\n",
    )
    .unwrap();
    let (bytes, lock) = artifact(Vec::new());
    let resolution = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap();
    assert_eq!(
        resolution.panel().definitions[0].parameters[0].value_type,
        "conduit.dev/std/reading"
    );

    let colliding = parse(
        "panel 0\n\
         import conduit.dev/std/{tee as Part}\n\
         Part{}\n",
    )
    .unwrap();
    let failure = resolve_package_imports(
        &colliding,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-002");
    assert!(failure.source_span.is_some());
}

#[test]
fn qualified_package_alias_resolves_public_exports_but_not_private_surface() {
    let panel = parse(
        "panel 0\n\
         import conduit.dev/std as std\n\
         split: std.tee\n\
         check: std.gate\n",
    )
    .unwrap();
    let (bytes, lock) = artifact(Vec::new());
    let resolution = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap();
    assert_eq!(resolution.panel().nodes[0].kind, "conduit.dev/std/tee");
    assert_eq!(resolution.panel().nodes[1].kind, "conduit.dev/std/gate");
    assert!(
        resolution
            .bindings()
            .iter()
            .all(|binding| binding.local_name != "std.internal")
    );
}

#[test]
fn duplicate_missing_hidden_and_descriptor_mismatch_fail_deterministically() {
    let duplicate = parse(
        "panel 0\n\
         import conduit.dev/std/{tee as part, gate as part}\n",
    )
    .unwrap_err();
    assert_eq!(duplicate.code, "CND-SRC-002");

    let (bytes, lock) = artifact(Vec::new());
    for (source, code) in [
        ("panel 0\nimport conduit.dev/std/{absent}\n", "CND-IPK-004"),
        (
            "panel 0\nimport conduit.dev/std/{internal}\n",
            "CND-IPK-006",
        ),
    ] {
        let panel = parse(source).unwrap();
        let failure = resolve_package_imports(
            &panel,
            &lock,
            &[ContractPackageArtifact {
                bytes: &bytes,
                mirror: None,
            }],
        )
        .unwrap_err();
        assert_eq!(failure.code, code);
        assert!(failure.source_span.is_some());
    }

    let panel = parse("panel 0\nimport conduit.dev/std/{tee}\n").unwrap();
    let mut mismatched = lock.clone();
    mismatched.packages[0].exports[0].descriptor_hash = OTHER_HASH.to_owned();
    let failure = resolve_package_imports(
        &panel,
        &mismatched,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-005");
}

#[test]
fn bytes_are_location_independent_and_mutation_or_missing_transitive_data_is_rejected() {
    let panel = parse("panel 0\nimport conduit.dev/std/{tee}\n").unwrap();
    let (bytes, lock) = artifact(Vec::new());
    let first = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: Some("mirror-a"),
        }],
    )
    .unwrap();
    let second = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: Some("mirror-b"),
        }],
    )
    .unwrap();
    assert_eq!(
        first.packages()[0].artifact_digest,
        second.packages()[0].artifact_digest
    );
    assert_eq!(first.bindings(), second.bindings());
    let both_mirrors = resolve_package_imports(
        &panel,
        &lock,
        &[
            ContractPackageArtifact {
                bytes: &bytes,
                mirror: Some("mirror-a"),
            },
            ContractPackageArtifact {
                bytes: &bytes,
                mirror: Some("mirror-b"),
            },
        ],
    )
    .unwrap();
    assert_eq!(first.bindings(), both_mirrors.bindings());

    let mut mutated = bytes.clone();
    mutated.push(b' ');
    let failure = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &mutated,
            mirror: None,
        }],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-003");

    let dependency = ContractPackageDependency {
        package_id: "foreign.dev/types".to_owned(),
        artifact_digest: OTHER_HASH.to_owned(),
    };
    let (dependent_bytes, dependent_lock) = artifact(vec![dependency]);
    let failure = resolve_package_imports(
        &panel,
        &dependent_lock,
        &[ContractPackageArtifact {
            bytes: &dependent_bytes,
            mirror: None,
        }],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-004");
}

#[test]
fn package_and_source_import_bounds_fail_before_unbounded_retention() {
    let panel = parse("panel 0\nimport conduit.dev/std/{tee}\n").unwrap();
    let oversized = vec![b' '; MAXIMUM_CONTRACT_PACKAGE_BYTES + 1];
    let (_, lock) = artifact(Vec::new());
    let failure = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &oversized,
            mirror: None,
        }],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-008");

    let names = (0..=conduit_panel::MAXIMUM_PACKAGE_IMPORT_NAMES)
        .map(|index| format!("name{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("panel 0\nimport example.dev/parts/{{{names}}}\n");
    let failure = parse(&source).unwrap_err();
    assert_eq!(failure.code, "CND-SEC-001");

    let imports = (0..=conduit_panel::MAXIMUM_PACKAGE_IMPORTS)
        .map(|index| format!("import example.dev/parts/{{probe as probe{index}}}\n"))
        .collect::<String>();
    let failure = parse(&format!("panel 0\n{imports}")).unwrap_err();
    assert_eq!(failure.code, "CND-SEC-001");

    let mut oversized_lock = lock;
    oversized_lock.packages[0].exports =
        vec![oversized_lock.packages[0].exports[0].clone(); MAXIMUM_CONTRACT_PACKAGE_EXPORTS + 1];
    let failure = resolve_package_imports(&panel, &oversized_lock, &[]).unwrap_err();
    assert_eq!(failure.code, "CND-IPK-008");
}

#[test]
fn checked_repository_artifact_and_lock_resolve_the_tour_alias_offline() {
    let panel = parse(include_str!(
        "../../../fixtures/contract-package-imports/alias.panel"
    ))
    .unwrap();
    let lock: ContractPackageLock = serde_json::from_str(include_str!(
        "../../../fixtures/contract-package-imports/contract-package-lock.json"
    ))
    .unwrap();
    let bytes = include_bytes!("../../../fixtures/contract-package-imports/conduit-dev-std.json");
    let resolution = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes,
            mirror: Some("repository"),
        }],
    )
    .unwrap();
    assert_eq!(resolution.bindings()[0].local_name, "split");
    assert_eq!(resolution.panel().nodes[0].kind, "conduit.dev/std/tee");
}

#[test]
fn bounded_control_composites_are_consumable_as_one_current_contract_package() {
    let bytes = include_bytes!("../../../contract-packages/conduit-std-control.json");
    let manifest: ContractPackageManifest = serde_json::from_slice(bytes).unwrap();
    assert_eq!(manifest.package_id, "conduit.std/control");
    assert!(manifest.exports.iter().all(|export| {
        export.kind == ContractExportKind::Composite
            && export.canonical_id == format!("{}/{}", manifest.package_id, export.name)
            && export.successor.is_none()
            && !export.deprecated
    }));
    let lock = ContractPackageLock {
        schema: "conduit.contract-package-lock".to_owned(),
        draft: 0,
        packages: vec![LockedContractPackage {
            package_id: manifest.package_id.clone(),
            artifact_digest: format!("sha256:{:x}", Sha256::digest(bytes)),
            source: "repository contract package".to_owned(),
            provenance_policy: "repository-owned".to_owned(),
            exports: manifest
                .exports
                .iter()
                .map(|export| LockedExport {
                    name: export.name.clone(),
                    canonical_id: export.canonical_id.clone(),
                    descriptor_hash: export.descriptor_hash.clone(),
                })
                .collect(),
        }],
    };
    let panel = parse(
        "panel 0\n\
         import conduit.std/control/{request-reply as exchange, cancellable-action as action}\n\
         request: exchange\n\
         goal: action\n",
    )
    .unwrap();
    let resolution = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes,
            mirror: Some("repository"),
        }],
    )
    .unwrap();
    assert_eq!(resolution.bindings().len(), 2);
    assert_eq!(
        resolution.panel().nodes[0].kind,
        "conduit.std/control/request-reply"
    );
    assert_eq!(
        resolution.panel().nodes[1].kind,
        "conduit.std/control/cancellable-action"
    );
}

#[test]
fn a_target_that_can_name_both_a_package_and_parent_export_is_ambiguous() {
    let manifest = |package_id: &str, name: &str| ContractPackageManifest {
        schema: "conduit.contract-package".to_owned(),
        draft: 0,
        package_id: package_id.to_owned(),
        owner: "fixture owner".to_owned(),
        provenance: "repository:test".to_owned(),
        license: "MIT".to_owned(),
        dependencies: Vec::new(),
        exports: vec![ContractPackageExport {
            name: name.to_owned(),
            canonical_id: format!("{package_id}/{name}"),
            kind: ContractExportKind::Node,
            descriptor_hash: HASH.to_owned(),
            descriptor: serde_json::json!({
                "id": format!("{package_id}/{name}"),
                "kind": "node",
                "ports": []
            }),
            public: true,
            structural_facets: Vec::new(),
            directional_obligations: Vec::new(),
            conformance_fixtures: Vec::new(),
            lessons: Vec::new(),
            successor: None,
            deprecated: false,
        }],
    };
    let (parent_bytes, parent_lock) = seal_manifest(manifest("example.dev", "std"));
    let (child_bytes, child_lock) = seal_manifest(manifest("example.dev/std", "tee"));
    let lock = ContractPackageLock {
        schema: "conduit.contract-package-lock".to_owned(),
        draft: 0,
        packages: vec![parent_lock, child_lock],
    };
    let panel = parse("panel 0\nimport example.dev/std as local\n").unwrap();
    let failure = resolve_package_imports(
        &panel,
        &lock,
        &[
            ContractPackageArtifact {
                bytes: &parent_bytes,
                mirror: None,
            },
            ContractPackageArtifact {
                bytes: &child_bytes,
                mirror: None,
            },
        ],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-002");
    assert!(failure.source_span.is_some());
}

#[test]
fn explicit_local_module_closure_resolves_package_names_without_loading_more_source() {
    struct Loader(BTreeMap<String, String>);
    impl ModuleLoader for Loader {
        fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
            Ok(self.0.get(canonical_uri).map(|source| LoadedModule {
                canonical_uri: canonical_uri.to_owned(),
                source: source.clone(),
            }))
        }
    }
    let loader = Loader(BTreeMap::from([
        (
            "mem://fixture/root.panel".to_owned(),
            "panel 0\nimport \"./child.panel\" as child\nroot: child.Part\n".to_owned(),
        ),
        (
            "mem://fixture/child.panel".to_owned(),
            "panel 0\nimport conduit.dev/std/{tee as split}\nPart {\nbranch: split\n}\n".to_owned(),
        ),
    ]));
    let graph = resolve_modules("mem://fixture/root.panel", None, &loader).unwrap();
    let (bytes, lock) = artifact(Vec::new());
    let resolved = resolve_module_package_imports(
        &graph,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap();
    let child = resolved
        .graph
        .modules
        .iter()
        .find(|module| module.canonical_uri.ends_with("child.panel"))
        .unwrap();
    assert_eq!(
        child.panel.definitions[0].nodes[0].kind,
        "conduit.dev/std/tee"
    );
    assert_eq!(resolved.bindings[0].module_uri, child.canonical_uri);
}

#[test]
fn semantic_descriptors_cannot_smuggle_fetch_install_or_authority_instructions() {
    let mut manifest = ContractPackageManifest {
        schema: "conduit.contract-package".to_owned(),
        draft: 0,
        package_id: "example.dev/parts".to_owned(),
        owner: "fixture owner".to_owned(),
        provenance: "repository:test".to_owned(),
        license: "MIT".to_owned(),
        dependencies: Vec::new(),
        exports: vec![ContractPackageExport {
            name: "probe".to_owned(),
            canonical_id: "example.dev/parts/probe".to_owned(),
            kind: ContractExportKind::Node,
            descriptor_hash: HASH.to_owned(),
            descriptor: serde_json::json!({
                "id": "example.dev/parts/probe",
                "kind": "node",
                "ports": [],
                "url": "https://example.invalid/provider"
            }),
            public: true,
            structural_facets: Vec::new(),
            directional_obligations: Vec::new(),
            conformance_fixtures: Vec::new(),
            lessons: Vec::new(),
            successor: None,
            deprecated: false,
        }],
    };
    let (bytes, locked) = seal_manifest(manifest.clone());
    let lock = ContractPackageLock {
        schema: "conduit.contract-package-lock".to_owned(),
        draft: 0,
        packages: vec![locked],
    };
    let panel = parse("panel 0\nimport example.dev/parts/{probe}\n").unwrap();
    let failure = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-001");

    manifest.exports[0].descriptor = serde_json::json!({
        "id": "example.dev/parts/probe",
        "kind": "node",
        "ports": [],
        "metadata": {
            "install": {
                "url": "https://example.invalid/provider"
            }
        }
    });
    let (bytes, locked) = seal_manifest(manifest.clone());
    let lock = ContractPackageLock {
        schema: "conduit.contract-package-lock".to_owned(),
        draft: 0,
        packages: vec![locked],
    };
    let failure = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-001");

    manifest.exports[0].descriptor = serde_json::json!({
        "id": "other.dev/parts/probe",
        "kind": "node",
        "ports": []
    });
    let (bytes, locked) = seal_manifest(manifest);
    let lock = ContractPackageLock {
        schema: "conduit.contract-package-lock".to_owned(),
        draft: 0,
        packages: vec![locked],
    };
    let failure = resolve_package_imports(
        &panel,
        &lock,
        &[ContractPackageArtifact {
            bytes: &bytes,
            mirror: None,
        }],
    )
    .unwrap_err();
    assert_eq!(failure.code, "CND-IPK-001");
}
