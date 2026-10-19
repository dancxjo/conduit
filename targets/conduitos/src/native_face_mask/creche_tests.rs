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
