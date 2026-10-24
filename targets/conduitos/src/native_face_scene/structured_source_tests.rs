//! Reviewed Source + actual domain Face geometry. CPU only, no Host/Show receipt.
use super::*;
use alloc::collections::BTreeSet;
use conduit_face::*;
use conduit_plot::{
    CanonicalStartupValue, ProfileCatalog, check_syntax_document, parse_syntax_document,
};

fn structure<'a>(source: &str, component: &str, face: &'a Face) -> PreparedComponentLayout<'a> {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert_kind(layout_component_mask_kind(48 * 1024))
        .unwrap();
    let mut startup = catalog.startup_catalog().unwrap();
    install_mask_plot_value_aliases(&mut startup).unwrap();
    for (name, schema) in [
        ("LookInfo", look_type()),
        ("ComponentRecipeInfo", component_recipe_type()),
        ("ComponentLayoutInfo", component_layout_type()),
    ] {
        let kind = schema.profile().unwrap().value_kind().clone();
        startup
            .insert_structured_type(name, schema.clone())
            .unwrap();
        startup
            .ensure_structured_type(kind.as_str(), schema)
            .unwrap();
    }
    let document = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let plot = document
        .plots
        .iter()
        .find(|plot| plot.name == component)
        .unwrap();
    let gear = &plot.gears[0];
    let literal = |name| {
        let binding = gear
            .startup_bindings
            .iter()
            .find(|binding| binding.name == name)
            .unwrap();
        let CanonicalStartupValue::Structured(value) = &binding.value else {
            panic!()
        };
        value.try_concrete().unwrap()
    };
    prepare_component_layout(literal("layout"), literal("recipe"), face).unwrap()
}

fn todo() -> Face {
    let state = conduit_todo_plot::TodoState {
        title: "Things".into(),
        revision: 3,
        next_id: 4,
        items: (1..=3)
            .map(|id| conduit_todo_plot::TodoItem {
                id: format!("task-{id}"),
                text: format!("Élan task {id}"),
                complete: id == 2,
            })
            .collect(),
    };
    let fragment = conduit_todo_face::todo_fragment(
        &state,
        FaceContributionBasis {
            checked_plot_id: "checked/todo".into(),
            plan_id: "plan/todo".into(),
            active_play_id: "play/todo".into(),
            required_interaction_context: None,
        },
        true,
    )
    .unwrap();
    Face::new_with_semantics(
        3,
        tests::face(3).basis,
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .unwrap()
}

#[test]
fn reviewed_structured_sources_retain_real_domain_content_and_controls_at_all_pages() {
    verify_reviewed_sources(false);
}

// First-page acceptance is separate from complete continuation conformance.
#[test]
fn wide_creche_shows_all_four_checked_plots_on_first_page() {
    verify_reviewed_sources(true);
}

fn verify_reviewed_sources(require_first_page: bool) {
    let expression = admit_visual_expression(
        Ok(VisualExpression::phosphor()),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    for (source, component, face) in [
        (
            CRECHE_LAYOUT_MASK_SOURCE,
            "creche-component-columns",
            naming_acceptance_tests::actual_face(),
        ),
        (TODO_LAYOUT_MASK_SOURCE, "todo-component-grouped", todo()),
    ] {
        let profile = structure(source, component, &face);
        let (primary, _) = document::prepare(&face, true).unwrap();
        let expected: String = primary.iter().map(|item| item.text.as_str()).collect();
        let controls: BTreeSet<_> = primary
            .iter()
            .filter_map(|item| {
                item.control
                    .map(|control| (control.action, control.argument))
            })
            .collect();
        for (width, height) in [(1280, 800), (640, 480), (320, 480)] {
            let mut scene =
                NativeFaceScene::prepare_structured(&profile, width, height, true, &expression)
                    .unwrap();
            assert_eq!(scene.face(), &face);
            assert_eq!(
                scene
                    .primary
                    .iter()
                    .map(|row| row.text.as_str())
                    .collect::<String>(),
                expected
            );
            assert_eq!(
                scene
                    .primary
                    .iter()
                    .filter_map(|row| row
                        .control
                        .map(|control| (control.action, control.argument)))
                    .collect::<BTreeSet<_>>(),
                controls
            );
            let origin = |region| {
                let row = scene
                    .primary
                    .iter()
                    .find(|row| {
                        row.subject_index.is_some_and(|subject| {
                            profile.recipe().subject_regions[subject] == Some(region)
                        })
                    })
                    .unwrap();
                let item = primary
                    .iter()
                    .find(|item| item.subject_index == row.subject_index)
                    .unwrap();
                i32::from(row.bounds.x) - i32::from(item.indent) * 14
            };
            if width == 1280 {
                // Prove real wide composition, rather than accepting a complete
                // generic one-column reader as the requested panel behavior.
                let (left, right) = if component.starts_with("creche") {
                    (0, 2)
                } else {
                    (0, 1)
                };
                assert!(
                    origin(right) > origin(left) + 32,
                    "wide composition collapsed: {component}"
                );
            }
            if require_first_page && width == 1280 && component == "creche-component-columns" {
                let selected: Vec<_> = scene
                    .primary
                    .iter()
                    .filter(|row| row.checkbox == Some(true))
                    .collect();
                assert_eq!(selected.len(), 4);
                assert!(
                    selected.iter().all(|row| row.page == 0),
                    "wide Crèche must show all four checked starting Plots on the first page"
                );
            }
            if width <= 640 && component == "creche-component-columns" {
                assert_eq!(
                    origin(0),
                    origin(2),
                    "measured narrow composition must collapse coherently"
                );
            }
            if require_first_page && width == 1280 && component == "creche-component-columns" {
                let checked_controls = scene
                    .primary
                    .iter()
                    .filter(|row| row.checkbox == Some(true))
                    .map(|row| {
                        let control = row.control.unwrap();
                        (control.action, control.argument)
                    })
                    .collect::<BTreeSet<_>>();
                let mut reached = BTreeSet::new();
                for _ in 0..scene.primary.len() {
                    if let Some(control) = scene.focus_next(true) {
                        let key = (control.action, control.argument);
                        if checked_controls.contains(&key) {
                            assert_eq!(scene.page, 0);
                            assert!(scene.frame().unwrap().scene.commands().len() <= 48);
                            reached.insert(key);
                        }
                    }
                }
                assert_eq!(reached, checked_controls);
            }
            for page in 0..scene.pages() {
                scene.page = page;
                let rows: Vec<_> = scene
                    .primary
                    .iter()
                    .filter(|row| row.page == page)
                    .collect();
                assert!(!rows.is_empty());
                assert!(rows.len() <= 20);
                for (index, row) in rows.iter().enumerate() {
                    let a = row.bounds;
                    assert!(a.x >= 0 && a.y >= 0);
                    assert!(i32::from(a.x) + i32::from(a.width) <= i32::from(width));
                    assert!(i32::from(a.y) + i32::from(a.height) <= i32::from(height));
                    for other in &rows[index + 1..] {
                        let b = other.bounds;
                        assert!(
                            i32::from(a.x) + i32::from(a.width) <= i32::from(b.x)
                                || i32::from(b.x) + i32::from(b.width) <= i32::from(a.x)
                                || i32::from(a.y) + i32::from(a.height) <= i32::from(b.y)
                                || i32::from(b.y) + i32::from(b.height) <= i32::from(a.y)
                        );
                    }
                }
                let frame = scene.frame().unwrap();
                assert!(frame.scene.commands().len() <= 48);
                for hit in &frame.hits {
                    assert!(controls.contains(&(hit.control.action, hit.control.argument)));
                    let x = hit.bounds.x + (hit.bounds.width / 2) as i16;
                    let y = hit.bounds.y + (hit.bounds.height / 2) as i16;
                    assert_eq!(frame.hit_test(x, y).unwrap().control, hit.control);
                }
            }
        }
    }
}

#[test]
fn whole_item_measurement_preserves_real_creche_and_todo_selected_geometry() {
    // CPU geometry and exact Face bindings; a frame is not acknowledged Show.
    for (source, component, face) in [
        (
            CRECHE_LAYOUT_MASK_SOURCE,
            "creche-component-columns",
            naming_acceptance_tests::actual_face(),
        ),
        (TODO_LAYOUT_MASK_SOURCE, "todo-component-grouped", todo()),
    ] {
        let profile = structure(source, component, &face);
        let (items, _) = document::prepare(&face, true).unwrap();
        let expected: String = items.iter().map(|item| item.text.as_str()).collect();
        let controls: BTreeSet<_> = items
            .iter()
            .filter_map(|item| item.control.map(|c| (c.action, c.argument)))
            .collect();
        for scale in [100, 200] {
            let mut expression = VisualExpression::phosphor();
            expression.text_scale_percent = scale;
            let expression = admit_visual_expression(
                Ok(expression),
                VisualUserConstraints {
                    forced_colors: false,
                    reduced_motion: true,
                    minimum_text_scale_percent: 100,
                    maximum_supported_text_scale_percent: 200,
                },
            )
            .unwrap();
            let mut scene =
                NativeFaceScene::prepare_structured(&profile, 1280, 800, true, &expression)
                    .unwrap();
            assert_eq!(scene.face(), &face);
            assert_eq!(
                scene
                    .primary
                    .iter()
                    .map(|row| row.text.as_str())
                    .collect::<String>(),
                expected
            );
            assert_eq!(
                scene
                    .primary
                    .iter()
                    .filter_map(|row| row.control.map(|c| (c.action, c.argument)))
                    .collect::<BTreeSet<_>>(),
                controls
            );
            for page in 0..scene.pages() {
                scene.page = page;
                let frame = scene.frame().unwrap();
                assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
                for hit in &frame.hits {
                    assert_eq!(hit.action, &face.actions[hit.control.action]);
                    let x = hit.bounds.x + (hit.bounds.width / 2) as i16;
                    let y = hit.bounds.y + (hit.bounds.height / 2) as i16;
                    assert_eq!(frame.hit_test(x, y).unwrap().control, hit.control);
                }
                let rows: Vec<_> = scene
                    .primary
                    .iter()
                    .filter(|row| row.page == page)
                    .collect();
                for (index, row) in rows.iter().enumerate() {
                    let a = row.bounds;
                    assert!(
                        a.x >= 0
                            && a.y >= 0
                            && i32::from(a.x) + i32::from(a.width) <= 1280
                            && i32::from(a.y) + i32::from(a.height) <= 800
                    );
                    for other in &rows[index + 1..] {
                        let b = other.bounds;
                        assert!(
                            i32::from(a.x) + i32::from(a.width) <= i32::from(b.x)
                                || i32::from(b.x) + i32::from(b.width) <= i32::from(a.x)
                                || i32::from(a.y) + i32::from(a.height) <= i32::from(b.y)
                                || i32::from(b.y) + i32::from(b.height) <= i32::from(a.y)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn checked_creche_groups_have_cohesive_surfaces_within_the_original_frame_budget() {
    let face = naming_acceptance_tests::actual_face();
    let prepared = structure(CRECHE_LAYOUT_MASK_SOURCE, "creche-component-columns", &face);
    let expression = admit_visual_expression(
        Ok(VisualExpression::phosphor()),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    let scene =
        NativeFaceScene::prepare_structured(&prepared, 1280, 800, true, &expression).unwrap();
    let frame = scene.frame().unwrap();
    let surfaces: Vec<_> = frame
        .scene
        .commands()
        .iter()
        .filter(|command| {
            command.kind == GraphicsCommandKind::Rect
                && command.paint == GraphicsPaintRole::Surface
                && command.style == GraphicsShapeStyle::RoundedFill
        })
        .collect();
    assert_eq!(surfaces.len(), 2);
    for surface in &surfaces {
        assert!(surface.bounds.x >= 16);
        assert!(i32::from(surface.bounds.x) + i32::from(surface.bounds.width) <= 1280 - 16);
    }
    for row in scene
        .primary
        .iter()
        .filter(|row| row.page == 0 && row.region.is_some())
    {
        assert!(
            row.region_inset > 0,
            "checked card padding must surround its fields"
        );
        let inset = i32::from(row.region_inset);
        assert!(surfaces.iter().any(|surface| {
            let outer = surface.bounds;
            i32::from(outer.x) <= i32::from(row.bounds.x) - inset
                && i32::from(outer.y) <= i32::from(row.bounds.y) - inset
                && i32::from(outer.x) + i32::from(outer.width)
                    >= i32::from(row.bounds.x) + i32::from(row.bounds.width) + inset
                && i32::from(outer.y) + i32::from(outer.height)
                    >= i32::from(row.bounds.y) + i32::from(row.bounds.height) + inset
        }));
    }
    assert_eq!(scene.face(), &face);
    assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
}
