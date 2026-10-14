use crate::{PlotEditor, PlotEditorError};
use conduit_core::{kind_id, ConfigurationValue};
use std::path::PathBuf;

fn editor() -> PlotEditor {
    PlotEditor::from_source(PathBuf::from("named.conduit"), "plot named {\n}\n".into()).unwrap()
}

#[test]
fn face_placement_requires_an_explicit_valid_name_without_changing_source() {
    let kinds = editor()
        .authoring_catalog()
        .unwrap()
        .into_iter()
        .filter(|entry| entry.authorable && entry.contract.kind_id.as_str().starts_with("face/"))
        .map(|entry| entry.contract.kind_id)
        .collect::<Vec<_>>();
    assert!(!kinds.is_empty());
    for kind in kinds {
        let mut editor = editor();
        let before = editor.view();
        assert!(matches!(
            editor.place_palette_kind(0, &kind),
            Err(PlotEditorError::InvalidConfiguration(_))
        ));
        assert_eq!(editor.view(), before);
        for label in ["   ", "Room\ntemperature", ""] {
            assert!(matches!(
                editor.place_palette_kind_with_label(0, &kind, Some(label)),
                Err(PlotEditorError::InvalidConfiguration(_))
            ));
            assert_eq!(editor.view(), before);
        }
    }
}

#[test]
fn data_name_is_checked_configuration_and_changes_authored_identity() {
    let mut first = editor();
    let label = "Boiler \"flow\" \\ return";
    first
        .place_palette_kind_with_label(0, &kind_id("face/text"), Some(label))
        .unwrap();
    assert!(first
        .view()
        .source
        .contains("face/text(\"Boiler \\\"flow\\\" \\\\ return\""));
    let expanded = first.expand_plot_for_authoring("named").unwrap();
    let field = expanded
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id == kind_id("face/text"))
        .unwrap()
        .configuration
        .iter()
        .find(|field| field.key.as_str() == "label")
        .unwrap();
    assert_eq!(field.value, ConfigurationValue::Text(label.into()));

    let mut second = editor();
    second
        .place_palette_kind_with_label(0, &kind_id("face/text"), Some("Boiler pressure"))
        .unwrap();
    assert_ne!(
        first.view().checked.source_document_id,
        second.view().checked.source_document_id
    );
}

#[test]
fn zero_body_admits_the_callers_name_and_refuses_unnamed_face_placement_atomically() {
    use crate::{
        PatchbayEdit, PatchbayEditBasis, PatchbayInvocationOutcome, PatchbayRefusal, PlotCandidate,
        ZeroBodyFrontDoor,
    };
    use conduit_core::{BootId, HostId, SignId};
    let mut session = ZeroBodyFrontDoor::with_identity(
        crate::host_adapter::test_host_adapter_arc(),
        HostId::from("named/host"),
        BootId::from("named/boot"),
    )
    .unwrap();
    let plot = PlotCandidate::from_source(
        "Named",
        "named.conduit",
        "plot named {\n}\n",
        "named data",
        SignId::from("named/source"),
        1,
    )
    .unwrap();
    let plot_id = plot.checked_plot_id.clone();
    session.add_plot(plot).unwrap();
    session.open_plot(&plot_id, session.revision()).unwrap();
    let before = session.opened_plot_document().unwrap();
    let graph = session
        .opened_plot_editor()
        .unwrap()
        .patchbay_graph_for_authoring("named")
        .unwrap();
    let basis = PatchbayEditBasis::new(
        before.checked.source_document_id.clone().unwrap(),
        before.revision,
        graph.expanded_plot_id,
    )
    .unwrap();
    let missing = PatchbayEdit::PlaceGear {
        basis: basis.clone(),
        kind_id: "face/text".into(),
        data_label: None,
    };
    assert_eq!(
        session.apply_opened_plot_edit(&missing),
        PatchbayInvocationOutcome::Refused(PatchbayRefusal::InvalidConfiguration)
    );
    assert_eq!(session.opened_plot_document().unwrap(), before);
    let named = PatchbayEdit::PlaceGear {
        basis,
        kind_id: "face/text".into(),
        data_label: Some("Room status".into()),
    };
    assert_eq!(
        session.apply_opened_plot_edit(&named),
        PatchbayInvocationOutcome::Succeeded
    );
    assert!(session
        .opened_plot_document()
        .unwrap()
        .source
        .contains("face/text(\"Room status\""));
}

#[test]
fn each_authorable_face_kind_can_be_constructed_with_an_explicit_name() {
    let kinds = editor()
        .authoring_catalog()
        .unwrap()
        .into_iter()
        .filter(|entry| entry.authorable && entry.contract.kind_id.as_str().starts_with("face/"))
        .map(|entry| entry.contract.kind_id)
        .collect::<Vec<_>>();
    assert!(
        !kinds.is_empty(),
        "the authoring profile contains Face constructors"
    );
    for kind in kinds {
        let mut editor = editor();
        assert!(
            matches!(
                editor.place_palette_kind(0, &kind),
                Err(PlotEditorError::InvalidConfiguration(_))
            ),
            "{} admitted unnamed data",
            kind.as_str()
        );
        editor
            .place_palette_kind_with_label(0, &kind, Some("Room condition"))
            .unwrap_or_else(|error| {
                panic!("{} refused a named constructor: {error}", kind.as_str())
            });
    }
}
