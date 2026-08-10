use super::*;

const GREET: &str = include_str!("../../../examples/greet.conduit");
const HELLO: &str = include_str!("../../../examples/hello.conduit");

#[test]
fn checked_examples_share_exact_source_and_graph_identities() {
    for (name, source) in [("hello", HELLO), ("greet", GREET)] {
        let mut editor =
            FormEditor::from_source(format!("{name}.conduit").into(), source.into()).unwrap();
        let view = editor.view();
        assert!(view.checked.diagnostics.is_empty());
        assert!(view.checked.source_document_id.is_some());
        let item = view.checked.forms[0].items[0].clone();
        assert!(!source[item.source_span.start..item.source_span.end].is_empty());
        assert!(editor.select_graph_item(&item.identity));
        assert_eq!(editor.view().selection.unwrap().span, item.source_span);
        assert!(editor.select_source_span(item.source_span));
        assert_eq!(editor.view().selection.unwrap().identity, item.identity);
    }
}

#[test]
fn greet_face_is_collapsed_then_its_checked_back_can_be_opened() {
    let mut editor = FormEditor::from_source("greet.conduit".into(), GREET.into()).unwrap();
    assert_eq!(editor.view().open_form, "default-welcome");
    editor.open_back("greet").unwrap();
    let view = editor.view();
    assert_eq!(view.open_form, "greet");
    let back = view
        .checked
        .forms
        .iter()
        .find(|form| form.name == "greet")
        .unwrap();
    assert!(back
        .items
        .iter()
        .any(|item| item.label == "join: text/join"));
    assert!(back
        .items
        .iter()
        .any(|item| item.kind == GraphItemKind::Cord));
    let welcome = view
        .checked
        .forms
        .iter()
        .find(|form| form.name == "welcome")
        .unwrap();
    assert!(welcome
        .items
        .iter()
        .any(|item| item.label == "hello: greet"));
}

#[test]
fn malformed_edit_keeps_exact_diagnostic_span() {
    let mut editor = FormEditor::from_source("hello.conduit".into(), HELLO.into()).unwrap();
    let malformed = "form hello {\n    upper: missing/operation\n}\n".to_string();
    editor.replace_source(malformed.clone()).unwrap();
    editor.recheck().unwrap();
    let diagnostic = &editor.view().checked.diagnostics[0];
    assert_eq!(
        &malformed[diagnostic.span.start..diagnostic.span.end],
        "missing/operation"
    );
    assert_eq!(diagnostic.code, "CND-FRM-028");
}

#[test]
fn stale_check_result_cannot_replace_newer_revision() {
    let mut editor = FormEditor::from_source("hello.conduit".into(), HELLO.into()).unwrap();
    let stale = editor.check_current().unwrap();
    editor.replace_source(GREET.into()).unwrap();
    assert_eq!(
        editor.publish_checked(stale),
        Err(FormEditorError::StaleRevision {
            current: 1,
            offered: 0
        })
    );
    editor.recheck().unwrap();
    assert_eq!(editor.view().checked.revision, 1);
}

#[test]
fn saved_revision_only_advances_for_the_current_source() {
    let mut editor = FormEditor::from_source("hello.conduit".into(), HELLO.into()).unwrap();
    editor.replace_source(GREET.into()).unwrap();
    assert_eq!(
        editor.mark_saved(0),
        Err(FormEditorError::StaleRevision {
            current: 1,
            offered: 0
        })
    );
    assert_eq!(editor.view().saved_revision, 0);
    editor.mark_saved(1).unwrap();
    assert_eq!(editor.view().saved_revision, 1);
}

#[test]
fn palette_placement_edits_canonical_source_and_creates_distinct_gears() {
    let source = "form making {\n}\n";
    let mut editor = FormEditor::from_source("making.conduit".into(), source.into()).unwrap();
    let original_id = editor.view().checked.source_document_id.unwrap();
    assert_eq!(
        editor.place_palette_kind(0, &conduit_core::kind_id("text/upper")),
        Ok("upper".into())
    );
    assert_eq!(
        editor.place_palette_kind(1, &conduit_core::kind_id("text/upper")),
        Ok("upper-2".into())
    );
    let view = editor.view();
    assert!(view.source.contains("upper: text/upper"));
    assert!(view.source.contains("upper-2: text/upper"));
    assert_ne!(view.checked.source_document_id.unwrap(), original_id);
    assert_eq!(
        view.checked.forms[0]
            .items
            .iter()
            .filter(|item| item.kind == GraphItemKind::Gear)
            .count(),
        2
    );
}

#[test]
fn stale_or_unknown_palette_placement_cannot_mutate_source() {
    let mut editor =
        FormEditor::from_source("making.conduit".into(), "form making {\n}\n".into()).unwrap();
    let source = editor.view().source;
    assert_eq!(
        editor.place_palette_kind(1, &conduit_core::kind_id("text/upper")),
        Err(FormEditorError::StaleRevision {
            current: 0,
            offered: 1
        })
    );
    assert_eq!(
        editor.place_palette_kind(0, &conduit_core::kind_id("invented/kind")),
        Err(FormEditorError::UnknownPaletteKind("invented/kind".into()))
    );
    assert_eq!(editor.view().source, source);
}
