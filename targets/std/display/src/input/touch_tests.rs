//! Real SDL raw queue fixtures preserve windowID that sdl2's Finger enum drops.
//! Dummy SDL tests establish CPU input custody, never physical touchscreen proof.
use super::*;
use sdl2::sys::{SDL_Event, SDL_EventType, SDL_TouchFingerEvent};
fn push(window: u32, kind: SDL_EventType, id: (i64, i64), position: (f32, f32)) {
    // SAFETY: SDL is initialized on this test thread. Initialize the complete
    // queue union and then its exact tagged member, including actual windowID.
    let mut raw: SDL_Event = unsafe { core::mem::zeroed() };
    raw.tfinger = SDL_TouchFingerEvent {
        type_: kind as u32,
        timestamp: 1,
        touchId: id.0,
        fingerId: id.1,
        x: position.0,
        y: position.1,
        dx: 0.0,
        dy: 0.0,
        pressure: 0.5,
        windowID: window,
    };
    assert_eq!(unsafe { sdl2::sys::SDL_PushEvent(&mut raw) }, 1);
}
fn sample(
    output: &mut Scanout<'_>,
    events: &mut EventPump,
) -> conduit_semantic_catalog::NormalizedPointerSample {
    for _ in 0..4 {
        match output.poll_input(events, 64).unwrap() {
            NativeInput::Pointer(sample) => return sample,
            NativeInput::Idle | NativeInput::Pending => {}
            other => panic!("unexpected queue occurrence {other:?}"),
        }
    }
    panic!("no pointer sample within finite queue allowance")
}
fn settle(output: &mut Scanout<'_>, events: &mut EventPump) {
    for _ in 0..4 {
        if output.prepare_input(events, 256).unwrap() == NativeInput::Idle {
            assert!(output.prepared_input().unwrap().is_live());
            return;
        }
    }
    panic!("SDL preparation did not settle within finite allowance")
}
#[test]
fn raw_queued_touch_preserves_window_primary_contact_and_finite_values() {
    let _serial = crate::SDL_TEST_LOCK.lock().unwrap();
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video.window("owned touch", 640, 480).build().unwrap();
    let own = window.id();
    let foreign = video.window("foreign touch", 640, 480).build().unwrap();
    let other = foreign.id();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut output = Scanout::prepare(&mut canvas, &textures, 640, 480).unwrap();
    let mut events = sdl.event_pump().unwrap();
    settle(&mut output, &mut events);
    push(other, SDL_EventType::SDL_FINGERDOWN, (7, 9), (0.1, 0.1));
    push(0, SDL_EventType::SDL_FINGERDOWN, (7, 8), (0.1, 0.1));
    push(own, SDL_EventType::SDL_FINGERDOWN, (7, 1), (0.25, 0.75));
    let down = sample(&mut output, &mut events);
    assert_eq!(
        (
            down.position_x,
            down.position_y,
            down.sequence,
            down.primary_pressed
        ),
        (250_000, 750_000, 1, true)
    );
    push(own, SDL_EventType::SDL_FINGERDOWN, (7, 2), (0.9, 0.9));
    push(own, SDL_EventType::SDL_FINGERUP, (7, 2), (0.9, 0.9));
    push(own, SDL_EventType::SDL_FINGERDOWN, (7, 1), (0.25, 0.75));
    push(own, SDL_EventType::SDL_FINGERMOTION, (7, 1), (0.5, 0.5));
    let motion = sample(&mut output, &mut events);
    assert_eq!(
        (motion.sequence, motion.delta_x, motion.delta_y),
        (2, 250_000, -250_000)
    );
    push(own, SDL_EventType::SDL_FINGERUP, (7, 1), (1.0, 0.0));
    let up = sample(&mut output, &mut events);
    assert_eq!((up.sequence, up.primary_pressed), (3, false));
    sdl.event()
        .unwrap()
        .push_event(Event::MouseButtonDown {
            timestamp: 1,
            window_id: own,
            which: u32::MAX,
            mouse_btn: sdl2::mouse::MouseButton::Left,
            clicks: 1,
            x: 10,
            y: 10,
        })
        .unwrap();
    assert!(
        matches!(
            output.poll_input(&mut events, 64).unwrap(),
            NativeInput::Idle | NativeInput::Pending
        ),
        "synthetic mouse cannot duplicate touch"
    );
    push(own, SDL_EventType::SDL_FINGERDOWN, (7, 3), (f32::NAN, 0.5));
    assert_eq!(
        output.poll_input(&mut events, 64),
        Err(NativeInputRefusal::InvalidPointer)
    );
    push(own, SDL_EventType::SDL_FINGERMOTION, (7, 3), (0.5, 0.5));
    push(own, SDL_EventType::SDL_FINGERDOWN, (7, 3), (0.5, 0.5));
    let fresh = sample(&mut output, &mut events);
    assert_eq!(fresh.sequence, 4);
    assert!(fresh.primary_pressed);
}
#[test]
fn queued_preparation_touch_requires_original_release_before_fresh_down() {
    let _serial = crate::SDL_TEST_LOCK.lock().unwrap();
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video.window("held touch", 320, 240).build().unwrap();
    let own = window.id();
    let mut canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    let mut output = Scanout::prepare(&mut canvas, &textures, 320, 240).unwrap();
    let mut events = sdl.event_pump().unwrap();
    push(own, SDL_EventType::SDL_FINGERDOWN, (8, 1), (0.3, 0.4));
    settle(&mut output, &mut events);
    push(own, SDL_EventType::SDL_FINGERMOTION, (8, 1), (0.4, 0.5));
    push(own, SDL_EventType::SDL_FINGERDOWN, (8, 2), (0.7, 0.8));
    push(own, SDL_EventType::SDL_FINGERUP, (8, 1), (0.4, 0.5));
    let released = sample(&mut output, &mut events);
    assert!(!released.primary_pressed);
    assert_eq!(released.sequence, 1);
    push(own, SDL_EventType::SDL_FINGERUP, (8, 2), (0.7, 0.8));
    push(own, SDL_EventType::SDL_FINGERDOWN, (8, 2), (0.7, 0.8));
    let down = sample(&mut output, &mut events);
    assert!(down.primary_pressed);
    assert_eq!(down.sequence, 2);
}
