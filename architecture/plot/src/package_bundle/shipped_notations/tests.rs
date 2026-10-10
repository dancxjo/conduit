use super::super::notation_tests::{MANIFEST, SOURCE};
use super::*;
use crate::glyph_notation_test_support::fixture;

fn exports(source: &str) -> PackageExportCatalog {
    exports_for(MANIFEST, source)
}

fn exports_for(manifest_source: &str, source: &str) -> PackageExportCatalog {
    let manifest = crate::parse_syntax_document(manifest_source);
    let members = [crate::PackageMemberSource {
        path: "glyph/notation",
        source,
    }];
    let bundle =
        crate::CheckedPackageBundle::from_sources(manifest_source, &manifest.packages[0], &members)
            .unwrap();
    PackageExportCatalog::from_bundle(&bundle, manifest_source, &manifest.packages[0], &members)
        .unwrap()
}

#[test]
fn source_declared_family_installs_against_exact_ordinary_constructor_truth() {
    let (mut startup, profile, fixture) = fixture();
    let exports = exports(SOURCE);
    let paths = exports
        .install_shipped_glyph_notations(&mut startup, &profile)
        .unwrap();
    assert_eq!(paths, ["fixture/glyph/notation"]);
    let family = startup.typed_literal_family(&paths[0]).unwrap();
    assert_eq!(family.branches, fixture.branches);
    assert_eq!(family.revision, fixture.revision);
    assert_eq!(
        family.origin.package_content_digest,
        exports.package_content_digest()
    );
    assert_eq!(
        family.origin.source_document_id,
        crate::parse_syntax_document(SOURCE).source_document_id()
    );
    assert_eq!(family.origin.module_path, "glyph/notation");
    let before = startup.clone();
    assert!(exports
        .install_shipped_glyph_notations(&mut startup, &profile)
        .is_err());
    assert_eq!(startup, before);
}

#[test]
fn foreign_result_and_stale_or_missing_constructor_refuse_without_mutation() {
    let (mut startup, profile, _) = fixture();
    let foreign =
        crate::parse_syntax_document("type ForeignLiteral = {\n payload: Text <= 64B\n}\n");
    let checked = crate::check_syntax_document(&foreign, &crate::StartupCatalog::new()).unwrap();
    startup
        .insert_checked_native_type("ForeignLiteral", &checked.native_types[0])
        .unwrap();
    for changed in [
        SOURCE.replace("FixtureLiteral", "ForeignLiteral"),
        SOURCE.replace("fixture/literal@1", "fixture/literal@0"),
        SOURCE.replace("\"fixture/literal\"", "\"fixture/missing\""),
        SOURCE.replace("FixtureLiteral", "MissingLiteral"),
    ] {
        let before = startup.clone();
        let error = exports(&changed)
            .install_shipped_glyph_notations(&mut startup, &profile)
            .unwrap_err();
        assert_eq!(error.code, "CND-FRM-062");
        assert_eq!(startup, before);
        assert!(error.span.start < error.span.end);
    }
    let before = startup.clone();
    assert!(exports(SOURCE)
        .install_shipped_glyph_notations(&mut startup, &crate::ProfileCatalog::new())
        .is_err());
    assert_eq!(startup, before);
}

#[test]
fn metadata_is_exact_bounded_data_and_cannot_install_arbitrary_grammar() {
    let (mut startup, profile, _) = fixture();
    for changed in [
        SOURCE.replace("\"slash\"", "\"arbitrary-token-pair\""),
        SOURCE.replace("\"raw-unicode\"", "\"host-regex-callback\""),
        SOURCE.replace("maximum-payload-bytes: 64", "maximum-payload-bytes: 4097"),
        SOURCE.replace("maximum-payload-bytes: 64", "maximum-payload-bytes: 0"),
        SOURCE.replace("maximum-payload-bytes: 64", "maximum-payload-bytes: 65536"),
        SOURCE.replace("maximum-payload-bytes: 64", "maximum-payload-bytes: \"64\""),
        SOURCE.replace(
            "result: \"FixtureLiteral\"",
            "result: \"FixtureLiteral\", surprise: 1",
        ),
        SOURCE.replace(
            "result: \"FixtureLiteral\"",
            "result: \"FixtureLiteral\", result: \"FixtureLiteral\"",
        ),
        SOURCE.replace(
            "revision: \"fixture/notation@1\"",
            "revision: sneaky(\"fixture/notation@1\")",
        ),
        SOURCE.replace("fixture/parser@1", &"x".repeat(257)),
    ] {
        let before = startup.clone();
        assert!(exports(&changed)
            .install_shipped_glyph_notations(&mut startup, &profile)
            .is_err());
        assert_eq!(startup, before);
    }
}

#[test]
fn parser_revision_and_exact_authored_source_pin_installed_family_identity() {
    let (startup, profile, _) = fixture();
    let mut original = startup.clone();
    exports(SOURCE)
        .install_shipped_glyph_notations(&mut original, &profile)
        .unwrap();
    for changed in [
        SOURCE.replace("fixture/parser@1", "fixture/parser@2"),
        format!("# different authored bytes\n{SOURCE}"),
    ] {
        let mut modified = startup.clone();
        exports(&changed)
            .install_shipped_glyph_notations(&mut modified, &profile)
            .unwrap();
        assert_ne!(
            original
                .typed_literal_family("fixture/glyph/notation")
                .unwrap()
                .identity_bytes()
                .unwrap(),
            modified
                .typed_literal_family("fixture/glyph/notation")
                .unwrap()
                .identity_bytes()
                .unwrap()
        );
    }
}

#[test]
fn failed_later_shipment_rolls_back_the_whole_family_batch() {
    let (mut startup, profile, _) = fixture();
    let manifest =
        "pack fixture/glyph (\n version = 1.0.0\n) {\n ship another\n ship notation\n}\n";
    let valid_first = SOURCE.replace("glyph notation notation", "glyph notation another");
    let invalid_later = SOURCE.replace("fixture/literal@1", "fixture/literal@0");
    let source = format!("{valid_first}\n{invalid_later}");
    let before = startup.clone();
    assert!(exports_for(manifest, &source)
        .install_shipped_glyph_notations(&mut startup, &profile)
        .is_err());
    assert_eq!(startup, before);
    assert!(startup
        .typed_literal_family("fixture/glyph/another")
        .is_none());
}
