use super::{gui::GuiAction, Arguments, PatchbayApplication};
use winit::keyboard::{Key, ModifiersState, NamedKey};

fn application(label: &str) -> (PatchbayApplication, std::path::PathBuf) {
    let directory = std::env::temp_dir().join(format!(
        "patchbay-forms-navigator-{label}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("greet.conduit");
    std::fs::write(&path, include_str!("../../../../examples/greet.conduit")).unwrap();
    let app = PatchbayApplication::new(Arguments {
        form_path: Some(path),
        ..Arguments::default()
    })
    .unwrap();
    (app, directory)
}

#[test]
fn rows_are_exact_actions_or_explicitly_unavailable() {
    let (app, directory) = application("rows");
    let rows = app.form_navigator_entries();
    assert!(rows[0].label.contains("ROOT default-welcome [CURRENT]"));
    assert!(rows[0].action.is_none());
    assert!(rows.iter().any(|row| {
        row.label == "CHILD hello : greet"
            && matches!(row.action, Some(GuiAction::OpenNavigatorComposition(_)))
    }));
    assert!(rows.iter().all(|row| {
        row.action.is_some()
            || row.label.contains("[CURRENT]")
            || row.label.contains("[UNAVAILABLE")
    }));

    std::fs::remove_file(directory.join("greet.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn composition_and_ancestor_actions_synchronize_every_form_surface() {
    let (mut app, directory) = application("actions");
    let child = app
        .form_navigator_entries()
        .into_iter()
        .find(|row| row.label == "CHILD hello : greet")
        .unwrap()
        .action
        .unwrap();
    app.handle_gui_action(child).unwrap();
    assert_eq!(app.form_editor.as_ref().unwrap().view().open_form, "greet");
    assert_eq!(app.graphical_form.as_ref().unwrap().form_name, "greet");
    assert_eq!(app.back_breadcrumb(), "default-welcome > hello : greet");
    assert!(app.selected_graphical_identity().is_none());
    app.details_lens = super::details::DetailsLens::Checked;
    assert!(app.details_lines().join("\n").contains(
        app.graphical_form
            .as_ref()
            .unwrap()
            .expanded_form_id
            .as_str()
    ));

    let ancestor = app
        .form_navigator_entries()
        .into_iter()
        .find(|row| row.label.contains("[ANCESTOR]"))
        .unwrap()
        .action
        .unwrap();
    let mut stale = ancestor.clone();
    if let GuiAction::OpenNavigatorAncestor {
        expanded_form_id, ..
    } = &mut stale
    {
        expanded_form_id.push_str("-stale");
    }
    assert!(app.handle_gui_action(stale).is_err());
    assert_eq!(app.form_editor.as_ref().unwrap().view().open_form, "greet");
    app.handle_gui_action(ancestor).unwrap();
    assert_eq!(
        app.form_editor.as_ref().unwrap().view().open_form,
        "default-welcome"
    );
    assert!(app.back_navigation.is_empty());

    std::fs::remove_file(directory.join("greet.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn alt_keyboard_traversal_is_nonwrapping_and_activates_the_same_action() {
    let (mut app, directory) = application("keyboard");
    app.modifiers = ModifiersState::ALT;
    for _ in 0..20 {
        app.handle_form_key(&Key::Named(NamedKey::ArrowDown))
            .unwrap();
    }
    let last = app.form_navigator_entries().len() - 1;
    assert_eq!(app.navigator_selection, last);
    assert!(app.navigator_scroll <= last);
    for _ in 0..20 {
        app.handle_form_key(&Key::Named(NamedKey::ArrowUp)).unwrap();
    }
    assert_eq!(app.navigator_selection, 0);
    app.handle_form_key(&Key::Named(NamedKey::ArrowDown))
        .unwrap();
    app.handle_form_key(&Key::Named(NamedKey::Enter)).unwrap();
    assert_eq!(app.form_editor.as_ref().unwrap().view().open_form, "greet");

    std::fs::remove_file(directory.join("greet.conduit")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
