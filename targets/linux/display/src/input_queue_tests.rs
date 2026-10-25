//! Actual SDL diagnostic queue, injected observations; no physical keyboard claim.
use crate::{NativeInput, NativeInputRefusal, Scanout};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::{Mod, Scancode},
};

pub(crate) fn live_events(scanout: &mut Scanout<'_>, sdl: &sdl2::Sdl) {
    let mut pump = sdl.event_pump().unwrap();
    pump.poll_iter().for_each(drop);
    let queue = sdl.event().unwrap();
    let window_id = scanout.initialized_display().unwrap().window_id();
    let down = |window_id, repeat| Event::KeyDown {
        timestamp: 0,
        window_id,
        keycode: None,
        scancode: Some(Scancode::A),
        keymod: Mod::LSHIFTMOD | Mod::RCTRLMOD,
        repeat,
    };
    assert_eq!(
        scanout.poll_input(&mut pump, 1),
        Err(NativeInputRefusal::NotPrepared)
    );
    // This key predates input preparation and must never be delivered.
    queue.push_event(down(window_id, false)).unwrap();
    let mut prepared = false;
    for _ in 0..8 {
        match scanout.prepare_input(&mut pump, 1).unwrap() {
            NativeInput::Idle => {
                prepared = true;
                break;
            }
            NativeInput::Pending => {}
            other => panic!("stale input escaped preparation: {other:?}"),
        }
    }
    assert!(
        prepared,
        "bounded queue preparation reaches actual empty queue"
    );
    queue
        .push_event(down(window_id.wrapping_add(1), false))
        .unwrap();
    queue.push_event(down(window_id, false)).unwrap();
    assert_eq!(
        scanout.poll_input(&mut pump, 0),
        Err(NativeInputRefusal::InvalidBound)
    );
    assert_eq!(
        scanout.poll_input(&mut pump, 257),
        Err(NativeInputRefusal::InvalidBound)
    );
    assert!(matches!(
        scanout.poll_input(&mut pump, 1),
        Ok(NativeInput::Idle | NativeInput::Pending)
    ));
    let expected = KeyEvent::new(
        4,
        KeyTransition::Pressed,
        KeyModifiers::from_bits(
            KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_CONTROL.bits(),
        ),
    )
    .unwrap();
    assert_eq!(queued(scanout, &mut pump), Ok(NativeInput::Key(expected)));
    queue.push_event(down(window_id, true)).unwrap();
    for _ in 0..4 {
        assert!(matches!(
            scanout.poll_input(&mut pump, 1),
            Ok(NativeInput::Idle | NativeInput::Pending)
        ));
    }
    queue
        .push_event(Event::KeyUp {
            timestamp: 0,
            window_id,
            keycode: None,
            scancode: Some(Scancode::A),
            keymod: Mod::NOMOD,
            repeat: false,
        })
        .unwrap();
    assert_eq!(
        queued(scanout, &mut pump),
        Ok(NativeInput::Key(
            KeyEvent::new(4, KeyTransition::Released, KeyModifiers::NONE).unwrap()
        ))
    );
    queue
        .push_event(Event::KeyDown {
            timestamp: 0,
            window_id,
            keycode: None,
            scancode: None,
            keymod: Mod::NOMOD,
            repeat: false,
        })
        .unwrap();
    assert_eq!(
        queued(scanout, &mut pump),
        Err(NativeInputRefusal::InvalidKey { scancode: None })
    );
    assert!(scanout.initialized_display().is_ok());
}
pub(crate) fn focus_loss(scanout: &mut Scanout<'_>, sdl: &sdl2::Sdl) {
    let token = scanout.initialized_display().unwrap();
    let mut pump = sdl.event_pump().unwrap();
    pump.poll_iter().for_each(drop);
    sdl.event()
        .unwrap()
        .push_event(Event::Window {
            timestamp: 0,
            window_id: token.window_id(),
            win_event: WindowEvent::FocusLost,
        })
        .unwrap();
    assert_eq!(
        scanout.prepare_input(&mut pump, 8),
        Err(NativeInputRefusal::Lost)
    );
    assert!(!token.is_live());
    assert_eq!(
        scanout.poll_input(&mut pump, 1),
        Err(NativeInputRefusal::Lost)
    );
}

fn queued(
    scanout: &mut Scanout<'_>,
    pump: &mut sdl2::EventPump,
) -> Result<NativeInput, NativeInputRefusal> {
    for _ in 0..4 {
        match scanout.poll_input(pump, 256) {
            Ok(NativeInput::Idle | NativeInput::Pending) => {}
            result => return result,
        }
    }
    panic!("injected observation absent after finite queue inspections")
}
