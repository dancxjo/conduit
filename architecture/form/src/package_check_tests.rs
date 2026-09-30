use super::*;

fn manifest() -> (String, PackageSyntax) {
    let source = "pack example/tools (\n    version = 1.0.0\n) {\n    ship public\n}\n".to_string();
    let document = parse_syntax_document(&source);
    (source, document.packages[0].clone())
}

const SUPPORT: &str = "form helper (\n input: Text >> output: Text\n) {\n input >> output\n}\n";
const PUBLIC_WITH_GLYPH: &str = "sans glyphs\nwith ./support/helper as ^^\nform public (\n input: Text >> output: Text\n) {\n input ^^ output\n}\n";

#[test]
fn package_check_retains_checked_codes() {
    let (manifest_source, manifest) = manifest();
    let source = "type Outcome =\n    ready\n    | refused\ncode example/outcome = Outcome as u8\nform public {\n}\n";
    let sources = [PackageMemberSource {
        path: "main",
        source,
    }];
    let bundle = CheckedPackageBundle::from_sources(&manifest_source, &manifest, &sources).unwrap();
    let checked = check_package_bundle(
        &bundle,
        &manifest_source,
        &manifest,
        &sources,
        &StartupCatalog::new(),
    )
    .unwrap();
    assert_eq!(checked.codes.len(), 1);
    assert_eq!(checked.codes[0].name, "example/outcome");
}

#[test]
fn explicit_local_glyph_import_checks_as_the_exact_ordinary_form() {
    let (manifest_source, manifest) = manifest();
    let sources = [
        PackageMemberSource {
            path: "main",
            source: PUBLIC_WITH_GLYPH,
        },
        PackageMemberSource {
            path: "support",
            source: SUPPORT,
        },
    ];
    let bundle = CheckedPackageBundle::from_sources(&manifest_source, &manifest, &sources).unwrap();
    let checked = check_package_bundle(
        &bundle,
        &manifest_source,
        &manifest,
        &sources,
        &StartupCatalog::new(),
    )
    .unwrap();
    let public = checked
        .forms
        .iter()
        .find(|form| form.name == "public")
        .unwrap();
    assert_eq!(public.gears[0].kind, "helper");

    let direct = check_syntax_document(
        &parse_syntax_document(&format!(
            "{SUPPORT}form public (\n input: Text >> output: Text\n) {{\n input >> helper() >> output\n}}\n"
        )),
        &StartupCatalog::new(),
    )
    .unwrap();
    let direct_public = direct
        .forms
        .iter()
        .find(|form| form.name == "public")
        .unwrap();
    assert_eq!(public.checked_form_id, direct_public.checked_form_id);
    assert_ne!(checked.source_document_id, direct.source_document_id);
}

#[test]
fn cross_module_form_is_not_ambiently_visible() {
    let (manifest_source, manifest) = manifest();
    let main =
        "form public (\n input: Text >> output: Text\n) {\n input >> helper() >> output\n}\n";
    let sources = [
        PackageMemberSource {
            path: "main",
            source: main,
        },
        PackageMemberSource {
            path: "support",
            source: SUPPORT,
        },
    ];
    let bundle = CheckedPackageBundle::from_sources(&manifest_source, &manifest, &sources).unwrap();
    let error = check_package_bundle(
        &bundle,
        &manifest_source,
        &manifest,
        &sources,
        &StartupCatalog::new(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        PackageCheckError::AmbientCrossModuleReference {
            module: "main".into(),
            form: "helper".into(),
        }
    );
}

#[test]
fn explicit_local_import_resolves_a_pool_member_form() {
    let (manifest_source, manifest) = manifest();
    let main =
        "with ./support/helper as worker\nform public {\n pool workers: worker(size = 2)\n}\n";
    let sources = [
        PackageMemberSource {
            path: "main",
            source: main,
        },
        PackageMemberSource {
            path: "support",
            source: SUPPORT,
        },
    ];
    let bundle = CheckedPackageBundle::from_sources(&manifest_source, &manifest, &sources).unwrap();
    let checked = check_package_bundle(
        &bundle,
        &manifest_source,
        &manifest,
        &sources,
        &StartupCatalog::new(),
    )
    .unwrap();
    let public = checked
        .forms
        .iter()
        .find(|form| form.name == "public")
        .unwrap();
    assert_eq!(public.pools[0].member_form, "helper");
}

#[test]
fn an_unimported_external_form_name_does_not_create_an_ambient_binding() {
    let (manifest_source, manifest) = manifest();
    let main = "form public (\n helper: Text >> output: Text\n) {\n helper >> output\n}\n";
    let sources = [
        PackageMemberSource {
            path: "main",
            source: main,
        },
        PackageMemberSource {
            path: "support",
            source: SUPPORT,
        },
    ];
    let bundle = CheckedPackageBundle::from_sources(&manifest_source, &manifest, &sources).unwrap();
    check_package_bundle(
        &bundle,
        &manifest_source,
        &manifest,
        &sources,
        &StartupCatalog::new(),
    )
    .expect("a local port is not shadowed by an unimported pack Form");
}

#[test]
fn local_source_path_does_not_replace_the_canonical_form_name() {
    let (manifest_source, manifest) = manifest();
    let main = "sans glyphs\nwith ./support/helper as ^^\nform public (\n input: Text >> output: Text\n) {\n input ^^ output\n}\n";
    let support = "form utility/helper (\n input: Text >> output: Text\n) {\n input >> output\n}\n";
    let sources = [
        PackageMemberSource {
            path: "main",
            source: main,
        },
        PackageMemberSource {
            path: "support",
            source: support,
        },
    ];
    let bundle = CheckedPackageBundle::from_sources(&manifest_source, &manifest, &sources).unwrap();
    assert_eq!(
        bundle.resolve_local_requirement("support/helper"),
        Some("utility/helper")
    );
    let checked = check_package_bundle(
        &bundle,
        &manifest_source,
        &manifest,
        &sources,
        &StartupCatalog::new(),
    )
    .unwrap();
    let public = checked
        .forms
        .iter()
        .find(|form| form.name == "public")
        .unwrap();
    assert_eq!(public.gears[0].kind, "utility/helper");
}

#[test]
fn pack_can_define_ship_and_use_native_semantic_types() {
    let manifest_source = "pack example/music (\n    version = 1.0.0\n) {\n    ship Note\n    ship Position\n    ship MusicEvent\n    ship public\n}\n";
    let manifest_document = parse_syntax_document(manifest_source);
    let manifest = &manifest_document.packages[0];
    let source = r#"type Note = U8 in 0..=127
type Position = {
    x: Distance
    y: Distance
}
type MusicEvent =
    note {
        velocity: U8 in 0..=127
        pitches: sequence Note <= 16
    }
    | rest
form public (
    >> note: Note
    >> position: Position
    event: MusicEvent >>
) {
}
"#;
    let sources = [PackageMemberSource {
        path: "main",
        source,
    }];
    let bundle = CheckedPackageBundle::from_sources(manifest_source, manifest, &sources).unwrap();

    assert_eq!(
        bundle.resolve_type_export("example/music/Note"),
        Some("Note")
    );
    assert_eq!(
        bundle.resolve_export("example/music/public"),
        Some("public")
    );
    let exports =
        PackageExportCatalog::from_bundle(&bundle, manifest_source, manifest, &sources).unwrap();
    assert_eq!(
        exports
            .resolve_type("example/music/MusicEvent")
            .unwrap()
            .name
            .text,
        "MusicEvent"
    );

    let checked = check_package_bundle(
        &bundle,
        manifest_source,
        manifest,
        &sources,
        &StartupCatalog::new(),
    )
    .unwrap();
    assert_eq!(checked.native_types.len(), 3);
    assert_eq!(checked.forms.len(), 1);

    let mut downstream_catalog = StartupCatalog::new();
    let shipped = exports
        .install_shipped_types(&mut downstream_catalog)
        .unwrap();
    let downstream = check_syntax_document(
        &parse_syntax_document(
            "with example/music/Note as Pitch\nform consumer (\n    >> value: Pitch\n) {\n}\n",
        ),
        &downstream_catalog,
    )
    .unwrap();
    assert_eq!(
        downstream.forms[0].checked_front().inputs()[0].value_kind,
        shipped
            .iter()
            .find(|value_type| value_type.name == "Note")
            .unwrap()
            .value_type
            .profile()
            .unwrap()
            .value_kind()
            .clone()
    );

    let direct =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    assert_eq!(
        checked
            .native_types
            .iter()
            .map(|value_type| &value_type.identity)
            .collect::<Vec<_>>(),
        direct
            .native_types
            .iter()
            .map(|value_type| &value_type.identity)
            .collect::<Vec<_>>()
    );
}
