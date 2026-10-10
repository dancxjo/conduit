#![cfg(feature = "semantic-bindings")]
use conduit_core::{ConfigurationEntry, ConfigurationValue, StructuredConfigurationValue};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    parse_syntax_document_with_glyph_notations, resolve_glyph_notation_scope,
    rust_binding::NativeRustBinding, BackStatement, ExpressionSyntax, LiteralPreparationRefusal,
    ProfileCatalog, StartupCatalog, SyntaxDocument, TypedGlyphLiteralSyntax,
};
use conduit_speech::{ipa_constructors::*, semantic::*};

const PHONETIC: &str = include_str!("../examples/ipa/quoted-transcriptions.conduit");
const PHONEMIC: &str = include_str!("../examples/ipa/quoted-phonemic.conduit");
fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_speech::authoring::install(&mut startup).unwrap();
    install(&mut startup, &mut profile).unwrap();
    install_notation(&mut startup, &profile).unwrap();
    (startup, profile)
}
fn ordinary(
    source: &str,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Vec<ConfigurationEntry> {
    let document = parse_syntax_document(source);
    let checked = check_syntax_document(&document, startup).unwrap();
    expand_canonical_plot_for_authoring(&checked, &document.plots[0].name.text, profile)
        .unwrap()
        .expanded
        .gears[0]
        .configuration
        .clone()
}
fn context(configuration: &[ConfigurationEntry]) -> Vec<ConfigurationEntry> {
    let request = configuration
        .iter()
        .find(|entry| entry.key == "request")
        .unwrap();
    let ConfigurationValue::Structured(request) = &request.value else {
        panic!()
    };
    let request = SpeechIpaUniversalRequest::decode(request.canonical_value()).unwrap();
    let ty = SpeechEvidenceProvenance::semantic_type().unwrap();
    let mut context = vec![ConfigurationEntry {
        key: "provenance".into(),
        value: ConfigurationValue::Structured(
            StructuredConfigurationValue::new(
                ty.profile().unwrap().value_kind().clone(),
                request.provenance().clone().encode().unwrap(),
            )
            .unwrap(),
        ),
    }];
    context.extend(
        configuration
            .iter()
            .filter(|entry| entry.key != "request")
            .cloned(),
    );
    context
}
fn literal(document: &SyntaxDocument) -> &TypedGlyphLiteralSyntax {
    let BackStatement::LocalValue(value) = &document.plots[0].back[0] else {
        panic!()
    };
    let ExpressionSyntax::TypedGlyphLiteral(literal) = &value.value.syntax else {
        panic!()
    };
    literal
}
fn source(startup: &StartupCatalog, spelling: &str) -> SyntaxDocument {
    let source =
        format!("with {NOTATION_EXPORT_PATH} as ph\nplot typed {{\n value = {spelling}\n}}\n");
    let document = parse_syntax_document_with_glyph_notations(&source, startup);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    assert_eq!(document.round_trip(), source);
    document
}

#[test]
fn shipped_speech_family_elaborates_both_delimiters_through_ordinary_admission() {
    let (startup, profile) = catalogs();
    let family = startup.typed_literal_family(NOTATION_EXPORT_PATH).unwrap();
    assert_eq!(family.origin.module_path, "notation");
    assert_eq!(family.branches.len(), 2);
    for (constructor, quoted, glyph) in [
        (IpaConstructor::Phonetic, PHONETIC, "ph[ˈt͡ʃãː.n̩]"),
        (IpaConstructor::Phonemic, PHONEMIC, "ph/ˈt͡ʃaː/"),
    ] {
        let configuration = ordinary(quoted, &startup, &profile);
        let document = source(&startup, glyph);
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        let prepared = scope
            .prepare_literal(
                &document,
                literal(&document),
                &context(&configuration),
                &constructor,
                &startup,
                &profile,
            )
            .unwrap();
        let explicit = prepare_configuration(constructor, &configuration).unwrap();
        assert_eq!(
            prepared.ordinary().value().canonical_bytes().unwrap(),
            explicit.bytes()
        );
        assert_eq!(prepared.ordinary().configuration(), configuration);
        assert_eq!(
            prepared.source_document_id(),
            &document.source_document_id()
        );
        assert_eq!(prepared.authored().authored.text, glyph);
        assert_ne!(
            document.source_document_id(),
            parse_syntax_document(quoted).source_document_id()
        );
    }
}

#[test]
fn one_segment_literal_never_switches_to_single_unit_type() {
    let (startup, profile) = catalogs();
    for (constructor, quoted, glyph) in [
        (
            IpaConstructor::Phonetic,
            PHONETIC.replace("ˈt͡ʃãː.n̩", "t͡ʃ"),
            "ph[t͡ʃ]",
        ),
        (
            IpaConstructor::Phonemic,
            PHONEMIC.replace("ˈt͡ʃaː", "t͡ʃ"),
            "ph/t͡ʃ/",
        ),
    ] {
        let configuration = ordinary(&quoted, &startup, &profile);
        let document = source(&startup, glyph);
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        let prepared = scope
            .prepare_literal(
                &document,
                literal(&document),
                &context(&configuration),
                &constructor,
                &startup,
                &profile,
            )
            .unwrap();
        assert_eq!(
            prepared.ordinary().value().value_type(),
            &constructor.output_type()
        );
        assert!(scope
            .prepare_literal(
                &document,
                literal(&document),
                &context(&configuration),
                &IpaConstructor::Phone,
                &startup,
                &profile
            )
            .is_err());
    }
}

#[test]
fn no_ambient_provenance_or_inventory_and_no_scanner_escape_privilege() {
    let (startup, profile) = catalogs();
    for (constructor, quoted, glyph) in [
        (IpaConstructor::Phonemic, PHONEMIC, "ph/z/"),
        (IpaConstructor::Phonetic, PHONETIC, r"ph[t͡ʃ\]]"),
    ] {
        let configuration = ordinary(quoted, &startup, &profile);
        let document = source(&startup, glyph);
        let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
        let context = context(&configuration);
        assert!(matches!(
            scope.prepare_literal(
                &document,
                literal(&document),
                &context,
                &constructor,
                &startup,
                &profile
            ),
            Err(LiteralPreparationRefusal::Constructor(_))
        ));
        assert!(matches!(
            scope.prepare_literal(
                &document,
                literal(&document),
                &[],
                &constructor,
                &startup,
                &profile
            ),
            Err(LiteralPreparationRefusal::Owner(_))
        ));
    }
    let configuration = ordinary(PHONEMIC, &startup, &profile);
    let document = source(&startup, "ph/t͡ʃ/");
    let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
    let mut context = context(&configuration);
    context.retain(|entry| entry.key != "basis");
    assert!(scope
        .prepare_literal(
            &document,
            literal(&document),
            &context,
            &IpaConstructor::Phonemic,
            &startup,
            &profile
        )
        .is_err());
}

#[test]
fn source_span_family_and_payload_tampering_refuse_before_receipt() {
    let (startup, profile) = catalogs();
    let configuration = ordinary(PHONETIC, &startup, &profile);
    let document = source(&startup, "ph[ˈt͡ʃãː.n̩]");
    let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
    let original = literal(&document);
    let mut identity = original.clone();
    identity.family_identity[0] ^= 1;
    let mut payload = original.clone();
    payload.raw_payload.text = "z".into();
    let mut span = original.clone();
    span.raw_payload.span.column += 1;
    let mut flags = original.clone();
    flags.case_insensitive = true;
    for altered in [identity, payload, span, flags] {
        assert!(matches!(
            scope.prepare_literal(
                &document,
                &altered,
                &context(&configuration),
                &IpaConstructor::Phonetic,
                &startup,
                &profile
            ),
            Err(LiteralPreparationRefusal::Identity)
        ));
    }
    let mut authored = original.clone();
    authored.authored.text = "ph[z]".into();
    assert!(matches!(
        scope.prepare_literal(
            &document,
            &authored,
            &context(&configuration),
            &IpaConstructor::Phonetic,
            &startup,
            &profile
        ),
        Err(LiteralPreparationRefusal::Source)
    ));
}

#[test]
fn oversized_explicit_context_refuses_before_configuration_cloning() {
    let (startup, profile) = catalogs();
    let configuration = ordinary(PHONEMIC, &startup, &profile);
    let document = source(&startup, "ph/t͡ʃ/");
    let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
    let mut context = context(&configuration);
    context
        .iter_mut()
        .find(|entry| entry.key == "basis")
        .unwrap()
        .value = ConfigurationValue::Text("x".repeat(1024 * 1024));
    assert!(matches!(
        scope.prepare_literal(
            &document,
            literal(&document),
            &context,
            &IpaConstructor::Phonemic,
            &startup,
            &profile
        ),
        Err(LiteralPreparationRefusal::ContextLimit)
    ));
}
