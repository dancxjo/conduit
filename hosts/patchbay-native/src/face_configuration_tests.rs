use super::{gui::GuiAction, Arguments, PatchbayApplication};
use conduit_core::ConfigurationValue;

#[test]
fn native_face_control_uses_interaction_execution_and_persists_canonical_source() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-face-control-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("controls.conduit");
    std::fs::write(
        &path,
        "form controls {\n    clock: time/every(freq = 25ms)\n}\n",
    )
    .unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    let graph = application.graphical_form.as_ref().unwrap();
    let subject = graph.subject_ref("gear/controls/clock").unwrap();
    let gear_id = graph.gears[0].gear_id.clone();
    let old_ids = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
    );
    application
        .control
        .request_plan(application.form_editor.as_ref().unwrap())
        .unwrap();
    let immutable_plan_id = application.control.plan().unwrap().plan_id.clone();
    application
        .handle_gui_action(GuiAction::ConfigureGear {
            subject,
            key: "freq".into(),
            value: ConfigurationValue::U64(26),
        })
        .unwrap();
    let graph = application.graphical_form.as_ref().unwrap();
    assert_eq!(graph.gears[0].gear_id, gear_id);
    assert_ne!(
        (
            graph.source_document_id.clone(),
            graph.checked_form_id.clone(),
            graph.expanded_form_id.clone()
        ),
        old_ids
    );
    assert_eq!(
        graph.gears[0].controls[0].value,
        ConfigurationValue::U64(26)
    );
    assert!(application
        .form_editor
        .as_ref()
        .unwrap()
        .view()
        .source
        .contains("freq = 26ms"));
    assert!(application
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .any(|receipt| matches!(
            &receipt.request,
            patchbay_model::PatchbayInteractionRequest::Edit {
                edit: patchbay_model::PatchbayEdit::ConfigureGear { .. },
                ..
            }
        )));
    assert_eq!(
        application.control.plan().unwrap().plan_id,
        immutable_plan_id
    );
    application
        .control
        .request_plan(application.form_editor.as_ref().unwrap())
        .unwrap();
    assert_ne!(
        application.control.plan().unwrap().plan_id,
        immutable_plan_id
    );
    let graph = application.graphical_form.as_ref().unwrap();
    let semantic_ids = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
    );
    let subject = graph.subject_ref("gear/controls/clock").unwrap();
    application
        .layout
        .move_gear(graph, &subject, 420, 220)
        .unwrap();
    assert_eq!(
        semantic_ids,
        (
            graph.source_document_id.clone(),
            graph.checked_form_id.clone(),
            graph.expanded_form_id.clone()
        )
    );
    application.handle_gui_action(GuiAction::SaveForm).unwrap();
    drop(application);
    let reopened = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    assert_eq!(
        reopened.graphical_form.as_ref().unwrap().gears[0].controls[0].value,
        ConfigurationValue::U64(26)
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(directory.join("controls.conduit.patchbay.json")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn native_face_control_refuses_invalid_value_without_changing_source() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-face-refusal-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("controls.conduit");
    std::fs::write(
        &path,
        "form controls {\n    show: presentation/tick(maximum-values = 3)\n}\n",
    )
    .unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    let graph = application.graphical_form.as_ref().unwrap();
    let subject = graph.subject_ref("gear/controls/show").unwrap();
    let source = application.form_editor.as_ref().unwrap().view().source;
    let error = application
        .handle_gui_action(GuiAction::ConfigureGear {
            subject,
            key: "maximum-values".into(),
            value: ConfigurationValue::U64(5),
        })
        .unwrap_err();
    assert!(error.contains("visible bounds"));
    assert_eq!(
        application.form_editor.as_ref().unwrap().view().source,
        source
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn maximum_short_text_crosses_the_bounded_interaction_envelope() {
    let directory = std::env::temp_dir().join(format!("patchbay-face-text-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("controls.conduit");
    std::fs::write(
        &path,
        "form controls {\n    literal: text/literal(\"hello\")\n}\n",
    )
    .unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    let subject = application
        .graphical_form
        .as_ref()
        .unwrap()
        .subject_ref("gear/controls/literal")
        .unwrap();
    let maximum = "x".repeat(conduit_std_catalog::MAX_TEXT_BYTES as usize);
    application
        .handle_gui_action(GuiAction::ConfigureGear {
            subject,
            key: "value".into(),
            value: ConfigurationValue::Text(maximum.clone()),
        })
        .unwrap();
    assert_eq!(
        application.graphical_form.as_ref().unwrap().gears[0].controls[0].value,
        ConfigurationValue::Text(maximum)
    );
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn pointer_hit_prefers_face_control_over_containing_gear_rectangle() {
    let directory =
        std::env::temp_dir().join(format!("patchbay-face-pointer-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("controls.conduit");
    std::fs::write(
        &path,
        "form controls {\n    clock: time/every(freq = 25ms)\n}\n",
    )
    .unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    let graph = application.graphical_form.as_ref().unwrap();
    let mut pixels = vec![super::BACKGROUND; 1_100 * 720];
    application.hit_targets = super::gui::draw_patchbay(
        &mut pixels,
        1_100,
        720,
        graph,
        super::gui::PatchbayViewContext {
            selected: None,
            breadcrumb: "",
            lifecycle: &Default::default(),
            palette_query: "",
            presentation_layout: &application.layout,
            realization_plan: None,
            realization_hosts: &[],
        },
    );
    // The first control is inside the first Gear rectangle. Later control hit
    // geometry must win over that containing selection target.
    application.cursor_position = (220.0, 150.0);
    application.handle_canvas_press().unwrap();
    assert_eq!(
        application.graphical_form.as_ref().unwrap().gears[0].controls[0].value,
        ConfigurationValue::U64(24)
    );
    assert!(application
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .any(|receipt| {
            matches!(
                &receipt.request,
                patchbay_model::PatchbayInteractionRequest::Edit {
                    edit: patchbay_model::PatchbayEdit::ConfigureGear { .. },
                    ..
                }
            )
        }));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
