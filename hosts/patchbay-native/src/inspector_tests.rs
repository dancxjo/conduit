use super::{Arguments, PatchbayApplication};
use winit::keyboard::Key;

#[test]
fn exact_identity_disclosure_is_keyboard_reachable_and_presentation_only() {
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/count.conduit"),
        ),
        ..Arguments::default()
    })
    .unwrap();
    let basis = application
        .graphical_form
        .as_ref()
        .map(|graph| {
            (
                graph.source_document_id.clone(),
                graph.checked_form_id.clone(),
                graph.expanded_form_id.clone(),
            )
        })
        .unwrap();
    assert!(!application.exact_identity_open);
    application.modifiers = winit::keyboard::ModifiersState::CONTROL;
    assert!(application
        .handle_form_key(&Key::Character("i".into()))
        .unwrap());
    assert!(application.exact_identity_open);
    let graph = application.graphical_form.as_ref().unwrap();
    assert_eq!(
        basis,
        (
            graph.source_document_id.clone(),
            graph.checked_form_id.clone(),
            graph.expanded_form_id.clone(),
        )
    );
}
