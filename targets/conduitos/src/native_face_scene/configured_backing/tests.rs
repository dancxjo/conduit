//! CPU resource descriptors, not display possession, admission or physical Show.
use super::*;
use conduit_face::{AmbientClockEncounter, FaceRole};
#[path = "fixture.rs"]
mod fixture;

#[test]
fn both_checked_contexts_have_allocation_free_bounds_and_all_pages_fit() {
    let face = fixture::face();
    for encounter in [
        AmbientClockEncounter::Tile,
        AmbientClockEncounter::Fullscreen,
    ] {
        let configuration = fixture::configuration(encounter);
        let structure = fixture::structure(&configuration, &face);
        let look = fixture::look(&configuration);
        let mut bounds = None;
        assert_eq!(
            crate::test_allocations::allocations(|| {
                bounds = Some(
                    NativeFaceScene::configured_interactive_memory_bounds(&structure, &look)
                        .unwrap(),
                );
            }),
            0,
            "Scene lease must precede factory allocation"
        );
        let bounds = bounds.unwrap();
        let extents: &[(u16, u16)] = match encounter {
            AmbientClockEncounter::Tile => &[(320, 320), (640, 480)],
            AmbientClockEncounter::Fullscreen => &[(640, 480), (800, 600), (1024, 600)],
        };
        for &(width, height) in extents {
            let mut scene =
                NativeFaceScene::prepare_structured_look(&structure, width, height, true, &look)
                    .unwrap();
            assert_eq!(scene.face(), &face);
            assert!(scene.look_corners.is_some());
            assert!(scene.image.is_some());
            assert_eq!(scene.standard_interactive_retained_backing_bytes(), None);
            assert!(
                scene
                    .configured_interactive_retained_backing_bytes()
                    .unwrap()
                    <= bounds.retained_bytes
            );
            for details in [false, true] {
                scene.showing_details = details;
                for page in 0..scene.pages() {
                    scene.page = page;
                    let frame = scene.frame().unwrap();
                    assert!(
                        frame.retained_backing_bytes().unwrap() <= bounds.frame_transient_bytes
                    );
                    assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
                }
            }
            scene.showing_details = false;
            scene.page = 0;
            scene.set_local_notice("Bounded local outcome").unwrap();
            assert!(
                scene
                    .configured_interactive_retained_backing_bytes()
                    .unwrap()
                    <= bounds.retained_bytes
            );
            scene.set_local_creche_preferences(true);
            assert_eq!(scene.configured_interactive_retained_backing_bytes(), None);
        }
    }
}

#[test]
fn graph_projection_is_refused_before_configured_scene_allocation() {
    let mut face = fixture::face();
    face.subjects[0].role = FaceRole::Gear;
    let face = fixture::rebuild(face);
    let configuration = fixture::configuration(AmbientClockEncounter::Tile);
    let structure = fixture::structure(&configuration, &face);
    let look = fixture::look(&configuration);
    assert_eq!(
        NativeFaceScene::configured_interactive_memory_bounds(&structure, &look),
        Err(FaceSceneError::UnsupportedInput)
    );
}

#[test]
fn fixed_image_schema_factory_is_accounted_separately_from_scene_payload() {
    let face = fixture::face();
    let schema = conduit_face::ImageResource::semantic_type().unwrap();
    let profile = schema.profile().unwrap();
    let canonical = schema.canonical_bytes().unwrap();
    let observed_retained = schema.retained_backing_bytes().unwrap()
        + profile.value_kind().0.capacity()
        + canonical.capacity();
    let mut bytes = None;
    assert_eq!(
        crate::test_allocations::allocations(|| {
            bytes = Some(image_schema::preparation_bytes(&face).unwrap());
        }),
        0
    );
    assert!(bytes.unwrap() >= observed_retained);
}
