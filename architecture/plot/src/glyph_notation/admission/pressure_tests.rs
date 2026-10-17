//! Combined admission uses real prepared receipts and immutable Source chains.
use crate::*;
use conduit_core::{
    ConfigurationEntry, Kind, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape as T, StructuredInfoValue,
};

struct Owner {
    kind: Kind,
    result: StructuredInfoType,
    context: StructuredInfoType,
}
fn value(ty: &StructuredInfoType) -> StructuredInfoValue {
    if let T::Nominal { representation, .. } = ty.shape() {
        return StructuredInfoValue::nominal(ty.clone(), value(representation)).unwrap();
    }
    let T::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "payload",
            StructuredInfoValue::leaf(fields[0].value_type().clone(), b"x".to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
}
impl StaticValueConstructor for Owner {
    type Refusal = ();
    fn contract(&self) -> Kind {
        self.kind.clone()
    }
    fn result_type(&self) -> StructuredInfoType {
        self.result.clone()
    }
    fn prepare_configuration(&self, _: &[ConfigurationEntry]) -> Result<StructuredInfoValue, ()> {
        Ok(value(&self.result))
    }
}
impl LiteralValueConstructor for Owner {
    fn context_types(&self) -> Vec<(String, StructuredInfoType)> {
        vec![("basis".into(), self.context.clone())]
    }
    fn parser_contract(&self) -> &str {
        "fixture/parser@1"
    }
    fn lexical_policy(&self) -> TypedLiteralLexicalPolicy {
        TypedLiteralLexicalPolicy::RawUnicode
    }
    fn literal_configuration(
        &self,
        _: &TypedGlyphLiteralSyntax,
        context: &[ConfigurationEntry],
    ) -> Result<Vec<ConfigurationEntry>, ()> {
        if context.len() != 1 || context[0].key != "basis" {
            return Err(());
        }
        Ok(vec![])
    }
}
fn chain(prefix: &str) -> String {
    let mut source = format!("{prefix} = {{payload: \"{}\"}}\n", "x".repeat(32768));
    for index in 0..16 {
        let previous = if index == 0 {
            prefix.into()
        } else {
            format!("{prefix}-{}", index - 1)
        };
        source.push_str(&format!("{prefix}-{index} = {previous}\n"));
    }
    source
}
fn prepared(shared: bool) -> (SyntaxDocument, StartupCatalog, Vec<PreparedGlyphLiteral>) {
    let (mut startup, profile, family) = crate::glyph_notation_test_support::fixture();
    startup
        .insert_typed_literal_family("fixture/notation", family.clone(), &profile)
        .unwrap();
    let packet_source = "type Packet = {\n payload: Text <= 32768B\n}\n";
    let packet = check_syntax_document(
        &parse_syntax_document(packet_source),
        &StartupCatalog::new(),
    )
    .unwrap();
    startup
        .insert_checked_native_type("Packet", &packet.native_types[0])
        .unwrap();
    let owner = Owner {
        kind: profile
            .canonical_kind(&family.branches[0].constructor_kind)
            .unwrap()
            .clone(),
        result: family.branches[0].result_type.clone(),
        context: packet.native_types[0].value_type.clone(),
    };
    let source = format!(
        "with fixture/notation as r\nplot example {{\n{}{}first = r/a/\nsecond = r/b/\n}}\n",
        chain("left"),
        if shared {
            String::new()
        } else {
            chain("right")
        }
    );
    assert!(source.len() < 100_000);
    let document = parse_syntax_document_with_glyph_notations(&source, &startup);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let scope = resolve_glyph_notation_scope(&document, &startup).unwrap();
    let mut receipts = Vec::new();
    for (name, selected) in [
        ("first", "left-15"),
        ("second", if shared { "left-15" } else { "right-15" }),
    ] {
        let local = document.plots[0]
            .back
            .iter()
            .find_map(|statement| match statement {
                BackStatement::LocalValue(local) if local.name.text == name => Some(local),
                _ => None,
            })
            .unwrap();
        let ExpressionSyntax::TypedGlyphLiteral(literal) = &local.value.syntax else {
            panic!()
        };
        receipts.push(
            scope
                .prepare_literal_from_source_locals(
                    &document,
                    "example",
                    literal,
                    &[("basis", selected)],
                    &owner,
                    &startup,
                    &profile,
                )
                .unwrap(),
        );
    }
    for receipt in &receipts {
        let bytes: usize = receipt
            .source_context
            .iter()
            .map(|(expression, value)| {
                expression.text.len()
                    + value
                        .try_concrete()
                        .unwrap()
                        .canonical_bytes()
                        .unwrap()
                        .len()
            })
            .sum();
        assert!(bytes > 1024 * 1024 / 2 && bytes < 1024 * 1024);
    }
    (document, startup, receipts)
}
#[test]
fn shared_large_context_is_deduplicated_before_combined_byte_admission() {
    let (document, startup, receipts) = prepared(true);
    let admitted = admit_glyph_values(&document, &startup, &receipts).unwrap();
    assert_eq!(admitted.source_values.len(), 17);
    assert_eq!(admitted.values.len(), 2);
    check_syntax_document_with_prepared_glyph_literals(&document, &startup, &receipts).unwrap();
}
#[test]
fn individually_valid_disjoint_contexts_refuse_when_combined_over_budget() {
    let (document, startup, receipts) = prepared(false);
    let error = admit_glyph_values(&document, &startup, &receipts).unwrap_err();
    assert!(
        error.message.contains("combined Source context exceeds"),
        "{error:?}"
    );
}
