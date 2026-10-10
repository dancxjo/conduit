use super::*;
use crate::glyph_notation_test_support::{fixture, MANIFEST, SOURCE};

fn installed() -> StartupCatalog {
    let (mut startup, profile, _) = fixture();
    let manifest = crate::parse_syntax_document(MANIFEST);
    let members = [crate::PackageMemberSource {
        path: "glyph/notation",
        source: SOURCE,
    }];
    let bundle =
        crate::CheckedPackageBundle::from_sources(MANIFEST, &manifest.packages[0], &members)
            .unwrap();
    crate::PackageExportCatalog::from_bundle(&bundle, MANIFEST, &manifest.packages[0], &members)
        .unwrap()
        .install_shipped_glyph_notations(&mut startup, &profile)
        .unwrap();
    startup
}

fn document(imports: &str, body: &str) -> SyntaxDocument {
    let source = format!("{imports}\n{body}");
    let document = crate::parse_syntax_document(&source);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    document
}

#[test]
fn with_binds_one_exact_shipped_family_and_preserves_distinct_alias_authorship() {
    let startup = installed();
    let before = startup.clone();
    let first = document("with fixture/glyph/notation as ph", "plot ordinary {\n}\n");
    let second = document("with fixture/glyph/notation as ipa", "plot ordinary {\n}\n");
    for (document, alias) in [(&first, "ph"), (&second, "ipa")] {
        let scope = resolve_glyph_notation_scope(document, &startup).unwrap();
        let binding = scope.binding(alias).unwrap();
        assert_eq!(binding.source_path, "fixture/glyph/notation");
        assert_eq!(
            binding.family,
            *startup.typed_literal_family(&binding.source_path).unwrap()
        );
        assert_eq!(binding.import_span, document.uses[0].span);
        let source = format!("{alias}/t͡ʃ/");
        let scanned = scope.scan_literal(alias, &source).unwrap();
        assert_eq!(scanned.raw_payload, "t͡ʃ");
        assert_eq!(
            scanned.branch.result_type,
            binding.family.branches[0].result_type
        );
        assert!(scope.binding("unbound").is_none());
    }
    assert_ne!(first.source_document_id(), second.source_document_id());
    assert_eq!(startup, before);
}

#[test]
fn no_prelude_or_other_import_category_silently_binds_a_literal_family() {
    let startup = installed();
    for imports in ["", "sans glyphs", "with fixture/literal as maker"] {
        let scope =
            resolve_glyph_notation_scope(&document(imports, "plot ordinary {\n}\n"), &startup)
                .unwrap();
        assert_eq!(scope.bindings().count(), 0);
        assert_eq!(
            scope.scan_literal("ph", "ph/x/").unwrap_err(),
            TypedLiteralScanRefusal::UnboundPrefix
        );
    }
    let explicit = document(
        "sans glyphs\nwith fixture/glyph/notation as ph",
        "plot ordinary {\n}\n",
    );
    assert!(resolve_glyph_notation_scope(&explicit, &startup)
        .unwrap()
        .binding("ph")
        .is_some());
}

#[test]
fn duplicate_and_foreign_import_categories_cannot_share_a_family_alias() {
    let startup = installed();
    let before = startup.clone();
    for imports in [
        "with fixture/glyph/notation as ph\nwith fixture/glyph/notation as ph",
        "with fixture/glyph/notation as ph\nwith fixture/literal as ph",
        "with fixture/glyph/notation as ph\nwith FixtureLiteral as ph",
    ] {
        let error =
            resolve_glyph_notation_scope(&document(imports, "plot ordinary {\n}\n"), &startup)
                .unwrap_err();
        assert!(error.message.contains("conflicts"));
        assert_eq!(startup, before);
    }
}

#[test]
fn ordinary_source_names_refuse_before_any_literal_interpretation() {
    let startup = installed();
    for body in [
        "type ph = U8\n",
        "plot ph {\n}\n",
        "plot ordinary (\n ph: U8 = 1\n) {\n}\n",
        "plot ordinary (\n ph: U8 >> output: U8\n) {\n}\n",
        "plot ordinary (\n ph: type\n) {\n}\n",
        "plot ordinary {\n ph = 1\n}\n",
        "plot ordinary {\n ph: fixture/literal\n}\n",
        "plot ordinary {\n plot ph {\n }\n}\n",
        "plot ordinary {\n plot child {\n  ph = 1\n }\n}\n",
        "body ordinary {\n ph = 1\n}\n",
    ] {
        let doc = document("with fixture/glyph/notation as ph", body);
        let error = resolve_glyph_notation_scope(&doc, &startup).unwrap_err();
        assert!(error.message.contains("conflicts"), "{body}: {error:?}");
        assert!(error
            .message
            .contains("qualified constructor: fixture/literal"));
    }
}

#[test]
fn unused_family_import_is_not_misclassified_as_an_executable_gear() {
    let startup = installed();
    let doc = document("with fixture/glyph/notation as ph", "plot ordinary {\n}\n");
    let error = crate::check_syntax_document(&doc, &startup).unwrap_err();
    assert_eq!(error.code, "CND-FRM-062");
    assert!(error
        .message
        .contains("unused with glyph notation alias 'ph'"));
}

#[test]
fn alias_limits_and_scope_capacity_refuse_without_changing_installed_truth() {
    let startup = installed();
    let before = startup.clone();
    for alias in ["x".repeat(257), "1ph".into()] {
        let imports = format!("with fixture/glyph/notation as {alias}");
        assert!(resolve_glyph_notation_scope(
            &document(&imports, "plot ordinary {\n}\n"),
            &startup
        )
        .is_err());
    }
    let imports = (0..64)
        .map(|i| format!("with fixture/glyph/notation as n{i}\n"))
        .collect::<String>();
    assert_eq!(
        resolve_glyph_notation_scope(&document(&imports, "plot ordinary {\n}\n"), &startup)
            .unwrap()
            .bindings()
            .count(),
        64
    );
    let excessive = format!("{imports}with fixture/glyph/notation as excess\n");
    let error =
        resolve_glyph_notation_scope(&document(&excessive, "plot ordinary {\n}\n"), &startup)
            .unwrap_err();
    assert!(error.message.contains("finite family"));
    assert_eq!(startup, before);
}

#[test]
fn inherited_family_scope_obeys_existing_plot_nesting_limits() {
    let startup = installed();
    let before = startup.clone();
    let body = (0..=crate::MAXIMUM_PLOT_NESTING_DEPTH)
        .map(|i| format!("plot nested{i} {{\n"))
        .collect::<String>()
        + &"}\n".repeat(crate::MAXIMUM_PLOT_NESTING_DEPTH + 1);
    let doc = document("with fixture/glyph/notation as ph", &body);
    let error = resolve_glyph_notation_scope(&doc, &startup).unwrap_err();
    assert!(error.message.contains("Plot nesting bound"));
    assert_eq!(startup, before);
}

#[test]
fn large_owner_contracts_reach_scope_byte_pressure_before_alias_count_limit() {
    let (mut startup, profile, mut family) = fixture();
    let fields = (0..64)
        .map(|i| format!(" field{i}{}: U8\n", "x".repeat(120)))
        .collect::<String>();
    let source = format!("type HugeFixture = {{\n{fields}}}\n");
    let parsed = crate::parse_syntax_document(&source);
    let checked = crate::check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();
    let ty = checked.native_types[0].value_type.clone();
    let mut constructor = profile
        .canonical_kind(&family.branches[0].constructor_kind)
        .unwrap()
        .clone();
    constructor.kind_contract_revision = conduit_core::KindIdentity::from("fixture/huge@1");
    constructor.outputs[0].value_kind = ty.profile().unwrap().value_kind().clone();
    let mut huge_profile = crate::ProfileCatalog::new();
    huge_profile.insert_kind(constructor.clone()).unwrap();
    family.branches[0].constructor_revision = constructor.kind_contract_revision;
    family.branches[0].result_type = ty;
    let branch = family.branches[0].clone();
    family.branches = [
        crate::TypedLiteralDelimiter::Square,
        crate::TypedLiteralDelimiter::Slash,
        crate::TypedLiteralDelimiter::Angle,
        crate::TypedLiteralDelimiter::DoubleSquare,
    ]
    .into_iter()
    .map(|delimiter| crate::TypedLiteralBranch {
        delimiter,
        ..branch.clone()
    })
    .collect();
    startup
        .insert_typed_literal_family("fixture/glyph/huge", family.clone(), &huge_profile)
        .unwrap();
    let identity_len = family.identity_bytes().unwrap().len();
    let count = MAXIMUM_SCOPE_BYTES / identity_len + 1;
    assert!(count < crate::MAXIMUM_TYPED_LITERAL_FAMILIES);
    let imports = (0..count)
        .map(|i| format!("with fixture/glyph/huge as n{i}\n"))
        .collect::<String>();
    let before = startup.clone();
    let error = resolve_glyph_notation_scope(&document(&imports, "plot ordinary {\n}\n"), &startup)
        .unwrap_err();
    assert!(error.message.contains("identity-byte bound"));
    assert_eq!(startup, before);
}

#[test]
fn two_documents_can_bind_distinct_shipped_families_under_the_same_prefix() {
    let mut startup = installed();
    let (_, profile, _) = fixture();
    let manifest_source = MANIFEST.replace("fixture/glyph", "fixture/alternate");
    let source = SOURCE.replace("fixture/notation@1", "fixture/notation@2");
    let manifest = crate::parse_syntax_document(&manifest_source);
    let members = [crate::PackageMemberSource {
        path: "glyph/notation",
        source: &source,
    }];
    let bundle = crate::CheckedPackageBundle::from_sources(
        &manifest_source,
        &manifest.packages[0],
        &members,
    )
    .unwrap();
    crate::PackageExportCatalog::from_bundle(
        &bundle,
        &manifest_source,
        &manifest.packages[0],
        &members,
    )
    .unwrap()
    .install_shipped_glyph_notations(&mut startup, &profile)
    .unwrap();
    let before = startup.clone();
    let first = resolve_glyph_notation_scope(
        &document("with fixture/glyph/notation as ph", "plot ordinary {\n}\n"),
        &startup,
    )
    .unwrap();
    let second = resolve_glyph_notation_scope(
        &document(
            "with fixture/alternate/notation as ph",
            "plot ordinary {\n}\n",
        ),
        &startup,
    )
    .unwrap();
    assert_ne!(
        first
            .binding("ph")
            .unwrap()
            .family
            .identity_bytes()
            .unwrap(),
        second
            .binding("ph")
            .unwrap()
            .family
            .identity_bytes()
            .unwrap()
    );
    assert_eq!(
        first.scan_literal("ph", "ph/x/").unwrap().payload,
        second.scan_literal("ph", "ph/x/").unwrap().payload
    );
    let conflicting = document(
        "with fixture/glyph/notation as ph\nwith fixture/alternate/notation as ph",
        "plot ordinary {\n}\n",
    );
    assert!(resolve_glyph_notation_scope(&conflicting, &startup).is_err());
    assert_eq!(startup, before);
}
