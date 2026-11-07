//! Actual SDL dummy queue and window flags; no physical KMSDRM proof.
use crate::{NativeInput, NativeInputRefusal, Scanout};
use sdl2::{
    event::{Event, WindowEvent},
    render::{Canvas, TextureCreator},
    video::{Window, WindowContext},
};
fn setup() -> (sdl2::Sdl, Canvas<Window>, TextureCreator<WindowContext>) {
    assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window("startup settlement diagnostic", 32, 24)
        .build()
        .unwrap();
    let canvas = window.into_canvas().software().build().unwrap();
    let textures = canvas.texture_creator();
    (sdl, canvas, textures)
}
fn event(window_id: u32, win_event: WindowEvent) -> Event {
    Event::Window {
        timestamp: 0,
        window_id,
        win_event,
    }
}
#[test]
fn startup_settlement_keeps_post_ready_input_loss_terminal() {
    assert_startup_settlement_and_post_ready_loss(false);
}
#[test]
fn startup_settlement_keeps_post_ready_output_loss_terminal() {
    assert_startup_settlement_and_post_ready_loss(true);
}
fn assert_startup_settlement_and_post_ready_loss(output_only: bool) {
    let _sdl_test = crate::SDL_TEST_LOCK.lock().unwrap();
    let (sdl, mut canvas, textures) = setup();
    let window = canvas.window().id();
    let queue = sdl.event().unwrap();
    for e in [
        event(window, WindowEvent::Hidden),
        event(window, WindowEvent::Shown),
        event(window.wrapping_add(1), WindowEvent::Close),
        event(window.wrapping_add(1), WindowEvent::Hidden),
    ] {
        queue.push_event(e).unwrap();
    }
    let mut output = Scanout::prepare(&mut canvas, &textures, 32, 24).unwrap();
    let token = output.initialized_display().unwrap();
    assert!(token.is_live());
    let mut events = sdl.event_pump().unwrap();
    assert_eq!(
        output.poll_input(&mut events, 1),
        Err(NativeInputRefusal::NotPrepared)
    );
    if !output_only {
        let mut input_prepared = false;
        for _ in 0..4 {
            match output.prepare_input(&mut events, 256).unwrap() {
                NativeInput::Idle => {
                    input_prepared = true;
                    break;
                }
                NativeInput::Pending => (),
                other => panic!("unexpected preparation input: {other:?}"),
            }
        }
        assert!(input_prepared);
    }
    queue
        .push_event(event(window, WindowEvent::Hidden))
        .unwrap();
    let mut lost = false;
    for _ in 0..4 {
        let observed = if output_only {
            output.poll_output_lifecycle(&mut events, 256)
        } else {
            output.poll_input(&mut events, 256)
        };
        if observed == Err(NativeInputRefusal::Lost) {
            lost = true;
            break;
        }
    }
    assert!(lost);
    assert!(!token.is_live());
    queue.push_event(event(window, WindowEvent::Shown)).unwrap();
    assert_eq!(
        if output_only {
            output.poll_output_lifecycle(&mut events, 256)
        } else {
            output.poll_input(&mut events, 256)
        },
        Err(NativeInputRefusal::Lost)
    );
}
#[test]
fn startup_quit_own_close_and_global_render_reset_refuse_before_token() {
    let _sdl_test = crate::SDL_TEST_LOCK.lock().unwrap();
    for kind in 0..2 {
        let (sdl, mut canvas, textures) = setup();
        sdl.event()
            .unwrap()
            .push_event(match kind {
                0 => Event::Quit { timestamp: 0 },
                1 => event(canvas.window().id(), WindowEvent::Close),
                _ => unreachable!(),
            })
            .unwrap();
        let error = Scanout::prepare(&mut canvas, &textures, 32, 24)
            .err()
            .unwrap();
        assert_eq!(
            error,
            match kind {
                0 => "display-preparation-quit",
                1 => "display-preparation-window-close",
                _ => "display-preparation-render-reset",
            }
        );
    }
}
#[test]
fn both_global_reset_variants_refuse_the_same_preparation_guard() {
    // SDL2's safe push_event cannot inject these two SDL-generated event types.
    // Exercise the exact guard used for each polled event without raw SDL FFI.
    for event in [
        Event::RenderDeviceReset { timestamp: 0 },
        Event::RenderTargetsReset { timestamp: 0 },
    ] {
        assert_eq!(
            super::validate_startup_event(event, 1),
            Err("display-preparation-render-reset".into())
        );
    }
}
#[test]
fn preparation_queue_pressure_refuses_instead_of_issuing_a_token() {
    let _sdl_test = crate::SDL_TEST_LOCK.lock().unwrap();
    let (sdl, mut canvas, textures) = setup();
    let queue = sdl.event().unwrap();
    for _ in 0..512 {
        queue
            .push_event(event(canvas.window().id(), WindowEvent::Exposed))
            .unwrap();
    }
    assert_eq!(
        Scanout::prepare(&mut canvas, &textures, 32, 24)
            .err()
            .unwrap(),
        "display-preparation-event-pressure"
    );
    let remaining: Vec<Event> = queue.peek_events(1);
    assert!(!remaining.is_empty());
}
#[test]
fn actual_hidden_window_and_wrong_geometry_never_issue_readiness() {
    let _sdl_test = crate::SDL_TEST_LOCK.lock().unwrap();
    let (_sdl, mut canvas, textures) = setup();
    canvas.window_mut().hide();
    assert_eq!(
        Scanout::prepare(&mut canvas, &textures, 32, 24)
            .err()
            .unwrap(),
        "display-preparation-window-not-shown"
    );
    canvas.window_mut().show();
    assert_eq!(
        Scanout::prepare(&mut canvas, &textures, 32, 23)
            .err()
            .unwrap(),
        "display-preparation-window-geometry"
    );
}
