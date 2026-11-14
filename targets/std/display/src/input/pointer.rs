//! One bounded primary pointer from actual SDL mouse or owned-window touch.
//! Touch-emulated mouse occurrences are excluded to prevent double activation.
use conduit_semantic_catalog::NormalizedPointerSample;
use sdl2::{event::Event, mouse::MouseButton};
#[derive(Default)]
pub(crate) struct Pointer {
    position: Option<(i64, i64)>,
    pressed: bool,
    armed: bool,
    sequence: u64,
    finger: Option<(i64, i64)>,
}
impl Pointer {
    pub(super) fn prepare(&mut self, held: bool) {
        self.armed = !held && self.finger.is_none();
        self.pressed = false;
    }
    /// Dropped pre-Play contact is historical, not a fresh Down. Window identity
    /// is checked by the raw-queue adapter before this function is called.
    pub(super) fn discard_touch(&mut self, event: &Event) {
        if matches!(
            event,
            Event::FingerDown { touch_id: -1, .. }
                | Event::FingerMotion { touch_id: -1, .. }
                | Event::FingerUp { touch_id: -1, .. }
        ) {
            return;
        }
        match event {
            Event::FingerDown {
                touch_id,
                finger_id,
                ..
            }
            | Event::FingerMotion {
                touch_id,
                finger_id,
                ..
            } if self.finger.is_none() => self.finger = Some((*touch_id, *finger_id)),
            Event::FingerUp {
                touch_id,
                finger_id,
                ..
            } if self.finger == Some((*touch_id, *finger_id)) => self.finger = None,
            _ => {}
        }
    }
    /// Existing SDL touch state only suppresses historical presses. No contact
    /// or readiness is manufactured from this state query.
    pub(super) fn reconcile_held_touch(&mut self) -> Result<(), ()> {
        let devices = sdl2::touch::num_touch_devices();
        if !(0..=16).contains(&devices) {
            return Err(());
        }
        let mut total = 0_i32;
        for index in 0..devices {
            let touch = sdl2::touch::touch_device(index);
            if touch == -1 {
                continue;
            } // SDL_MOUSE_TOUCHID: physical mouse has its own held-state witness.
            let fingers = sdl2::touch::num_touch_fingers(touch);
            if fingers < 0 {
                return Err(());
            }
            total = total.checked_add(fingers).ok_or(())?;
            if total > 64 {
                return Err(());
            }
            if self.finger.is_none() && fingers > 0 {
                let finger = sdl2::touch::touch_finger(touch, 0).ok_or(())?;
                self.finger = Some((touch, finger.id));
            }
        }
        Ok(())
    }
    pub(super) fn observe(
        &mut self,
        event: &Event,
        width: u32,
        height: u32,
    ) -> Result<Option<NormalizedPointerSample>, ()> {
        if let Event::FingerDown {
            touch_id,
            finger_id,
            x,
            y,
            dx,
            dy,
            pressure,
            ..
        }
        | Event::FingerMotion {
            touch_id,
            finger_id,
            x,
            y,
            dx,
            dy,
            pressure,
            ..
        }
        | Event::FingerUp {
            touch_id,
            finger_id,
            x,
            y,
            dx,
            dy,
            pressure,
            ..
        } = event
        {
            if *touch_id == -1 {
                return Ok(None);
            } // SDL_MOUSE_TOUCHID must not duplicate ordinary mouse input.
            if ![x, y, dx, dy, pressure].into_iter().all(|v| v.is_finite())
                || !(0.0..=1.0).contains(x)
                || !(0.0..=1.0).contains(y)
                || !(0.0..=1.0).contains(pressure)
            {
                return Err(());
            }
            let id = (*touch_id, *finger_id);
            let down = matches!(event, Event::FingerDown { .. });
            let up = matches!(event, Event::FingerUp { .. });
            if down {
                if self.finger.is_some() || self.pressed || !self.armed {
                    return Ok(None);
                }
            } else if self.finger != Some(id) {
                return Ok(None);
            }
            if !self.armed && !up {
                return Ok(None);
            }
            let sample = self.emit(
                (
                    (*x as f64 * 1_000_000.0).round() as i64,
                    (*y as f64 * 1_000_000.0).round() as i64,
                ),
                !up,
            )?;
            self.finger = if up { None } else { Some(id) };
            self.armed = true;
            return Ok(Some(sample));
        }
        if self.finger.is_some() {
            return Ok(None);
        }
        let (x, y, pressed, motion) = match event {
            Event::MouseMotion {
                which,
                x,
                y,
                mousestate,
                ..
            } if *which != u32::MAX => (*x, *y, mousestate.left(), true),
            Event::MouseButtonDown {
                which,
                x,
                y,
                mouse_btn: MouseButton::Left,
                ..
            } if *which != u32::MAX => (*x, *y, true, false),
            Event::MouseButtonUp {
                which,
                x,
                y,
                mouse_btn: MouseButton::Left,
                ..
            } if *which != u32::MAX => (*x, *y, false, false),
            _ => return Ok(None),
        };
        if !self.armed {
            if pressed {
                return Ok(None);
            }
        } else if motion && pressed != self.pressed {
            return Err(());
        } else if !motion && pressed == self.pressed {
            return Ok(None);
        }
        let scale = |value: i32, extent: u32| -> Result<i64, ()> {
            let value = u32::try_from(value).map_err(|_| ())?;
            if extent == 0 || value >= extent {
                return Err(());
            }
            Ok(if extent == 1 {
                0
            } else {
                i64::from(value) * 1_000_000 / i64::from(extent - 1)
            })
        };
        let sample = self.emit((scale(x, width)?, scale(y, height)?), pressed)?;
        self.armed = true;
        Ok(Some(sample))
    }
    fn emit(&mut self, position: (i64, i64), pressed: bool) -> Result<NormalizedPointerSample, ()> {
        let previous = self.position.unwrap_or(position);
        let sequence = self.sequence.checked_add(1).ok_or(())?;
        self.position = Some(position);
        self.pressed = pressed;
        self.sequence = sequence;
        Ok(NormalizedPointerSample {
            position_x: position.0,
            position_y: position.1,
            delta_x: position.0 - previous.0,
            delta_y: position.1 - previous.1,
            primary_pressed: pressed,
            coalesced: 0,
            dropped: 0,
            queue_capacity: 1,
            sequence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn button(pressed: bool, x: i32, y: i32) -> Event {
        if pressed {
            Event::MouseButtonDown {
                timestamp: 1,
                window_id: 1,
                which: 0,
                mouse_btn: MouseButton::Left,
                clicks: 1,
                x,
                y,
            }
        } else {
            Event::MouseButtonUp {
                timestamp: 2,
                window_id: 1,
                which: 0,
                mouse_btn: MouseButton::Left,
                clicks: 1,
                x,
                y,
            }
        }
    }
    #[test]
    fn held_mouse_requires_release_and_normalized_geometry_is_not_panel_specific() {
        let mut pointer = Pointer::default();
        pointer.prepare(true);
        assert!(pointer
            .observe(&button(true, 99, 49), 100, 50)
            .unwrap()
            .is_none());
        let released = pointer
            .observe(&button(false, 99, 49), 100, 50)
            .unwrap()
            .unwrap();
        assert_eq!(
            (released.position_x, released.position_y),
            (1_000_000, 1_000_000)
        );
        assert!(!released.primary_pressed);
        let pressed = pointer
            .observe(&button(true, 0, 0), 100, 50)
            .unwrap()
            .unwrap();
        assert!(pressed.primary_pressed);
        assert_eq!(
            (pressed.delta_x, pressed.delta_y, pressed.sequence),
            (-1_000_000, -1_000_000, 2)
        );
        assert!(pointer
            .observe(&button(true, 0, 0), 100, 50)
            .unwrap()
            .is_none());
    }
    #[test]
    fn invalid_coordinates_do_not_advance_the_observation() {
        let mut pointer = Pointer::default();
        pointer.prepare(false);
        assert!(pointer.observe(&button(true, -1, 0), 100, 50).is_err());
        assert_eq!(
            pointer
                .observe(&button(true, 1, 1), 100, 50)
                .unwrap()
                .unwrap()
                .sequence,
            1
        );
    }
}
