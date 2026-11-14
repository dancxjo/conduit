//! Finite acquisition of actual SDL window input; semantic routing stays in Mask.
use crate::prepared_input::TouchBinding;
use crate::Scanout;
pub(super) mod pointer;
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
    Pointer(conduit_semantic_catalog::NormalizedPointerSample),
    Quit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInputRefusal {
    Lost,
    InvalidBound,
    NotPrepared,
    PreparationUnavailable,
    InvalidKey { scancode: Option<i32> },
    InvalidPointer,
}
// SDL2 0.38's high-level Finger events omit raw SDL_TouchFingerEvent.windowID.
// Retain that driver witness before conversion; never infer it from coordinates.
struct QueuedEvent {
    event: Event,
    touch_window: Option<u32>,
}
fn poll_queued(_events: &mut EventPump) -> Option<QueuedEvent> {
    let mut raw = core::mem::MaybeUninit::<sdl2::sys::SDL_Event>::uninit();
    // SAFETY: the exclusive EventPump borrow keeps SDL's single event-pump
    // owner alive on its original thread. This is the same one-event poll used
    // by sdl2::EventPump; SDL initializes the union only on return value 1.
    let raw = unsafe {
        if sdl2::sys::SDL_PollEvent(raw.as_mut_ptr()) != 1 {
            return None;
        }
        raw.assume_init()
    };
    // SAFETY: only the matching SDL finger discriminants read tfinger. Other
    // union members are decoded by sdl2's existing Event::from_ll implementation.
    let touch_window = unsafe {
        match raw.type_ {
            kind if kind == sdl2::sys::SDL_EventType::SDL_FINGERDOWN as u32
                || kind == sdl2::sys::SDL_EventType::SDL_FINGERMOTION as u32
                || kind == sdl2::sys::SDL_EventType::SDL_FINGERUP as u32 =>
            {
                Some(raw.tfinger.windowID)
            }
            _ => None,
        }
    };
    Some(QueuedEvent {
        event: Event::from_ll(raw),
        touch_window,
    })
}
fn touch_id(event: &Event) -> Option<i64> {
    match event {
        Event::FingerDown { touch_id, .. }
        | Event::FingerMotion { touch_id, .. }
        | Event::FingerUp { touch_id, .. } => Some(*touch_id),
        _ => None,
    }
}
fn touch_window_matches(window: Option<u32>, own: u32, global: bool) -> bool {
    window == Some(own) || (window == Some(0) && global)
}
impl Scanout<'_> {
    /// Actual settled input readiness, with no resource or interaction grant.
    pub fn prepared_input(&self) -> Result<crate::PreparedNativeInput, NativeInputRefusal> {
        if !self.input_ready {
            return Err(NativeInputRefusal::NotPrepared);
        }
        self.input_owner
            .as_ref()
            .filter(|input| input.matches_display(&self.initialized))
            .cloned()
            .ok_or(NativeInputRefusal::Lost)
    }

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
            if self
                .input_owner
                .as_ref()
                .is_some_and(|input| input.is_live())
            {
                self.check_touch_inventory()?;
                return Ok(NativeInput::Idle);
            }
            self.input_ready = false;
            self.input_owner = None;
        }
        let touch = TouchBinding::observe(&self.initialized, self.canvas.window())
            .map_err(|_| NativeInputRefusal::PreparationUnavailable)?;
        let queue = self
            .canvas
            .window()
            .subsystem()
            .sdl()
            .event()
            .map_err(|_| NativeInputRefusal::PreparationUnavailable)?;
        for _ in 0..maximum_events {
            let queued = poll_queued(events);
            touch
                .check(&self.initialized, self.canvas.window())
                .map_err(|_| NativeInputRefusal::PreparationUnavailable)?;
            match queued {
                Some(queued)
                    if matches!(
                        queued.event,
                        Event::FingerDown { .. }
                            | Event::FingerMotion { .. }
                            | Event::FingerUp { .. }
                    ) =>
                {
                    let global = touch_id(&queued.event).is_some_and(|id| touch.global_matches(id));
                    if touch_window_matches(
                        queued.touch_window,
                        self.initialized.window_id(),
                        global,
                    ) {
                        self.pointer.discard_touch(&queued.event);
                    }
                }
                Some(QueuedEvent {
                    event:
                        Event::KeyDown { .. }
                        | Event::KeyUp { .. }
                        | Event::MouseMotion { .. }
                        | Event::MouseButtonDown { .. }
                        | Event::MouseButtonUp { .. },
                    ..
                }) => {}
                Some(event) => {
                    if let Some(input) = self.translate_input(event)? {
                        return Ok(input);
                    }
                }
                None => {
                    let pending: Vec<Event> = queue.peek_events(1);
                    if pending.is_empty() {
                        self.pointer
                            .reconcile_held_touch()
                            .map_err(|_| NativeInputRefusal::PreparationUnavailable)?;
                        self.pointer.prepare(events.mouse_state().left());
                        self.input_owner = Some(crate::PreparedNativeInput::prepared(
                            self.initialized.clone(),
                            touch,
                        ));
                        self.input_ready = true;
                        return Ok(NativeInput::Idle);
                    }
                }
            }
        }
        Ok(NativeInput::Pending)
    }
    /// Observe output lifetime without preparing or granting local input.
    /// Keys are discarded; only this preparation's lifecycle events are handled.
    /// Work is bounded even while no Body Play or input provider is active.
    pub fn poll_output_lifecycle(
        &mut self,
        events: &mut EventPump,
        maximum_events: u16,
    ) -> Result<NativeInput, NativeInputRefusal> {
        self.require_live().map_err(|_| NativeInputRefusal::Lost)?;
        if maximum_events == 0 || maximum_events > 256 {
            return Err(NativeInputRefusal::InvalidBound);
        }
        for _ in 0..maximum_events {
            let Some(event) = poll_queued(events) else {
                return Ok(NativeInput::Idle);
            };
            match event.event {
                Event::KeyDown { .. }
                | Event::KeyUp { .. }
                | Event::MouseMotion { .. }
                | Event::MouseButtonDown { .. }
                | Event::MouseButtonUp { .. }
                | Event::FingerDown { .. }
                | Event::FingerUp { .. }
                | Event::FingerMotion { .. } => {}
                _ => {
                    if let Some(input) = self.translate_input(event)? {
                        return Ok(input);
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
        if !self
            .input_owner
            .as_ref()
            .is_some_and(|input| input.matches_display(&self.initialized))
        {
            return Err(NativeInputRefusal::Lost);
        }
        self.check_touch_inventory()?;
        for _ in 0..maximum_events {
            let queued = poll_queued(events);
            self.check_touch_inventory()?;
            let Some(event) = queued else {
                return Ok(NativeInput::Idle);
            };
            if let Some(input) = self.translate_input(event)? {
                return Ok(input);
            }
        }
        Ok(NativeInput::Pending)
    }
    fn check_touch_inventory(&mut self) -> Result<(), NativeInputRefusal> {
        if self
            .input_owner
            .as_ref()
            .is_none_or(|input| input.check_touch(self.canvas.window()).is_err())
        {
            if let Some(input) = &self.input_owner {
                input.revoke();
            }
            self.input_ready = false;
            return Err(NativeInputRefusal::Lost);
        }
        Ok(())
    }
    fn translate_input(
        &mut self,
        queued: QueuedEvent,
    ) -> Result<Option<NativeInput>, NativeInputRefusal> {
        let own_window = self.initialized.window_id();
        let QueuedEvent {
            event,
            touch_window,
        } = queued;
        let global = touch_id(&event).is_some_and(|id| {
            self.input_owner
                .as_ref()
                .is_some_and(|input| input.global_touch_matches(id))
        });
        if touch_window.is_some() && !touch_window_matches(touch_window, own_window, global) {
            return Ok(None);
        }
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
            event @ (Event::FingerDown { .. }
            | Event::FingerMotion { .. }
            | Event::FingerUp { .. })
                if touch_window_matches(touch_window, own_window, global) =>
            {
                let (width, height) = self.initialized.window_size();
                self.pointer
                    .observe(&event, width, height)
                    .map(|sample| sample.map(NativeInput::Pointer))
                    .map_err(|_| NativeInputRefusal::InvalidPointer)
            }
            event @ (Event::MouseMotion { window_id, .. }
            | Event::MouseButtonDown { window_id, .. }
            | Event::MouseButtonUp { window_id, .. })
                if window_id == own_window =>
            {
                let (width, height) = self.initialized.window_size();
                self.pointer
                    .observe(&event, width, height)
                    .map(|sample| sample.map(NativeInput::Pointer))
                    .map_err(|_| NativeInputRefusal::InvalidPointer)
            }
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

#[cfg(test)]
mod touch_tests;
