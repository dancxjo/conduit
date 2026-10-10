use super::*;

pub(super) const MANIFEST: &str =
    "pack fixture/glyph (\n version = 1.0.0\n) {\n ship notation\n}\n";
pub(super) const SOURCE: &str = "# Original π / IPA owner metadata\nglyph notation notation = {\n revision: \"fixture/notation@1\",\n branches: [{ delimiter: \"slash\", lexical-policy: \"raw-unicode\", parser: \"fixture/parser@1\", constructor: \"fixture/literal\", constructor-revision: \"fixture/literal@1\", result: \"FixtureLiteral\", maximum-payload-bytes: 64 }]\n}\n";

fn bundle(source: &str) -> Result<CheckedPackageBundle, PackageBundleError> {
    let document = crate::parse_syntax_document(MANIFEST);
    CheckedPackageBundle::from_sources(
        MANIFEST,
        &document.packages[0],
        &[PackageMemberSource {
            path: "glyph/notation",
            source,
        }],
    )
}

#[test]
fn shipped_notation_retains_actual_member_and_package_source_identity() {
    let document = crate::parse_syntax_document(SOURCE);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), SOURCE);
    assert_eq!(document.glyph_notations.len(), 1);
    let declaration = &document.glyph_notations[0];
    assert_eq!(
        &SOURCE[declaration.name.span.start..declaration.name.span.end],
        "notation"
    );
    assert_eq!(
        &SOURCE[declaration.metadata.span.start..declaration.metadata.span.end],
        declaration.metadata.text
    );
    let manifest = crate::parse_syntax_document(MANIFEST);
    let bundle = bundle(SOURCE).unwrap();
    assert_eq!(
        bundle.resolve_glyph_notation_export("fixture/glyph/notation"),
        Some("notation")
    );
    assert!(bundle.resolve_export("fixture/glyph/notation").is_none());
    assert!(bundle
        .resolve_type_export("fixture/glyph/notation")
        .is_none());
    let exports = PackageExportCatalog::from_bundle(
        &bundle,
        MANIFEST,
        &manifest.packages[0],
        &[PackageMemberSource {
            path: "glyph/notation",
            source: SOURCE,
        }],
    )
    .unwrap();
    let (syntax, origin) = exports
        .resolve_glyph_notation("fixture/glyph/notation")
        .unwrap();
    assert_eq!(syntax, declaration);
    assert_eq!(origin.module_path, "glyph/notation");
    assert_eq!(origin.source_document_id, document.source_document_id());
    assert_eq!(origin.package_content_digest, bundle.package.content_digest);
    assert!(exports
        .resolve_glyph_notation("fixture/other/notation")
        .is_none());
    assert!(exports
        .resolve_glyph_notation("fixture/glyph/private")
        .is_none());
    assert!(exports.resolve("fixture/glyph/notation").is_none());
    assert!(exports.resolve_type("fixture/glyph/notation").is_none());
}

#[test]
fn tampered_source_or_serialized_member_cannot_supply_notation_provenance() {
    let accepted = bundle(SOURCE).unwrap();
    let changed = SOURCE.replace("fixture/parser@1", "fixture/parser@2");
    assert_ne!(
        bundle(&changed).unwrap().package.content_digest,
        accepted.package.content_digest
    );
    let manifest = crate::parse_syntax_document(MANIFEST);
    assert_eq!(
        accepted.validate_against(
            MANIFEST,
            &manifest.packages[0],
            &[PackageMemberSource {
                path: "glyph/notation",
                source: &changed,
            }]
        ),
        Err(PackageBundleError::SourceMismatch)
    );
    let mut forged = accepted.clone();
    forged.members[0].glyph_notations[0] = "forged".into();
    assert_eq!(
        forged.validate_against(
            MANIFEST,
            &manifest.packages[0],
            &[PackageMemberSource {
                path: "glyph/notation",
                source: SOURCE,
            }]
        ),
        Err(PackageBundleError::SourceMismatch)
    );
}

#[test]
fn ambiguous_and_duplicate_notation_exports_refuse() {
    let ambiguous = format!("{SOURCE}\ntype notation = U8\n");
    assert_eq!(
        bundle(&ambiguous).unwrap_err(),
        PackageBundleError::AmbiguousExport("notation".into())
    );
    let duplicate = format!("{SOURCE}\n{SOURCE}");
    assert_eq!(
        bundle(&duplicate).unwrap_err(),
        PackageBundleError::DuplicateGlyphNotation("notation".into())
    );
    let missing = SOURCE.replace("glyph notation notation", "glyph notation private");
    assert_eq!(
        bundle(&missing).unwrap_err(),
        PackageBundleError::MissingExport("notation".into())
    );
}

#[test]
fn quoted_braces_unicode_and_escape_spelling_are_lossless() {
    let source =
        "glyph notation example = { revision: \"{é} \\\"quoted\\\"\", branches: [\"⟦x⟧\"] } # source comment\n";
    let parsed = crate::parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.round_trip(), source);
    assert_eq!(
        parsed.glyph_notations[0].metadata.text,
        source
            .split_once("= ")
            .unwrap()
            .1
            .split(" # source comment")
            .next()
            .unwrap()
    );
}

#[test]
fn malformed_and_unbounded_declarations_refuse_before_installation() {
    for source in [
        "glyph notation x = { revision: \"unterminated }",
        "glyph notation x = [1]",
        "glyph notation x = { revision: 1 } injected",
        "glyph notation x = { revision: 1, }",
    ] {
        let parsed = crate::parse_syntax_document(source);
        assert!(!parsed.diagnostics.is_empty(), "{source}");
    }
    let huge = format!(
        "glyph notation x = {{ revision: \"{}\" }}",
        "ə".repeat(32768)
    );
    assert!(!crate::parse_syntax_document(&huge).diagnostics.is_empty());
    let too_many = (0..65)
        .map(|i| format!("glyph notation n{i} = {{ revision: 1 }}\n"))
        .collect::<String>();
    assert!(!crate::parse_syntax_document(&too_many)
        .diagnostics
        .is_empty());
    let nested = format!(
        "glyph notation x = {{ revision: {}1{} }}",
        "[".repeat(20),
        "]".repeat(20)
    );
    assert!(!crate::parse_syntax_document(&nested).diagnostics.is_empty());
}

#[test]
fn unresolved_notation_metadata_cannot_be_ignored_by_executable_checking() {
    let document = crate::parse_syntax_document(SOURCE);
    let failure =
        crate::check_syntax_document(&document, &crate::StartupCatalog::new()).unwrap_err();
    assert_eq!(failure.code, "CND-FRM-062");
    let manifest = crate::parse_syntax_document(MANIFEST);
    let failure = crate::check_package_bundle(
        &bundle(SOURCE).unwrap(),
        MANIFEST,
        &manifest.packages[0],
        &[PackageMemberSource {
            path: "glyph/notation",
            source: SOURCE,
        }],
        &crate::StartupCatalog::new(),
    )
    .unwrap_err();
    assert!(
        matches!(failure, crate::PackageCheckError::Syntax { diagnostic, .. } if diagnostic.code == "CND-FRM-062")
    );
}

#[test]
fn notation_declarations_are_member_source_and_not_manifest_contents() {
    let mixed = format!("{MANIFEST}\n{SOURCE}");
    assert!(!crate::parse_syntax_document(&mixed).diagnostics.is_empty());
    let ordinary =
        crate::parse_syntax_document("with fixture/glyph/notation as ph\nplot ordinary {\n}\n");
    assert!(
        ordinary.diagnostics.is_empty(),
        "{:?}",
        ordinary.diagnostics
    );
    assert_eq!(ordinary.uses[0].path, "fixture/glyph/notation");
    assert_eq!(ordinary.uses[0].alias.text, "ph");
}
