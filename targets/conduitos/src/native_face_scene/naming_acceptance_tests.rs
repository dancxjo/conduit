//! Actual naming-field pagination and finite page-cost evidence; CPU only.
//! Generic pressure/oversized continuation controls live in field_cluster_tests.
use super::*;
use alloc::{format, vec::Vec};
use conduit_birth_plot::{BirthDraft, BirthFaceBasis, BirthPlotChoice};
use conduit_face::*;

fn actual_face() -> Face {
    let choices = crate::native_workset::profile()
        .installed()
        .iter()
        .map(|plot| BirthPlotChoice {
            title: plot.title().into(),
            search_text: plot.title().into(),
            plot: crate::native_workset::resident(*plot).unwrap(),
            refusal: None,
            selected: true,
        })
        .collect::<Vec<_>>();
    assert_eq!(choices.len(), 4);
    let mut draft =
        BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    assert!(draft.naming_systems().any(|(id, _)| id == "welsh"));
    draft.suggest(draft.revision(), "welsh").unwrap();
    let name = format!("{}{}", "a".repeat(52), "é".repeat(6));
    assert_eq!(name.len(), 64);
    draft.edit_name(draft.revision(), name).unwrap();
    let producer = tests::fixture::producer_plan();
    draft
        .face(&BirthFaceBasis {
            host_id: "host/naming-acceptance".into(),
            boot_id: "boot/naming-acceptance".into(),
            encounter_id: "arrival/naming-acceptance".into(),
            producer_plot: conduit_core::PlotIdentity {
                source_document_id: producer.source_document_id,
                checked_plot_id: producer.checked_plot_id,
                expanded_plot_id: producer.expanded_plot_id,
            },
            producer_plan_id: producer.plan_id,
        })
        .unwrap()
}

fn configured(face: &Face, width: u16, height: u16) -> NativeFaceScene {
    let provider = tests::fixture::framebuffer_provider_entry();
    let host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider,
    );
    let (_, configuration) = conduit_conduitos_mask_offer::prepare_creche_stage_from_offer(
        &host,
        CrecheComponentRecipe::ResponsiveColumns,
    )
    .unwrap();
    let info =
        |bytes: &[u8]| conduit_core::StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let recipe =
        prepare_component_recipe(info(configuration.recipe.canonical_value()), face).unwrap();
    let expression = admit_visual_expression(
        Ok(visual_expression_from_info(info(configuration.expression.canonical_value())).unwrap()),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    NativeFaceScene::prepare_configured(face.clone(), width, height, true, &recipe, &expression)
        .unwrap()
}

fn exact_document(items: &[document::Item], rows: &[layout::Row]) {
    assert_eq!(
        items.iter().map(|i| i.text.as_str()).collect::<String>(),
        rows.iter().map(|r| r.text.as_str()).collect::<String>(),
        "lost/reordered Source wording"
    );
    let mut expected = items
        .iter()
        .filter_map(|i| i.control.map(|c| (i.subject_index, c)))
        .collect::<Vec<_>>();
    let mut observed = rows
        .iter()
        .filter_map(|r| r.control.map(|c| (r.subject_index, c)))
        .collect::<Vec<_>>();
    // Wrapped fragments retain their original control; they are not new controls.
    expected.dedup();
    observed.dedup();
    assert_eq!(expected, observed, "lost/reordered/rebound controls");
    // Each subject's exact reading survives wrapping and panel reordering.
    for subject in items.iter().filter_map(|i| i.subject_index) {
        assert_eq!(
            items
                .iter()
                .filter(|i| i.subject_index == Some(subject))
                .map(|i| i.text.as_str())
                .collect::<String>(),
            rows.iter()
                .filter(|r| r.subject_index == Some(subject))
                .map(|r| r.text.as_str())
                .collect::<String>()
        );
    }
}

fn naming_group(scene: &NativeFaceScene) {
    let action = scene
        .face
        .actions
        .iter()
        .position(|a| a.identity == "creche.naming")
        .unwrap();
    let source = &scene.face.actions[action];
    assert_eq!(source.name, "Use naming tradition");
    let subject = scene
        .face
        .subjects
        .iter()
        .position(|s| s.identity == source.target)
        .unwrap();
    assert!(source.target.ends_with("/name-system"));
    let rows = scene
        .primary
        .iter()
        .filter(|r| r.subject_index == Some(subject))
        .collect::<Vec<_>>();
    let words = rows.iter().map(|r| r.text.as_str()).collect::<String>();
    assert!(words.contains("Choose a tradition for the next name suggestion."));
    assert!(words.contains("Use naming tradition"));
    assert!(words.contains("Naming tradition: welsh · Left/Right choose"));
    assert_eq!(source.arguments.len(), 1);
    assert_eq!(source.arguments[0].name, "value");
    assert_eq!(source.arguments[0].contract.maximum_bytes, 64);
    assert!(source.arguments[0].contract.validate(b"welsh").is_ok());
    assert!(rows.iter().any(|r| r.control
        == Some(FaceControl {
            action,
            argument: None
        })));
    assert!(rows.iter().any(|r| r.control
        == Some(FaceControl {
            action,
            argument: Some(0)
        })));
    // This actual short field must fit coherently at both admitted real extents.
    // Oversized-field continuation is separately tested; do not excuse this field.
    assert!(!rows.is_empty());
    assert!(
        rows.iter().all(|r| r.page == rows[0].page),
        "actual naming field orphaned across pages"
    );
    assert!(
        rows.iter().all(|r| r.bounds.x == rows[0].bounds.x),
        "actual naming field orphaned across lanes"
    );
}

fn page_costs(scene: &mut NativeFaceScene, configured: bool) {
    // Literal acceptance limits intentionally detect a silently widened contract.
    assert_eq!(MAX_GRAPHICS_COMMANDS, 48);
    assert_eq!(MAX_GRAPHICS_TEXT_BYTES, 192);
    assert_eq!(layout::MAX_PAGE_ROWS, 20);
    for details in [false, true] {
        scene.showing_details = details;
        scene.focus = None;
        for page in 0..scene.pages() {
            scene.page = page;
            let rows = if details {
                &scene.details
            } else {
                &scene.primary
            };
            let rows = rows.iter().filter(|r| r.page == page).collect::<Vec<_>>();
            assert!(!rows.is_empty());
            assert!(rows.len() <= 20);
            let mut surfaces = Vec::new();
            for row in &rows {
                if let Some(panel) = row.panel_bounds {
                    if !surfaces.contains(&panel) {
                        surfaces.push(panel);
                    }
                }
                std::println!(
                    "naming_row configured={configured} details={details} extent={}x{} page={} subject={:?} control={:?} bounds={:?} panel={:?} bytes={} text={}",
                    scene.screen.width,
                    scene.screen.height,
                    page + 1,
                    row.subject_index,
                    row.control,
                    row.bounds,
                    row.panel_bounds,
                    row.text.len(),
                    row.text
                );
            }
            assert!(surfaces.len() <= 8);
            let frame = scene.frame().unwrap();
            let commands = frame.scene.commands();
            assert!(commands.len() <= 48);
            assert!(
                commands.iter().all(|c| c.payload().len() <= 192
                    && c.clip_class() == GraphicsClipClass::FullyVisible)
            );
            for hit in &frame.hits {
                assert_eq!(hit.action, &scene.face.actions[hit.control.action]);
                assert_eq!(
                    hit.argument,
                    hit.control.argument.map(|i| &hit.action.arguments[i])
                );
            }
            let bytes = commands.iter().map(|c| c.payload().len()).sum::<usize>();
            let chrome = chrome::Chrome::prepare(scene.screen, scene.text_scale_percent).unwrap();
            let bottom = rows
                .iter()
                .map(|r| i32::from(r.bounds.y) + i32::from(r.bounds.height))
                .max()
                .unwrap();
            std::println!(
                "naming_page_cost configured={configured} details={details} extent={}x{} page={}/{} rows={}/20 commands={}/48 payload_bytes={} max_payload_bytes={} surfaces={}/8 content_top={} content_bottom={} last_row_bottom={} spare_below={}",
                scene.screen.width,
                scene.screen.height,
                page + 1,
                scene.pages(),
                rows.len(),
                commands.len(),
                bytes,
                commands
                    .iter()
                    .map(|c| c.payload().len())
                    .max()
                    .unwrap_or(0),
                surfaces.len(),
                chrome.top,
                chrome.bottom,
                bottom,
                i32::from(chrome.bottom) - bottom
            );
            // Costs/geometry explain observed finite pagination; spare pixels alone
            // do not prove that another whole authored group could be admitted.
        }
    }
}

#[test]
fn actual_welsh_unicode_naming_group_configured_at_both_real_extents() {
    let face = actual_face();
    let (primary, details) = document::prepare(&face, true).unwrap();
    for (width, height) in [(1280, 800), (640, 480)] {
        let mut scene = configured(&face, width, height);
        assert_eq!(scene.face(), &face);
        exact_document(&primary, &scene.primary);
        exact_document(&details, &scene.details);
        naming_group(&scene);
        page_costs(&mut scene, true);
        assert_eq!(scene.face(), &face);
    }
}

#[test]
fn actual_welsh_unicode_naming_group_generic_at_both_real_extents() {
    let face = actual_face();
    let (primary, details) = document::prepare(&face, true).unwrap();
    for (width, height) in [(1280, 800), (640, 480)] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, height).unwrap();
        exact_document(&primary, &scene.primary);
        exact_document(&details, &scene.details);
        naming_group(&scene);
        page_costs(&mut scene, false);
        assert_eq!(scene.face(), &face);
    }
}
