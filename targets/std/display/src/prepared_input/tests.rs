//! Actual SDL queue fixture; no physical USB/seat acceptance claim.
use super::*;
use crate::{NativeInput, NativeInputRefusal, Scanout};
use sdl2::{event::Event, mouse::MouseButton};

#[test]
fn settled_pointer_and_keyboard_share_exclusive_revocable_input_owner() {
    let _serial = crate::SDL_TEST_LOCK.lock().unwrap();
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window("prepared input fixture", 32, 24)
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut scanout = Scanout::prepare(&mut canvas, &textures, 32, 24).unwrap();
    let display = scanout.initialized_display().unwrap();
    assert!(matches!(
        scanout.prepared_input(),
        Err(NativeInputRefusal::NotPrepared)
    ));
    let mut events = sdl.event_pump().unwrap();
    let queue = sdl.event().unwrap();
    let down = |window_id| Event::MouseButtonDown {
        timestamp: 1,
        window_id,
        which: 0,
        mouse_btn: MouseButton::Left,
        clicks: 1,
        x: 31,
        y: 23,
    };
    queue.push_event(down(display.window_id())).unwrap();
    for _ in 0..32 {
        match scanout.prepare_input(&mut events, 4).unwrap() {
            NativeInput::Idle => break,
            NativeInput::Pending => (),
            other => panic!("historical input escaped: {other:?}"),
        }
    }
    let input = scanout.prepared_input().unwrap();
    assert!(input.matches_display(&display));
    let claim = input.claim_host().unwrap();
    assert!(matches!(
        input.claim_host(),
        Err(NativeInputClaimRefusal::AlreadyClaimed)
    ));
    queue
        .push_event(down(display.window_id().wrapping_add(1)))
        .unwrap();
    queue.push_event(down(display.window_id())).unwrap();
    let mut observed = None;
    for _ in 0..32 {
        match scanout.poll_input(&mut events, 1).unwrap() {
            NativeInput::Idle | NativeInput::Pending => (),
            NativeInput::Pointer(value) => {
                observed = Some(value);
                break;
            }
            other => panic!("unexpected input: {other:?}"),
        }
    }
    let sample = observed.expect("actual own-window pointer event");
    assert_eq!(
        (sample.position_x, sample.position_y, sample.sequence),
        (1_000_000, 1_000_000, 1)
    );
    assert!(sample.primary_pressed);
    // Keyboard and pointer use the same prepared window and Host claim.
    // SDL scancodes retain the existing USB HID semantics, including modifiers.
    queue
        .push_event(Event::KeyDown {
            timestamp: 2,
            window_id: display.window_id(),
            keycode: None,
            scancode: Some(sdl2::keyboard::Scancode::Tab),
            keymod: sdl2::keyboard::Mod::LSHIFTMOD,
            repeat: false,
        })
        .unwrap();
    let mut key = None;
    for _ in 0..32 {
        match scanout.poll_input(&mut events, 1).unwrap() {
            NativeInput::Idle | NativeInput::Pending => (),
            NativeInput::Key(event) => {
                key = Some(event);
                break;
            }
            other => panic!("unexpected input: {other:?}"),
        }
    }
    assert_eq!(
        key,
        Some(
            conduit_human::KeyEvent::new(
                43,
                conduit_human::KeyTransition::Pressed,
                conduit_human::KeyModifiers::LEFT_SHIFT,
            )
            .unwrap()
        )
    );
    drop(claim);
    assert!(
        display.is_live(),
        "input retirement must preserve displayed pixels"
    );
    assert!(!input.is_live());
    assert!(matches!(
        input.claim_host(),
        Err(NativeInputClaimRefusal::Lost)
    ));
    assert!(matches!(
        scanout.poll_input(&mut events, 1),
        Err(NativeInputRefusal::Lost)
    ));
    for _ in 0..32 {
        if scanout.prepare_input(&mut events, 4).unwrap() == NativeInput::Idle {
            break;
        }
    }
    let replacement = scanout.prepared_input().unwrap();
    assert!(!replacement.same_instance(&input));
    scanout.lose();
    assert!(!replacement.is_live());
    assert!(!display.is_live());
}
