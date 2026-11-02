//! Finite acquisition of actual SDL window input; semantic routing stays in Mask.
use crate::Scanout;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use conduitos::product_bases::InitializedNativeOutput;
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::{Mod, Scancode},
    EventPump,
};

#[derive(Debug, PartialEq, Eq)]
pub enum NativeInput {
    Idle,
    /// The finite inspection allowance ended; queued events remain untouched.
    Pending,
    Key(KeyEvent),
    Quit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInputRefusal {
    Lost,
    InvalidBound,
    NotPrepared,
    PreparationUnavailable,
    InvalidKey { scancode: Option<i32> },
}
impl Scanout<'_> {
    /// Before Play, discard queued input from earlier window lifetimes. Each
    /// call inspects at most the supplied allowance. Readiness requires an
    /// actual empty SDL queue, not merely the end of a poll cycle. Preparation
    /// may allocate one bounded peek slot; poll_input never performs that peek.
    pub fn prepare_input(
        &mut self,
        events: &mut EventPump,
        maximum_events: u16,
    ) -> Result<NativeInput, NativeInputRefusal> {
        self.require_live().map_err(|_| NativeInputRefusal::Lost)?;
        if maximum_events == 0 || maximum_events > 256 {
            return Err(NativeInputRefusal::InvalidBound);
        }
        if self.input_ready {
            return Ok(NativeInput::Idle);
        }
        let queue = self
            .canvas
            .window()
            .subsystem()
            .sdl()
            .event()
            .map_err(|_| NativeInputRefusal::PreparationUnavailable)?;
        for _ in 0..maximum_events {
            match events.poll_event() {
                Some(Event::KeyDown { .. } | Event::KeyUp { .. }) => {}
                Some(event) => {
                    if let Some(input) = self.translate_input(event)? {
                        return Ok(input);
                    }
                }
                None => {
                    let pending: Vec<Event> = queue.peek_events(1);
                    if pending.is_empty() {
                        self.input_ready = true;
                        return Ok(NativeInput::Idle);
                    }
                }
            }
        }
        Ok(NativeInput::Pending)
    }
    /// Inspect at most `maximum_events` actual queue entries without waiting.
    /// Only this live window's keys leave the driver. A key is an observation,
    /// not an action: the selected Mask must check its acknowledged Show/focus.
    /// Repeat notifications do not synthesize additional physical transitions.
    /// Focus/output loss is terminal and requires fresh output preparation.
    pub fn poll_input(
        &mut self,
        events: &mut EventPump,
        maximum_events: u16,
    ) -> Result<NativeInput, NativeInputRefusal> {
        self.require_live().map_err(|_| NativeInputRefusal::Lost)?;
        if maximum_events == 0 || maximum_events > 256 {
            return Err(NativeInputRefusal::InvalidBound);
        }
        if !self.input_ready {
            return Err(NativeInputRefusal::NotPrepared);
        }
        for _ in 0..maximum_events {
            let Some(event) = events.poll_event() else {
                return Ok(NativeInput::Idle);
            };
            if let Some(input) = self.translate_input(event)? {
                return Ok(input);
            }
        }
        Ok(NativeInput::Pending)
    }
    fn translate_input(&mut self, event: Event) -> Result<Option<NativeInput>, NativeInputRefusal> {
        let own_window = self.initialized.window_id();
        match event {
            Event::Quit { .. } => {
                self.lose();
                Ok(Some(NativeInput::Quit))
            }
            Event::RenderTargetsReset { .. } | Event::RenderDeviceReset { .. } => self.input_lost(),
            Event::Window {
                window_id,
                win_event,
                ..
            } if window_id == own_window => match win_event {
                WindowEvent::Close => {
                    self.lose();
                    Ok(Some(NativeInput::Quit))
                }
                WindowEvent::FocusLost
                | WindowEvent::Hidden
                | WindowEvent::Minimized
                | WindowEvent::DisplayChanged(_) => self.input_lost(),
                WindowEvent::Resized(w, h) | WindowEvent::SizeChanged(w, h)
                    if (u32::try_from(w).ok(), u32::try_from(h).ok())
                        != (
                            Some(self.initialized.window_size().0),
                            Some(self.initialized.window_size().1),
                        ) =>
                {
                    self.input_lost()
                }
                _ => Ok(None),
            },
            Event::KeyDown {
                window_id,
                scancode,
                keymod,
                repeat: false,
                ..
            } if window_id == own_window => key(scancode, keymod, KeyTransition::Pressed).map(Some),
            Event::KeyUp {
                window_id,
                scancode,
                keymod,
                repeat: false,
                ..
            } if window_id == own_window => {
                key(scancode, keymod, KeyTransition::Released).map(Some)
            }
            _ => Ok(None),
        }
    }
    fn input_lost(&mut self) -> Result<Option<NativeInput>, NativeInputRefusal> {
        self.lose();
        Err(NativeInputRefusal::Lost)
    }
}
fn key(
    scancode: Option<Scancode>,
    modifiers: Mod,
    transition: KeyTransition,
) -> Result<NativeInput, NativeInputRefusal> {
    let invalid = NativeInputRefusal::InvalidKey {
        scancode: scancode.map(|s| s as i32),
    };
    let usage = scancode
        .and_then(|s| u8::try_from(s as i32).ok())
        .ok_or(invalid)?;
    let mut bits = 0;
    for (sdl, human) in [
        (Mod::LCTRLMOD, KeyModifiers::LEFT_CONTROL),
        (Mod::LSHIFTMOD, KeyModifiers::LEFT_SHIFT),
        (Mod::LALTMOD, KeyModifiers::LEFT_ALT),
        (Mod::LGUIMOD, KeyModifiers::LEFT_GUI),
        (Mod::RCTRLMOD, KeyModifiers::RIGHT_CONTROL),
        (Mod::RSHIFTMOD, KeyModifiers::RIGHT_SHIFT),
        (Mod::RALTMOD, KeyModifiers::RIGHT_ALT),
        (Mod::RGUIMOD, KeyModifiers::RIGHT_GUI),
    ] {
        if modifiers.contains(sdl) {
            bits |= human.bits();
        }
    }
    KeyEvent::new(usage, transition, KeyModifiers::from_bits(bits))
        .map(NativeInput::Key)
        .map_err(|_| invalid)
}
#[cfg(test)]
mod tests;
