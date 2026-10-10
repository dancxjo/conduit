use crate::{PlotEditor, PlotEditorError};

fn pattern_source(spelling: &str) -> String {
    format!("with text/pattern/notation as r\nplot patterns {{\n value = {spelling}\n}}\n")
}

#[test]
fn editor_glyph_revision_and_expansion_preserve_checked_source_custody() {
    let source = pattern_source("r⟦a\n#[}]=>>\nt͡ʃ⟧");
    let mut editor = PlotEditor::from_source("glyphs.conduit".into(), source.clone()).unwrap();
    assert!(editor.view().checked.diagnostics.is_empty());
    let first_identity = editor.view().checked.source_document_id.clone().unwrap();
    assert_eq!(editor.view().source, source);
    editor.expand_plot("patterns").unwrap();
    editor.expand_plot_for_authoring("patterns").unwrap();
    let stale = editor.check_current().unwrap();
    let replacement = pattern_source("r/a[/]b/");
    editor.replace_source(replacement.clone()).unwrap();
    assert!(matches!(
        editor.publish_checked(stale),
        Err(PlotEditorError::StaleRevision { .. })
    ));
    editor.recheck().unwrap();
    assert!(editor.view().checked.diagnostics.is_empty());
    assert_ne!(
        editor.view().checked.source_document_id.unwrap(),
        first_identity
    );
    assert_eq!(editor.view().source, replacement);
    editor.expand_plot("patterns").unwrap();
}

#[test]
fn editor_glyph_refusals_are_checked_without_rewriting_source() {
    for source in [
        pattern_source("r/a\\/b/"),
        pattern_source("r/unterminated"),
        "with speech/ipa/notation as ph\nplot speech {\n value = ph/t͡ʃ/\n}\n".into(),
        "with text/pattern/notation as r\nplot collision {\n r = 1\n value = r/a/\n}\n".into(),
    ] {
        let editor = PlotEditor::from_source("glyphs.conduit".into(), source.clone()).unwrap();
        assert!(!editor.view().checked.diagnostics.is_empty(), "{source}");
        assert_eq!(editor.view().source, source);
        assert!(editor.view().checked.source_document_id.is_none());
    }
}

#[test]
fn editor_admits_both_ipa_branches_with_explicit_checked_basis() {
    use conduit_plot::{Argument, BackStatement, ExpressionSyntax};
    let quoted =
        include_str!("../../../../../semantics/speech/examples/ipa/quoted-phonemic.conduit");
    let document = conduit_plot::parse_syntax_document(quoted);
    let BackStatement::NamedGear(gear) = &document.plots[0].back[0] else {
        panic!()
    };
    let fields = gear
        .invocation
        .arguments
        .iter()
        .map(|argument| {
            let Argument::Named { name, value, .. } = argument else {
                panic!()
            };
            if name.text == "request" {
                let ExpressionSyntax::Record { fields, .. } = &value.syntax else {
                    panic!()
                };
                let span = fields
                    .iter()
                    .find(|field| field.name.text == "provenance")
                    .unwrap()
                    .value
                    .span();
                (
                    "provenance".to_string(),
                    quoted[span.start..span.end].to_string(),
                )
            } else {
                (name.text.clone(), value.text.clone())
            }
        })
        .collect::<Vec<_>>();
    let context = fields
        .iter()
        .map(|(key, _)| format!("{key}: chosen-{key}"))
        .collect::<Vec<_>>()
        .join(", ");
    let declarations = fields
        .iter()
        .map(|(key, value)| format!(" chosen-{key} = {value}\n"))
        .collect::<String>();
    let source = format!("with speech/ipa/notation as ph using {{{context}}}\nplot speech {{\n{declarations} phonetic = ph[ˈt͡ʃaː]\n phonemic = ph/ˈt͡ʃaː/\n}}\n");
    let editor = PlotEditor::from_source("ipa.conduit".into(), source.clone()).unwrap();
    assert!(
        editor.view().checked.diagnostics.is_empty(),
        "{:?}",
        editor.view().checked.diagnostics
    );
    assert_eq!(editor.view().source, source);
    editor.expand_plot("speech").unwrap();
    editor.expand_plot_for_authoring("speech").unwrap();
}
