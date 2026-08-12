use super::{Arguments, PatchbayApplication};
use winit::keyboard::{Key, NamedKey};

fn application(label: &str) -> (PatchbayApplication, std::path::PathBuf) {
    let directory =
        std::env::temp_dir().join(format!("patchbay-details-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("making.conduit");
    std::fs::write(
        &path,
        "form making {\n    message: text/literal(\"Hello\")\n}\n",
    )
    .unwrap();
    let app = PatchbayApplication::new(Arguments {
        form_path: Some(path),
        ..Arguments::default()
    })
    .unwrap();
    (app, directory)
}

#[test]
fn ordinary_typing_backspace_and_enter_cannot_mutate_source() {
    let (mut app, directory) = application("readonly");
    let before = app.form_editor.as_ref().unwrap().view().source;
    for key in [
        Key::Character("invented".into()),
        Key::Named(NamedKey::Backspace),
    ] {
        assert!(app.handle_form_key(&key).unwrap());
        assert_eq!(app.form_editor.as_ref().unwrap().view().source, before);
    }
    app.handle_gui_action(super::gui::GuiAction::ToggleLinearView)
        .unwrap();
    assert!(app.handle_form_key(&Key::Named(NamedKey::Enter)).unwrap());
    assert_eq!(app.form_editor.as_ref().unwrap().view().source, before);
    assert!(app
        .interaction_status
        .current()
        .unwrap()
        .text
        .contains("read-only"));

    std::fs::remove_file(directory.join("making.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn source_is_exact_and_all_lens_retains_complete_linear_truth() {
    let (mut app, directory) = application("lenses");
    let source = app.form_editor.as_ref().unwrap().view().source;
    let source_lines = app.details_lines();
    assert_eq!(source_lines[3..].join("\n"), source.trim_end());
    let complete = app.presentation_lines();
    let graph = app.graphical_form.as_ref().unwrap();
    let identities = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
    );
    app.details_lens = super::details::DetailsLens::Checked;
    let checked = app.details_lines().join("\n");
    assert!(checked.contains(identities.0.as_str()));
    assert!(checked.contains(identities.1.as_str()));
    assert!(checked.contains(identities.2.as_str()));
    for _ in 0..20 {
        app.details_lens.move_by(1);
    }
    assert_eq!(app.details_lens, super::details::DetailsLens::All);
    let all = app.details_lines();
    assert_eq!(&all[2..], complete);
    app.details_scroll = usize::MAX;
    assert_eq!(app.details_lines().len(), 2);

    std::fs::remove_file(directory.join("making.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn semantic_placement_still_regenerates_canonical_source() {
    let (mut app, directory) = application("semantic");
    let before = app.form_editor.as_ref().unwrap().view().source;
    app.palette.focus();
    app.palette.append("uppercase").unwrap();
    assert!(app.handle_palette_key(&Key::Named(NamedKey::Enter)));
    let after = app.form_editor.as_ref().unwrap().view().source;
    assert_ne!(after, before);
    assert!(after.contains("text/upper"));
    app.handle_gui_action(super::gui::GuiAction::SaveForm)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(directory.join("making.conduit")).unwrap(),
        after
    );
    assert!(app
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .last()
        .is_some_and(
            |receipt| receipt.disposition == patchbay_model::InteractionDisposition::Succeeded
        ));

    std::fs::remove_file(directory.join("making.conduit")).unwrap();
    std::fs::remove_file(directory.join("making.conduit.patchbay.json")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn keyboard_traversal_is_bounded_and_reaches_every_linear_fact() {
    let (mut app, directory) = application("traversal");
    app.handle_gui_action(super::gui::GuiAction::ToggleLinearView)
        .unwrap();
    for _ in 0..20 {
        assert!(app
            .handle_form_key(&Key::Named(NamedKey::ArrowRight))
            .unwrap());
    }
    assert_eq!(app.details_lens, super::details::DetailsLens::All);
    let complete = app.presentation_lines();
    for (offset, expected) in complete.iter().enumerate() {
        assert_eq!(app.details_scroll, offset);
        assert_eq!(app.details_lines().get(2), Some(expected));
        app.handle_form_key(&Key::Named(NamedKey::ArrowDown))
            .unwrap();
    }
    assert_eq!(app.details_scroll, complete.len().saturating_sub(1));
    app.handle_form_key(&Key::Named(NamedKey::ArrowDown))
        .unwrap();
    assert_eq!(app.details_scroll, complete.len().saturating_sub(1));
    for _ in 0..complete.len().saturating_add(2) {
        app.handle_form_key(&Key::Named(NamedKey::ArrowUp)).unwrap();
    }
    assert_eq!(app.details_scroll, 0);

    std::fs::remove_file(directory.join("making.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
