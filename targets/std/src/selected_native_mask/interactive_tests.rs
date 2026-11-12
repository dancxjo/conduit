//! CPU-only integration through SDL's actual dummy queue and admitted Mask.
//! This supplies neither physical USB evidence nor owner execution authority.
use super::*;
use crate::selected_native_mask::SelectedNativeMaskInput;
use conduit_linux_display::NativeInput;
use sdl2::{
    event::Event,
    keyboard::{Mod, Scancode},
    mouse::MouseButton,
};
use std::sync::atomic::Ordering;

fn next_input(output: &mut Scanout<'_>, events: &mut sdl2::EventPump) -> NativeInput {
    // Rendering can enqueue unrelated SDL lifecycle entries. Inspection is
    // finite: at most four batches of 32, never a fabricated key/pointer.
    for _ in 0..4 {
        match output.poll_input(events, 32).unwrap() {
            NativeInput::Idle | NativeInput::Pending => continue,
            input => return input,
        }
    }
    panic!("injected SDL occurrence did not arrive within finite inspection bound")
}
fn key_event(window_id: u32, scan: Scancode) -> Event {
    Event::KeyDown {
        timestamp: 1,
        window_id,
        keycode: None,
        scancode: Some(scan),
        keymod: Mod::NOMOD,
        repeat: false,
    }
}
#[test]
fn actual_sdl_keyboard_pointer_submit_original_show_and_retire_original_custody() {
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window("interactive admitted Mask CPU diagnostic", 640, 480)
        .build()
        .unwrap();
    let window_id = window.id();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut output = Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    let mut events = sdl.event_pump().unwrap();
    let queue = sdl.event().unwrap();
    let mut settled = false;
    for _ in 0..4 {
        if output.prepare_input(&mut events, 256).unwrap() == NativeInput::Idle {
            settled = true;
            break;
        }
    }
    assert!(settled, "finite dummy SDL queue must actually settle");
    let input = output.prepared_input().unwrap();
    let (checked, image) = selected_input(32 * 1024 * 1024);
    let catalog = conduit_workspace_make::catalog();
    let mut host = StdHost::from_native_mask_input_diagnostic(
        config(),
        &image,
        &catalog,
        &checked,
        CheckedNativeMaskInitialization::from_configuration(&checked).unwrap(),
        &mut output,
        input.clone(),
    )
    .unwrap();
    assert!(host.selected_native_mask_supports_input());
    let observed = host.kernel_resources.image_memory_observer();
    let original_offer = host.advertisement.clone();
    let (encounter, route, face) =
        super::super::encounter_fixture::action_encounter(&original_offer, 1, "fixture/input");
    let (stale, stale_route, _) =
        super::super::encounter_fixture::action_encounter(&original_offer, 99, "fixture/input");
    let mut first_retired = None;
    for (phase, pointer) in [false, true].into_iter().enumerate() {
        host.render_selected_interactive_native_mask(
            &encounter,
            &route,
            &face,
            phase as u64 + 1,
            "front/native-mask",
            &mut output,
        )
        .unwrap();
        let show = host.selected_native_mask_show().unwrap().clone();
        // Inspection must not invoke the explicit retiring evidence handoff.
        assert_eq!(host.selected_native_mask_show(), Some(&show));
        assert!(
            host.selected_native_mask_key(
                &stale,
                &stale_route,
                &input,
                conduit_human::KeyEvent::new(
                    43,
                    conduit_human::KeyTransition::Pressed,
                    conduit_human::KeyModifiers::NONE
                )
                .unwrap(),
                100,
                &mut output
            )
            .is_err(),
            "stale route cannot authorize even focus changes"
        );
        let result = if pointer {
            // Geometry comes from the same semantic Scene, never an action ID
            // inferred from a pixel or bypassed into a forged interaction.
            let scene =
                conduitos::native_face_scene::NativeFaceScene::prepare(face.clone(), 640, 480)
                    .unwrap();
            let frame = scene.frame().unwrap();
            let hit = frame
                .hits
                .iter()
                .find(|hit| hit.action.identity == "fixture/activate" && hit.argument.is_none())
                .unwrap();
            let x = i32::from(hit.bounds.x) + i32::from(hit.bounds.width) / 2;
            let y = i32::from(hit.bounds.y) + i32::from(hit.bounds.height) / 2;
            queue
                .push_event(Event::MouseButtonDown {
                    timestamp: 2,
                    window_id,
                    which: 0,
                    mouse_btn: MouseButton::Left,
                    clicks: 1,
                    x,
                    y,
                })
                .unwrap();
            let NativeInput::Pointer(sample) = next_input(&mut output, &mut events) else {
                panic!("SDL pointer sample required")
            };
            host.selected_native_mask_pointer(
                &encounter,
                &route,
                &input,
                sample,
                true,
                3,
                &mut output,
            )
            .unwrap()
        } else {
            queue
                .push_event(key_event(window_id, Scancode::Tab))
                .unwrap();
            let NativeInput::Key(key) = next_input(&mut output, &mut events) else {
                panic!("SDL Tab required")
            };
            assert!(matches!(
                host.selected_native_mask_key(&encounter, &route, &input, key, 1, &mut output)
                    .unwrap(),
                SelectedNativeMaskInput::Redrawn
            ));
            queue
                .push_event(key_event(window_id, Scancode::Return))
                .unwrap();
            let NativeInput::Key(key) = next_input(&mut output, &mut events) else {
                panic!("SDL Enter required")
            };
            host.selected_native_mask_key(&encounter, &route, &input, key, 2, &mut output)
                .unwrap()
        };
        let SelectedNativeMaskInput::Submitted(submitted) = result else {
            panic!("ordinary Mask must mint a submitted interaction")
        };
        assert_eq!(submitted.show(), &show);
        assert_eq!(submitted.correlation().show_id, show.show_id);
        assert_eq!(submitted.correlation().face_id, face.identity);
        assert_eq!(submitted.correlation().face_revision, face.revision);
        assert_eq!(
            submitted.correlation().interaction.action_id,
            "fixture/activate"
        );
        assert!(
            submitted
                .evidence
                .as_ref()
                .unwrap()
                .evidence()
                .receipt
                .scanout_pixels_written
                > 0
        );
        let charged = observed.load(Ordering::Acquire);
        let mut retained = Some(submitted);
        let mut foreign = StdHost::new();
        assert!(foreign
            .retire_selected_native_interaction(&mut retained)
            .is_err());
        assert!(retained.is_some());
        assert_eq!(
            observed.load(Ordering::Acquire),
            charged,
            "foreign retirement preserves original charge"
        );
        host.retire_selected_native_interaction(&mut retained)
            .unwrap();
        assert!(retained.is_none());
        let retired = observed.load(Ordering::Acquire);
        assert!(retired < charged);
        if let Some(previous) = first_retired {
            assert_eq!(
                retired, previous,
                "same prepared Plan recovers the same original per-frame capacity"
            );
        } else {
            first_retired = Some(retired);
        }
    }
    assert_eq!(
        host.advertisement, original_offer,
        "input does not rewrite equipment offers"
    );
    assert!(
        encounter.calls.get() >= 6,
        "each input checks current owner encounter"
    );
}
