use super::*;
use crate::PatchbayGraph;

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
    let default_graph = editor
        .patchbay_graph_for_authoring("default-welcome")
        .unwrap();
    assert_eq!(default_graph.compositions.len(), 1);
    let hello = &default_graph.compositions[0];
    assert_eq!(hello.identity, "composition/hello");
    assert_eq!(hello.gear_name, "hello");
    assert_eq!(hello.back_name, "greet");
    assert_eq!(hello.inputs[0].identity, "composition/hello/input/name");
    assert_eq!(hello.outputs[0].identity, "composition/hello/output/text");
    assert_eq!(
        hello.input_bindings[0].internal_port,
        "port/default-welcome/hello/join/input/text"
    );
    assert_eq!(
        hello.output_bindings[0].internal_port,
        "port/default-welcome/hello/join/output/text"
    );
}

#[test]
fn open_back_authoring_preserves_exact_face_ports_without_claiming_a_runnable_root() {
    let editor = FormEditor::from_source("greet.conduit".into(), GREET.into()).unwrap();
    let authoring = editor.expand_form_for_authoring("greet").unwrap();
    let graph = PatchbayGraph::from_authoring(&authoring).unwrap();

    assert_eq!(graph.face_inputs.len(), 1);
    assert_eq!(graph.face_inputs[0].identity, "face/input/name");
    assert_eq!(
        graph.face_inputs[0].descriptor.value_kind.as_str(),
        "value/text@1"
    );
    assert_eq!(graph.face_outputs.len(), 1);
    assert_eq!(graph.face_outputs[0].identity, "face/output/text");
    assert!(graph
        .cords
        .iter()
        .any(|cord| cord.source_port == "face/input/name"));
    assert!(graph
        .cords
        .iter()
        .any(|cord| cord.sink_port == "face/output/text"));
    assert!(editor
        .expand_form("greet")
        .unwrap_err()
        .to_string()
        .contains("unbound runtime face ports"));
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

#[test]
fn duplicate_connect_and_remove_are_atomic_canonical_form_edits() {
    let mut editor = FormEditor::from_source(
        PathBuf::from("compose.conduit"),
        "form compose {\n    literal: text/literal(\"hello\")\n}\n".into(),
    )
    .unwrap();
    editor
        .place_palette_kind(0, &conduit_core::kind_id("text/upper"))
        .unwrap();
    editor
        .place_palette_kind(1, &conduit_core::kind_id("presentation/text"))
        .unwrap();
    assert_eq!(editor.duplicate_gear(2, "literal").unwrap(), "literal-2");

    let graph = PatchbayGraph::from_expanded(&editor.expand_form("compose").unwrap()).unwrap();
    let output = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "compose/literal")
        .unwrap()
        .outputs[0]
        .identity
        .clone();
    let input = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "compose/upper")
        .unwrap()
        .inputs[0]
        .identity
        .clone();
    editor
        .connect_ports(3, &graph.expanded_form_id, &output, &input)
        .unwrap();
    let view = editor.view();
    assert!(view.source.contains("literal.text > upper.text"));
    let graph = PatchbayGraph::from_expanded(&editor.expand_form("compose").unwrap()).unwrap();
    assert_eq!(graph.cords.len(), 1);
    editor
        .remove_cord(4, &graph.expanded_form_id, &graph.cords[0].identity)
        .unwrap();
    assert!(!editor.view().source.contains("literal.text > upper.text"));

    let graph = PatchbayGraph::from_expanded(&editor.expand_form("compose").unwrap()).unwrap();
    let output = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "compose/literal-2")
        .unwrap()
        .outputs[0]
        .identity
        .clone();
    let input = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "compose/upper")
        .unwrap()
        .inputs[0]
        .identity
        .clone();
    editor
        .connect_ports(5, &graph.expanded_form_id, &output, &input)
        .unwrap();
    assert!(editor.view().source.contains("literal-2.text > upper.text"));

    editor.remove_gear(6, "upper").unwrap();
    let view = editor.view();
    assert!(!view.source.contains("upper:"));
    assert!(!view.source.contains("literal.text > upper.text"));
    assert!(!view.source.contains("literal-2.text > upper.text"));
    assert!(view.source.contains("literal-2: text/literal(\"hello\")"));
    assert_eq!(
        PatchbayGraph::from_expanded(&editor.expand_form("compose").unwrap())
            .unwrap()
            .cords
            .len(),
        0
    );
}

#[test]
fn incompatible_duplicate_and_stale_composition_edits_preserve_source() {
    let mut editor = FormEditor::from_source(
        PathBuf::from("compose.conduit"),
        "form compose {\n    count: state/count(0)\n    show: presentation/text\n}\n".into(),
    )
    .unwrap();
    let graph = PatchbayGraph::from_expanded(&editor.expand_form("compose").unwrap()).unwrap();
    let output = graph.gears[0].outputs[0].identity.clone();
    let input = graph.gears[1].inputs[0].identity.clone();
    let before = editor.view().source;
    assert!(matches!(
        editor.connect_ports(0, &graph.expanded_form_id, &output, &input),
        Err(FormEditorError::IncompatiblePorts(_))
    ));
    assert_eq!(editor.view().source, before);
    assert_eq!(
        editor.remove_gear(1, "count"),
        Err(FormEditorError::StaleRevision {
            current: 0,
            offered: 1
        })
    );
    assert_eq!(editor.view().source, before);
}

#[test]
fn reroute_either_cord_endpoint_changes_identities_and_can_be_reversed() {
    let mut editor = FormEditor::from_source(
        PathBuf::from("reroute.conduit"),
        "form reroute {\n    literal: text/literal(\"hello\")\n    literal-2: text/literal(\"again\")\n    upper: text/upper\n    upper-2: text/upper\n    count: state/count(0)\n    literal.text > upper.text\n}\n".into(),
    )
    .unwrap();
    let original = PatchbayGraph::from_expanded(&editor.expand_form("reroute").unwrap()).unwrap();
    let original_ids = (
        original.source_document_id.clone(),
        original.checked_form_id.clone(),
        original.expanded_form_id.clone(),
    );
    let second_sink = original
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "reroute/upper-2")
        .unwrap()
        .inputs[0]
        .identity
        .clone();
    let incompatible_sink = original
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "reroute/count")
        .unwrap()
        .inputs[0]
        .identity
        .clone();
    let unchanged_source = editor.view().source;
    assert!(matches!(
        editor.reroute_cord_endpoint(
            0,
            &original.expanded_form_id,
            &original.cords[0].identity,
            &incompatible_sink,
        ),
        Err(FormEditorError::IncompatiblePorts(_))
    ));
    assert_eq!(editor.view().source, unchanged_source);
    editor
        .reroute_cord_endpoint(
            0,
            &original.expanded_form_id,
            &original.cords[0].identity,
            &second_sink,
        )
        .unwrap();
    assert!(editor.view().source.contains("literal.text > upper-2.text"));
    let rerouted = PatchbayGraph::from_expanded(&editor.expand_form("reroute").unwrap()).unwrap();
    assert_ne!(
        original_ids,
        (
            rerouted.source_document_id.clone(),
            rerouted.checked_form_id.clone(),
            rerouted.expanded_form_id.clone()
        )
    );
    let original_sink = rerouted
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "reroute/upper")
        .unwrap()
        .inputs[0]
        .identity
        .clone();
    let second_source = rerouted
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "reroute/literal-2")
        .unwrap()
        .outputs[0]
        .identity
        .clone();
    editor
        .reroute_cord_endpoint(
            1,
            &rerouted.expanded_form_id,
            &rerouted.cords[0].identity,
            &second_source,
        )
        .unwrap();
    assert!(editor
        .view()
        .source
        .contains("literal-2.text > upper-2.text"));
    let source_rerouted =
        PatchbayGraph::from_expanded(&editor.expand_form("reroute").unwrap()).unwrap();
    editor
        .reroute_cord_endpoint(
            2,
            &source_rerouted.expanded_form_id,
            &source_rerouted.cords[0].identity,
            &original_sink,
        )
        .unwrap();
    assert!(editor.view().source.contains("literal-2.text > upper.text"));
    let sink_reversed =
        PatchbayGraph::from_expanded(&editor.expand_form("reroute").unwrap()).unwrap();
    let original_source = sink_reversed
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "reroute/literal")
        .unwrap()
        .outputs[0]
        .identity
        .clone();
    editor
        .reroute_cord_endpoint(
            3,
            &sink_reversed.expanded_form_id,
            &sink_reversed.cords[0].identity,
            &original_source,
        )
        .unwrap();
    assert!(editor.view().source.contains("literal.text > upper.text"));
}
