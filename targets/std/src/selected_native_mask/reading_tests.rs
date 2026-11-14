//! CPU SDL/managed IMAGE proof; no physical input or display claim.
use super::*;
use std::sync::atomic::Ordering;

fn page(host: &StdHost) -> usize {
    match &host.ready_selected_native_mask().unwrap().stores {
        Stores::Bound { renderer, .. } => renderer.scene.as_ref().unwrap().page(),
        _ => panic!("actual bound renderer required"),
    }
}
fn retire(host: &mut StdHost) {
    let mut evidence = host.take_selected_native_mask_evidence().unwrap();
    host.retire_selected_native_mask_evidence(&mut evidence)
        .unwrap();
}
#[test]
fn managed_reading_cursor_survives_refresh_and_plan_renewal_on_original_counter() {
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window("local reading custody CPU diagnostic", 640, 480)
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut output = Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    let mut events = sdl.event_pump().unwrap();
    for _ in 0..4 {
        let _ = output.prepare_input(&mut events, 256).unwrap();
    }
    let input = output.prepared_input().unwrap();
    let (checked, image) = selected_input(32 * 1024 * 1024);
    let mut host = StdHost::from_native_mask_input_diagnostic(
        config(),
        &image,
        &conduit_workspace_make::catalog(),
        &checked,
        CheckedNativeMaskInitialization::from_configuration(&checked).unwrap(),
        &mut output,
        input.clone(),
    )
    .unwrap();
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    let mut cursor = host.prepare_selected_native_reading_state().unwrap();
    let fixed = SelectedNativeReadingState::owned_backing_bytes();
    assert_eq!(observed.load(Ordering::Acquire), baseline + fixed);
    let foreign = StdHost::new();
    assert!(foreign
        .capture_selected_native_reading_state(&mut cursor)
        .is_err());
    let (encounter, route, face) = super::super::encounter_fixture::reading_encounter(
        &host.advertisement,
        1,
        "fixture/reading",
    );
    host.render_selected_interactive_native_mask_with_reading_state(
        &encounter,
        &route,
        &face,
        1,
        "front/native-mask",
        &mut output,
        &mut cursor,
    )
    .unwrap();
    for sequence in 1..=2 {
        host.selected_native_mask_key(
            &encounter,
            &route,
            &input,
            conduit_human::KeyEvent::new(
                78,
                conduit_human::KeyTransition::Pressed,
                conduit_human::KeyModifiers::NONE,
            )
            .unwrap(),
            sequence,
            &mut output,
        )
        .unwrap();
    }
    assert_eq!(page(&host), 2);
    host.capture_selected_native_reading_state(&mut cursor)
        .unwrap();
    let original = cursor.position.unwrap();
    retire(&mut host);
    host.render_selected_interactive_native_mask_with_reading_state(
        &encounter,
        &route,
        &face,
        2,
        "front/native-mask",
        &mut output,
        &mut cursor,
    )
    .unwrap();
    assert_eq!(page(&host), 2, "fresh Scene restores before painting");
    host.capture_selected_native_reading_state(&mut cursor)
        .unwrap();
    retire(&mut host);
    host.retire_selected_native_mask_plan().unwrap();
    assert_eq!(observed.load(Ordering::Acquire), baseline + fixed);
    let (renewed, renewed_route, _) = super::super::encounter_fixture::reading_encounter(
        &host.advertisement,
        1,
        "fixture/renewed",
    );
    host.render_selected_interactive_native_mask_with_reading_state(
        &renewed,
        &renewed_route,
        &face,
        3,
        "front/native-mask",
        &mut output,
        &mut cursor,
    )
    .unwrap();
    assert_eq!(
        page(&host),
        2,
        "finite Plan renewal retains only local reading position"
    );
    let mut changed_layout =
        conduitos::native_face_scene::NativeFaceScene::prepare(face.clone(), 320, 240).unwrap();
    assert!(!changed_layout.restore_reading_position(original));
    retire(&mut host);
    host.retire_selected_native_mask_plan().unwrap();
    let (changed, changed_route, changed_face) = super::super::encounter_fixture::reading_encounter(
        &host.advertisement,
        2,
        "fixture/changed",
    );
    host.render_selected_interactive_native_mask_with_reading_state(
        &changed,
        &changed_route,
        &changed_face,
        4,
        "front/native-mask",
        &mut output,
        &mut cursor,
    )
    .unwrap();
    assert_eq!(page(&host), 0, "changed exact Face refuses old cursor");
    retire(&mut host);
    host.retire_selected_native_mask_plan().unwrap();
    assert_eq!(observed.load(Ordering::Acquire), baseline + fixed);
    drop(cursor);
    assert_eq!(observed.load(Ordering::Acquire), baseline);
    drop(host);
    assert_eq!(observed.load(Ordering::Acquire), 0);
}
