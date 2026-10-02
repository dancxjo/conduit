use super::*;

fn package_manifest(exports: &[&str]) -> (String, PackageSyntax) {
    let mut source = String::from("pack example/tools (\n    version = 1.0.0\n) {\n");
    for export in exports {
        source.push_str(&format!("    ship {export}\n"));
    }
    source.push_str("}\n");
    let document = parse_syntax_document(&source);
    (source, document.packages[0].clone())
}

#[test]
fn package_bundle_identity_covers_sorted_member_paths_and_exact_source() {
    let (manifest_source, manifest) = package_manifest(&["public"]);
    let public = "with ./support/helper\nplot public {\n helper: helper\n}\n";
    let support = "plot helper {\n}\n";
    let forward = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[
            PackageMemberSource {
                path: "main",
                source: public,
            },
            PackageMemberSource {
                path: "support",
                source: support,
            },
        ],
    )
    .unwrap();
    let reversed = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[
            PackageMemberSource {
                path: "support",
                source: support,
            },
            PackageMemberSource {
                path: "main",
                source: public,
            },
        ],
    )
    .unwrap();
    assert_eq!(forward, reversed);
    assert_eq!(forward.members[0].path, "main");
    assert_eq!(forward.members[0].local_requirements, ["support/helper"]);
    assert_eq!(
        forward.resolve_export("example/tools/public"),
        Some("public")
    );
    assert_eq!(forward.member_owning_plot("public"), Some("main"));
    assert_eq!(forward.resolve_export("example/tools/helper"), None);
    assert_eq!(forward.resolve_export("other/tools/public"), None);
    let exports = PackageExportCatalog::from_bundle(
        &forward,
        &manifest_source,
        &manifest,
        &[
            PackageMemberSource {
                path: "main",
                source: public,
            },
            PackageMemberSource {
                path: "support",
                source: support,
            },
        ],
    )
    .unwrap();
    assert_eq!(
        exports.resolve("example/tools/public").unwrap().name.text,
        "public"
    );
    assert!(exports.resolve("example/tools/helper").is_none());
    assert_eq!(
        exports.package_content_digest(),
        forward.package.content_digest
    );

    let changed = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[
            PackageMemberSource {
                path: "main",
                source: public,
            },
            PackageMemberSource {
                path: "support",
                source: "# exact source changes identity\nplot helper {\n}\n",
            },
        ],
    )
    .unwrap();
    assert_ne!(
        forward.package.content_digest,
        changed.package.content_digest
    );
    assert_ne!(
        forward.members[1].source_document_id,
        changed.members[1].source_document_id
    );

    let mut detached = forward.clone();
    detached.members[0].plots[0].push_str("-forged");
    assert_eq!(
        detached.validate_against(
            &manifest_source,
            &manifest,
            &[
                PackageMemberSource {
                    path: "main",
                    source: public,
                },
                PackageMemberSource {
                    path: "support",
                    source: support,
                },
            ],
        ),
        Err(PackageBundleError::SourceMismatch)
    );
}

#[test]
fn package_bundle_refuses_missing_exports_members_and_plots() {
    let (manifest_source, manifest) = package_manifest(&["absent"]);
    let missing_export = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[PackageMemberSource {
            path: "main",
            source: "plot present {\n}\n",
        }],
    )
    .unwrap_err();
    assert_eq!(
        missing_export,
        PackageBundleError::MissingExport("absent".into())
    );

    let (manifest_source, manifest) = package_manifest(&["public"]);
    let missing_member = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[PackageMemberSource {
            path: "main",
            source: "with ./missing/helper\nplot public {\n}\n",
        }],
    )
    .unwrap_err();
    assert_eq!(
        missing_member,
        PackageBundleError::MissingLocalMember("missing".into())
    );

    let missing_plot = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[
            PackageMemberSource {
                path: "main",
                source: "with ./support/absent\nplot public {\n}\n",
            },
            PackageMemberSource {
                path: "support",
                source: "plot present {\n}\n",
            },
        ],
    )
    .unwrap_err();
    assert_eq!(
        missing_plot,
        PackageBundleError::MissingLocalPlot("support/absent".into())
    );
}

#[test]
fn lexical_local_plot_cannot_become_a_pack_export() {
    let (manifest_source, manifest) = package_manifest(&["helper"]);
    let error = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[PackageMemberSource {
            path: "main",
            source: "plot public {\n plot helper {\n }\n child: helper\n}\n",
        }],
    )
    .unwrap_err();
    assert_eq!(error, PackageBundleError::MissingExport("helper".into()));
}

#[test]
fn package_bundle_refuses_local_module_cycles_and_duplicate_plots() {
    let (manifest_source, manifest) = package_manifest(&["left"]);
    let cycle = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[
            PackageMemberSource {
                path: "a",
                source: "with ./b/right\nplot left {\n}\n",
            },
            PackageMemberSource {
                path: "b",
                source: "with ./a/left\nplot right {\n}\n",
            },
        ],
    )
    .unwrap_err();
    assert_eq!(
        cycle,
        PackageBundleError::DependencyCycle(vec!["a".into(), "b".into(), "a".into()])
    );

    let duplicate = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[
            PackageMemberSource {
                path: "a",
                source: "plot left {\n}\n",
            },
            PackageMemberSource {
                path: "b",
                source: "plot left {\n}\n",
            },
        ],
    )
    .unwrap_err();
    assert_eq!(duplicate, PackageBundleError::DuplicatePlot("left".into()));
}

#[test]
fn export_source_name_remains_distinct_from_canonical_plot_name() {
    let (manifest_source, manifest) = package_manifest(&["temperature"]);
    let source = "plot climate/temperature {\n}\n";
    let bundle = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[PackageMemberSource {
            path: "temperature",
            source,
        }],
    )
    .unwrap();
    assert_eq!(
        bundle.resolve_export("example/tools/temperature"),
        Some("climate/temperature")
    );
    let exports = PackageExportCatalog::from_bundle(
        &bundle,
        &manifest_source,
        &manifest,
        &[PackageMemberSource {
            path: "temperature",
            source,
        }],
    )
    .unwrap();
    assert_eq!(
        exports
            .resolve("example/tools/temperature")
            .unwrap()
            .name
            .text,
        "climate/temperature"
    );

    let ambiguous = CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest,
        &[
            PackageMemberSource {
                path: "a",
                source: "plot climate/temperature {\n}\n",
            },
            PackageMemberSource {
                path: "b",
                source: "plot weather/temperature {\n}\n",
            },
        ],
    )
    .unwrap_err();
    assert_eq!(
        ambiguous,
        PackageBundleError::AmbiguousExport("temperature".into())
    );
}
