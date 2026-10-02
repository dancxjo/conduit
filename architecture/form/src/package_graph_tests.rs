use super::*;

fn package_manifest(
    path: &str,
    export: &str,
    requirements: &[(&str, &str)],
) -> (String, PackageSyntax) {
    let mut source = format!(
        "package {path} (\n    version = \"1.0.0\"\n) {{\n    export {export}\n"
    );
    for (dependency, version) in requirements {
        source.push_str(&format!("    require {dependency} = \"{version}\"\n"));
    }
    source.push_str("}\n");
    let document = parse_syntax_document(&source);
    (source, document.packages[0].clone())
}

fn version() -> PackageVersion {
    PackageVersion {
        major: 1,
        minor: 0,
        patch: 0,
    }
}

#[test]
fn exact_lock_resolves_package_export_glyph_to_canonical_form() {
    let (dependency_manifest_source, dependency_manifest) =
        package_manifest("text/tools", "upper", &[]);
    let dependency_member =
        "form text/upper (\n input: Text >> output: Text\n) {\n input >> output\n}\n";
    let dependency_members = [PackageMemberSource {
        path: "upper",
        source: dependency_member,
    }];
    let dependency = CheckedPackageBundle::from_sources(
        &dependency_manifest_source,
        &dependency_manifest,
        &dependency_members,
    )
    .unwrap();

    let (root_manifest_source, root_manifest) =
        package_manifest("example/app", "public", &[("text/tools", "1.0.0")]);
    let root_member = "without glyphs\nuse text/tools/upper as ^^\nform app/public (\n input: Text >> output: Text\n) {\n input ^^ output\n}\n";
    let root_members = [PackageMemberSource {
        path: "main",
        source: root_member,
    }];
    let root = CheckedPackageBundle::from_sources(
        &root_manifest_source,
        &root_manifest,
        &root_members,
    )
    .unwrap();
    let lock = resolve_package_lock(
        &[root.package.clone(), dependency.package.clone()],
        &[("example/app", version())],
    )
    .unwrap();
    let checked = check_package_graph(
        &[
            PackageGraphSource {
                bundle: &root,
                manifest_source: &root_manifest_source,
                manifest: &root_manifest,
                members: &root_members,
            },
            PackageGraphSource {
                bundle: &dependency,
                manifest_source: &dependency_manifest_source,
                manifest: &dependency_manifest,
                members: &dependency_members,
            },
        ],
        &lock,
        &StartupCatalog::new(),
    )
    .unwrap();
    let public = checked
        .forms
        .iter()
        .find(|form| form.name == "app/public")
        .unwrap();
    assert_eq!(public.gears[0].kind, "text/upper");
    assert_eq!(
        checked
            .forms
            .iter()
            .find(|form| form.name == "text/upper")
            .unwrap()
            .runtime_front,
        public.runtime_front
    );
}

#[test]
fn transitive_package_exports_are_not_ambiently_visible() {
    let (leaf_manifest_source, leaf_manifest) = package_manifest("leaf/tools", "work", &[]);
    let leaf_members = [PackageMemberSource {
        path: "work",
        source: "form leaf/work {\n}\n",
    }];
    let leaf = CheckedPackageBundle::from_sources(
        &leaf_manifest_source,
        &leaf_manifest,
        &leaf_members,
    )
    .unwrap();
    let (middle_manifest_source, middle_manifest) =
        package_manifest("middle/tools", "middle", &[("leaf/tools", "1.0.0")]);
    let middle_members = [PackageMemberSource {
        path: "middle",
        source: "form middle/work {\n}\n",
    }];
    let middle = CheckedPackageBundle::from_sources(
        &middle_manifest_source,
        &middle_manifest,
        &middle_members,
    )
    .unwrap();
    let (root_manifest_source, root_manifest) =
        package_manifest("root/app", "public", &[("middle/tools", "1.0.0")]);
    let root_members = [PackageMemberSource {
        path: "main",
        source: "use leaf/tools/work as stolen\nform root/public {\n gear: stolen\n}\n",
    }];
    let root = CheckedPackageBundle::from_sources(
        &root_manifest_source,
        &root_manifest,
        &root_members,
    )
    .unwrap();
    let packages = [
        root.package.clone(),
        middle.package.clone(),
        leaf.package.clone(),
    ];
    let lock = resolve_package_lock(&packages, &[("root/app", version())]).unwrap();
    let error = check_package_graph(
        &[
            PackageGraphSource {
                bundle: &root,
                manifest_source: &root_manifest_source,
                manifest: &root_manifest,
                members: &root_members,
            },
            PackageGraphSource {
                bundle: &middle,
                manifest_source: &middle_manifest_source,
                manifest: &middle_manifest,
                members: &middle_members,
            },
            PackageGraphSource {
                bundle: &leaf,
                manifest_source: &leaf_manifest_source,
                manifest: &leaf_manifest,
                members: &leaf_members,
            },
        ],
        &lock,
        &StartupCatalog::new(),
    )
    .unwrap_err();
    let PackageGraphError::Syntax { diagnostic, .. } = error else {
        panic!("expected exact source import refusal")
    };
    assert!(diagnostic.message.contains("does not resolve"));
}

#[test]
fn graph_refuses_a_bundle_not_selected_by_the_lock() {
    let (manifest_source, manifest) = package_manifest("example/app", "public", &[]);
    let members = [PackageMemberSource {
        path: "main",
        source: "form app/public {\n}\n",
    }];
    let bundle =
        CheckedPackageBundle::from_sources(&manifest_source, &manifest, &members).unwrap();
    let lock = resolve_package_lock(&[bundle.package.clone()], &[("example/app", version())])
        .unwrap();
    let mut stale = lock.clone();
    stale.packages[0].content_digest[0] ^= 1;
    let error = check_package_graph(
        &[PackageGraphSource {
            bundle: &bundle,
            manifest_source: &manifest_source,
            manifest: &manifest,
            members: &members,
        }],
        &stale,
        &StartupCatalog::new(),
    )
    .unwrap_err();
    assert!(matches!(error, PackageGraphError::Lock(_)));
}
