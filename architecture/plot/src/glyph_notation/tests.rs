use crate::glyph_notation_test_support::{fixture, MANIFEST, SOURCE};

fn package(
    manifest_source: &str,
) -> (
    crate::CheckedPackageBundle,
    crate::PackageExportCatalog,
    crate::PackageSyntax,
) {
    let document = crate::parse_syntax_document(manifest_source);
    let manifest = document.packages[0].clone();
    let members = [crate::PackageMemberSource {
        path: "glyph/notation",
        source: SOURCE,
    }];
    let bundle =
        crate::CheckedPackageBundle::from_sources(manifest_source, &manifest, &members).unwrap();
    let exports =
        crate::PackageExportCatalog::from_bundle(&bundle, manifest_source, &manifest, &members)
            .unwrap();
    (bundle, exports, manifest)
}

#[test]
fn installed_source_declarations_check_and_retain_owner_truth_for_inspection() {
    let (mut startup, profile, _) = fixture();
    let (bundle, exports, manifest) = package(MANIFEST);
    exports
        .install_shipped_glyph_notations(&mut startup, &profile)
        .unwrap();
    let before = startup.clone();
    let document = crate::parse_syntax_document(SOURCE);
    let checked = crate::check_syntax_document(&document, &startup).unwrap();
    assert!(checked.plots.is_empty());
    assert_eq!(checked.glyph_notations.len(), 1);
    let declaration = &checked.glyph_notations[0];
    assert_eq!(declaration.name, "notation");
    assert_eq!(declaration.source_path, "fixture/glyph/notation");
    assert_eq!(
        declaration.family,
        *startup
            .typed_literal_family(&declaration.source_path)
            .unwrap()
    );
    assert_eq!(declaration.source_span, document.glyph_notations[0].span);
    assert_eq!(
        declaration.family.origin.source_document_id,
        document.source_document_id()
    );
    let members = [crate::PackageMemberSource {
        path: "glyph/notation",
        source: SOURCE,
    }];
    let checked_package =
        crate::check_package_bundle(&bundle, MANIFEST, &manifest, &members, &startup).unwrap();
    assert_eq!(checked_package.glyph_notations, checked.glyph_notations);
    assert_eq!(startup, before);
}

#[test]
fn changed_source_and_spoofed_metadata_refuse_without_mutating_catalogue() {
    let (base, profile, _) = fixture();
    let (_, exports, _) = package(MANIFEST);
    let mut startup = base.clone();
    exports
        .install_shipped_glyph_notations(&mut startup, &profile)
        .unwrap();
    let before = startup.clone();
    let changed = SOURCE.replace("fixture/parser@1", "fixture/parser@2");
    let failure = crate::check_syntax_document(&crate::parse_syntax_document(&changed), &startup)
        .unwrap_err();
    assert_eq!(failure.code, "CND-FRM-062");
    assert_eq!(startup, before);
    let mut family = startup
        .typed_literal_family("fixture/glyph/notation")
        .unwrap()
        .clone();
    family.branches[0].parser_contract = "fixture/spoof@1".into();
    let mut spoofed = base;
    spoofed
        .insert_typed_literal_family("fixture/glyph/notation", family, &profile)
        .unwrap();
    let before = spoofed.clone();
    let failure =
        crate::check_syntax_document(&crate::parse_syntax_document(SOURCE), &spoofed).unwrap_err();
    assert!(failure.message.contains("differs from installed"));
    assert_eq!(spoofed, before);
}

#[test]
fn identical_source_in_two_packages_requires_exact_package_context() {
    let (mut startup, profile, _) = fixture();
    let (bundle, exports, manifest) = package(MANIFEST);
    exports
        .install_shipped_glyph_notations(&mut startup, &profile)
        .unwrap();
    let other_manifest_source = MANIFEST.replace("fixture/glyph", "fixture/other");
    let (other_bundle, other_exports, other_manifest) = package(&other_manifest_source);
    let members = [crate::PackageMemberSource {
        path: "glyph/notation",
        source: SOURCE,
    }];
    let foreign = crate::check_package_bundle(
        &other_bundle,
        &other_manifest_source,
        &other_manifest,
        &members,
        &startup,
    )
    .unwrap_err();
    assert!(
        matches!(foreign, crate::PackageCheckError::Syntax { diagnostic, .. } if diagnostic.code == "CND-FRM-062")
    );
    other_exports
        .install_shipped_glyph_notations(&mut startup, &profile)
        .unwrap();
    let before = startup.clone();
    let failure =
        crate::check_syntax_document(&crate::parse_syntax_document(SOURCE), &startup).unwrap_err();
    assert!(failure.message.contains("more than one package"));
    for (bundle, source, manifest, path) in [
        (&bundle, MANIFEST, &manifest, "fixture/glyph/notation"),
        (
            &other_bundle,
            other_manifest_source.as_str(),
            &other_manifest,
            "fixture/other/notation",
        ),
    ] {
        let checked =
            crate::check_package_bundle(bundle, source, manifest, &members, &startup).unwrap();
        assert_eq!(checked.glyph_notations[0].source_path, path);
        assert_eq!(
            checked.glyph_notations[0]
                .family
                .origin
                .package_content_digest,
            bundle.package.content_digest
        );
    }
    assert_eq!(startup, before);
}

#[test]
fn duplicate_or_ordinary_names_cannot_hide_in_checked_declarations() {
    let (base, profile, _) = fixture();
    let (_, exports, _) = package(MANIFEST);
    let mut original = base.clone();
    exports
        .install_shipped_glyph_notations(&mut original, &profile)
        .unwrap();
    for changed in [
        format!("{SOURCE}\n{SOURCE}"),
        format!("{SOURCE}\ntype notation = U8\n"),
        format!("{SOURCE}\nplot notation {{\n}}\n"),
    ] {
        let document = crate::parse_syntax_document(&changed);
        assert!(
            document.diagnostics.is_empty(),
            "{:?}",
            document.diagnostics
        );
        let mut family = original
            .typed_literal_family("fixture/glyph/notation")
            .unwrap()
            .clone();
        family.origin.source_document_id = document.source_document_id();
        let mut startup = base.clone();
        startup
            .insert_typed_literal_family("fixture/glyph/notation", family, &profile)
            .unwrap();
        let before = startup.clone();
        let error = crate::check_syntax_document(&document, &startup).unwrap_err();
        assert!(error.message.contains("duplicate or ambiguous"));
        assert_eq!(startup, before);
    }
}
