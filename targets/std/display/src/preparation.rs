//! Settle historical startup notifications before any live output token exists.
//! This does not prepare keyboard input or suppress loss of an issued token.
use sdl2::{
    event::{Event, WindowEvent},
    render::Canvas,
    video::Window,
};
const MAXIMUM_PREPARATION_POLLS: usize = 256;

pub(super) fn settle_output(
    canvas: &Canvas<Window>,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let sdl = canvas.window().subsystem().sdl();
    let mut events = sdl
        .event_pump()
        .map_err(|e| format!("display-preparation-event-pump:{e}"))?;
    let queue = sdl
        .event()
        .map_err(|e| format!("display-preparation-event-queue:{e}"))?;
    let own_window = canvas.window().id();
    for _ in 0..MAXIMUM_PREPARATION_POLLS {
        match events.poll_event() {
            Some(event) => validate_startup_event(event, own_window)?,
            None => {
                // SDL can end a poll cycle while real entries remain queued.
                // This one-slot preparation peek never grows with queue size.
                events.pump_events();
                let pending: Vec<Event> = queue.peek_events(1);
                if pending.is_empty() {
                    return verify_window(canvas, width, height);
                }
            }
        }
    }
    Err("display-preparation-event-pressure".into())
}
fn validate_startup_event(event: Event, own_window: u32) -> Result<(), String> {
    match event {
        Event::Quit { .. } => Err("display-preparation-quit".into()),
        Event::RenderTargetsReset { .. } | Event::RenderDeviceReset { .. } => {
            Err("display-preparation-render-reset".into())
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::Close,
            ..
        } if window_id == own_window => Err("display-preparation-window-close".into()),
        // No issued lifetime exists yet: renderer construction can queue old
        // Hidden/Shown/focus/size transitions. Final actual state is checked
        // after settlement. Foreign window notifications are not our loss.
        _ => Ok(()),
    }
}

fn verify_window(canvas: &Canvas<Window>, width: u32, height: u32) -> Result<(), String> {
    let window = canvas.window();
    let flags = window.window_flags();
    let shown = sdl2::sys::SDL_WindowFlags::SDL_WINDOW_SHOWN as u32;
    let hidden = sdl2::sys::SDL_WindowFlags::SDL_WINDOW_HIDDEN as u32;
    if flags & shown == 0 || flags & hidden != 0 || window.is_minimized() {
        return Err("display-preparation-window-not-shown".into());
    }
    if window.size() != (width, height)
        || canvas
            .output_size()
            .map_err(|e| format!("display-preparation-output-size:{e}"))?
            != (width, height)
    {
        return Err("display-preparation-window-geometry".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
