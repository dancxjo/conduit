use super::{Arguments, PatchbayApplication, BACKGROUND};

fn redraw(application: &mut PatchbayApplication) {
    let mut pixels = vec![BACKGROUND; 1_100 * 720];
    application.hit_targets = super::gui::draw_patchbay(
        &mut pixels,
        1_100,
        720,
        application.graphical_form.as_ref().unwrap(),
        super::gui::PatchbayViewContext {
            selected: application.selected_graphical_identity(),
            lifecycle: &Default::default(),
            palette_query: "",
            presentation_layout: &application.layout,
        },
    );
}

fn topmost_point_for(
    application: &PatchbayApplication,
    wanted: impl Fn(&super::gui::GuiAction) -> bool,
) -> (f64, f64) {
    for y in 53..650 {
        for x in 177..800 {
            let action = application
                .hit_targets
                .iter()
                .rev()
                .find(|target| target.contains(x as f64, y as f64))
                .map(|target| &target.action);
            if action.is_some_and(&wanted) {
                return (x as f64, y as f64);
            }
        }
    }
    panic!("wanted production hit target has no topmost point")
}

#[test]
fn pointer_moves_presentation_route_then_reroutes_authored_sink() {
    let directory = std::env::temp_dir().join(format!("patchbay-cord-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("cord.conduit");
    std::fs::write(
        &path,
        "form cord {\n    literal: text/literal(\"hello\")\n    literal-2: text/literal(\"again\")\n    upper: text/upper\n    upper-2: text/upper\n    literal.text > upper.text\n}\n",
    )
    .unwrap();
    let mut application = PatchbayApplication::new(Arguments {
        form_path: Some(path.clone()),
        ..Arguments::default()
    })
    .unwrap();
    application
        .control
        .request_plan(application.form_editor.as_ref().unwrap())
        .unwrap();
    let original_plan = application.control.plan().unwrap().plan_id.clone();
    redraw(&mut application);
    let cord_identity = application.graphical_form.as_ref().unwrap().cords[0]
        .identity
        .clone();
    let cord_point = topmost_point_for(
        &application,
        |action| matches!(action, super::gui::GuiAction::SelectSubject(subject) if subject.subject_identity == cord_identity),
    );
    application.cursor_position = cord_point;
    application.handle_canvas_press().unwrap();
    application.cursor_position = (690.0, 610.0);
    application.handle_canvas_release().unwrap();
    let graph = application.graphical_form.as_ref().unwrap();
    assert_eq!(
        application
            .layout
            .cord_route(&graph.cords[0].source_port, &graph.cords[0].sink_port),
        Some((690, 610))
    );
    let unchanged_ids = (
        graph.source_document_id.clone(),
        graph.checked_form_id.clone(),
        graph.expanded_form_id.clone(),
    );

    redraw(&mut application);
    let cord_identity = application.graphical_form.as_ref().unwrap().cords[0]
        .identity
        .clone();
    let cord_point = topmost_point_for(
        &application,
        |action| matches!(action, super::gui::GuiAction::SelectSubject(subject) if subject.subject_identity == cord_identity),
    );
    let sink_identity = application
        .graphical_form
        .as_ref()
        .unwrap()
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "cord/upper-2")
        .unwrap()
        .inputs[0]
        .identity
        .clone();
    let sink_point = topmost_point_for(
        &application,
        |action| matches!(action, super::gui::GuiAction::SelectSubject(subject) if subject.subject_identity == sink_identity),
    );
    application.cursor_position = cord_point;
    application.handle_canvas_press().unwrap();
    application.cursor_position = sink_point;
    application.handle_canvas_release().unwrap();
    let rerouted = application.graphical_form.as_ref().unwrap();
    assert!(application
        .form_editor
        .as_ref()
        .unwrap()
        .view()
        .source
        .contains("literal.text > upper-2.text"));
    assert_ne!(
        unchanged_ids,
        (
            rerouted.source_document_id.clone(),
            rerouted.checked_form_id.clone(),
            rerouted.expanded_form_id.clone()
        )
    );
    assert!(application.layout.cords.is_empty());
    assert_eq!(application.control.plan().unwrap().plan_id, original_plan);
    application
        .control
        .request_plan(application.form_editor.as_ref().unwrap())
        .unwrap();
    assert_ne!(application.control.plan().unwrap().plan_id, original_plan);
    redraw(&mut application);
    let cord_identity = application.graphical_form.as_ref().unwrap().cords[0]
        .identity
        .clone();
    let cord_point = topmost_point_for(
        &application,
        |action| matches!(action, super::gui::GuiAction::SelectSubject(subject) if subject.subject_identity == cord_identity),
    );
    let source_identity = application
        .graphical_form
        .as_ref()
        .unwrap()
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "cord/literal-2")
        .unwrap()
        .outputs[0]
        .identity
        .clone();
    let source_point = topmost_point_for(
        &application,
        |action| matches!(action, super::gui::GuiAction::SelectSubject(subject) if subject.subject_identity == source_identity),
    );
    application.cursor_position = cord_point;
    application.handle_canvas_press().unwrap();
    application.cursor_position = source_point;
    application.handle_canvas_release().unwrap();
    assert!(application
        .form_editor
        .as_ref()
        .unwrap()
        .view()
        .source
        .contains("literal-2.text > upper-2.text"));
    assert!(application
        .interaction
        .as_ref()
        .unwrap()
        .history()
        .any(|receipt| {
            matches!(
                &receipt.request,
                patchbay_model::PatchbayInteractionRequest::Edit {
                    edit: patchbay_model::PatchbayEdit::RerouteCord { .. },
                    ..
                }
            )
        }));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
