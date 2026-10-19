//! CPU checked Source/Plan/Mask/scanout proof; actual QEMU remains separate.
use super::*;
use crate::native_face_scene::raster_layout_tests::Pixels;
use conduit_face::{CrecheComponentRecipe, prepare_component_recipe};
use conduit_human::{KeyModifiers, KeyTransition};

#[test]
fn checked_creche_masks_admit_three_fields_and_present_both_column_counts() {
    let provider = crate::product_bases::fixture_surface_provider();
    let draft = tests::draft();
    for recipe in [
        CrecheComponentRecipe::SingleColumn,
        CrecheComponentRecipe::ResponsiveColumns,
    ] {
        for (width, height) in [(320, 240), (1280, 800)] {
            let mut mask = NativeFaceMask::prepare_creche(
                "host/creche-layout".into(),
                "boot/creche-layout".into(),
                OfferGeneration(1),
                "creche-layout",
                provider.entry.base_id.clone(),
                "surface/creche-layout",
                &provider,
                recipe,
            )
            .unwrap();
            let face = draft
                .face(&mask.birth_basis("arrival/creche-layout"))
                .unwrap();
            assert_eq!(
                face.subjects
                    .iter()
                    .filter(|subject| subject.role == FaceRole::TextEntry)
                    .count(),
                3
            );
            let configuration = mask.component_configuration().unwrap();
            let prepared = prepare_component_recipe(
                conduit_core::StructuredInfoValue::from_canonical_bytes(
                    configuration.recipe.canonical_value(),
                )
                .unwrap(),
                &face,
            )
            .unwrap();
            let columns = prepared.geometry_for_width(width).unwrap().columns;
            assert_eq!(
                columns,
                if width == 1280 && recipe == CrecheComponentRecipe::ResponsiveColumns {
                    2
                } else {
                    1
                }
            );
            let mut pixels = Pixels::new(width, height);
            mask.present(face.clone(), 1, 1, &mut pixels).unwrap();
            assert_eq!(mask.component_fallback(), None);
            assert_eq!(mask.show().unwrap().show.face_id, face.identity);
            let pages = mask.scene().unwrap().pages();
            let mut second_column_control = false;
            for page in 0..pages {
                let scene = mask.scene().unwrap();
                assert_eq!(scene.face(), &face);
                let frame = scene.frame().unwrap();
                assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
                second_column_control |= frame
                    .hits
                    .iter()
                    .any(|hit| hit.bounds.x > (width / 2) as i16);
                let result = mask
                    .key(
                        KeyEvent::new(78, KeyTransition::Pressed, KeyModifiers::NONE).unwrap(),
                        page as u64 + 2,
                        &mut pixels,
                    )
                    .unwrap();
                assert!(matches!(result, NativeFaceMaskInput::Redrawn(_)));
            }
            assert_eq!(second_column_control, columns == 2);
        }
    }
}

#[test]
fn full_inventory_fixture_keeps_long_and_empty_names_and_filtered_review_readable() {
    use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
    // Declared inventory fixture, not sixteen installed or executed Plots.
    let choices = (0..conduit_body::MAX_BODY_PLOTS)
        .map(|index| BirthPlotChoice {
            title: alloc::format!("Plot {index:02} {}", "é".repeat(28)),
            search_text: alloc::string::String::new(),
            selected: true,
            refusal: None,
            plot: conduit_body::ResidentPlot::new(
                alloc::format!("source/fixture/{index}").into(),
                alloc::format!("checked/fixture/{index}").into(),
            ),
        })
        .collect();
    let mut draft =
        BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    let provider = crate::product_bases::fixture_surface_provider();
    for query in ["", "no-match"] {
        draft.search_plots(draft.revision(), query).unwrap();
        for name in ["É".repeat(32), alloc::string::String::new()] {
            draft.edit_name(draft.revision(), name.clone()).unwrap();
            let mut mask = NativeFaceMask::prepare_creche(
                "host/creche-bounds".into(),
                "boot/creche-bounds".into(),
                OfferGeneration(1),
                "creche-bounds",
                provider.entry.base_id.clone(),
                "surface/creche-bounds",
                &provider,
                CrecheComponentRecipe::ResponsiveColumns,
            )
            .unwrap();
            let face = draft
                .face(&mask.birth_basis("arrival/creche-bounds"))
                .unwrap();
            let mut pixels = Pixels::new(320, 240);
            mask.present(face.clone(), 1, 1, &mut pixels).unwrap();
            assert_eq!(mask.component_fallback(), None);
            let mut words = alloc::string::String::new();
            let mut birth_hit = false;
            for page in 0..mask.scene().unwrap().pages() {
                let frame = mask.scene().unwrap().frame().unwrap();
                assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
                for command in frame.scene.commands() {
                    words.push_str(command.payload());
                }
                birth_hit |= frame
                    .hits
                    .iter()
                    .any(|hit| hit.action.identity == "creche.birth");
                mask.key(
                    KeyEvent::new(78, KeyTransition::Pressed, KeyModifiers::NONE).unwrap(),
                    page as u64 + 2,
                    &mut pixels,
                )
                .unwrap();
            }
            assert!(words.contains(&alloc::format!(
                "Friendly Body name: {}",
                if name.is_empty() { "(empty)" } else { &name }
            )));
            for choice in draft.choices() {
                assert!(
                    words.contains(&choice.title),
                    "lost full Plot name with query {query:?}"
                );
            }
            assert_eq!(birth_hit, !name.is_empty());
            assert_eq!(mask.scene().unwrap().face(), &face);
        }
    }
}
