//! One prepared window's ordinary SDL mouse stream, including SDL's synthetic
//! mouse events for a touch device. Finger events are not delivered a second time.
use conduit_semantic_catalog::NormalizedPointerSample;
use sdl2::{event::Event, mouse::MouseButton};

#[derive(Default)]
pub(crate) struct Pointer {
    position: Option<(i64, i64)>,
    pressed: bool,
    armed: bool,
    sequence: u64,
}
impl Pointer {
    pub(super) fn prepare(&mut self, held: bool) {
        // An already held button is historical input, never a new activation.
        self.armed = !held;
        self.pressed = false;
    }
    pub(super) fn observe(
        &mut self,
        event: &Event,
        width: u32,
        height: u32,
    ) -> Result<Option<NormalizedPointerSample>, ()> {
        let (x, y, pressed, motion) = match event {
            Event::MouseMotion {
                x, y, mousestate, ..
            } => (*x, *y, mousestate.left(), true),
            Event::MouseButtonDown {
                x,
                y,
                mouse_btn: MouseButton::Left,
                ..
            } => (*x, *y, true, false),
            Event::MouseButtonUp {
                x,
                y,
                mouse_btn: MouseButton::Left,
                ..
            } => (*x, *y, false, false),
            _ => return Ok(None),
        };
        if !self.armed {
            if pressed {
                return Ok(None);
            }
            self.armed = true;
        } else if motion && pressed != self.pressed {
            // Do not synthesize a missing press/release from a state snapshot.
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
        let position = (scale(x, width)?, scale(y, height)?);
        let previous = self.position.unwrap_or(position);
        let sequence = self.sequence.checked_add(1).ok_or(())?;
        self.position = Some(position);
        self.pressed = pressed;
        self.sequence = sequence;
        Ok(Some(NormalizedPointerSample {
            position_x: position.0,
            position_y: position.1,
            delta_x: position.0 - previous.0,
            delta_y: position.1 - previous.1,
            primary_pressed: pressed,
            // Counts describe this adapter's uncoalesced one-observation slot,
            // not a claim about USB/controller or SDL's upstream queue history.
            coalesced: 0,
            dropped: 0,
            queue_capacity: 1,
            sequence,
        }))
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
                which: u32::MAX,
                mouse_btn: MouseButton::Left,
                clicks: 1,
                x,
                y,
            }
        } else {
            Event::MouseButtonUp {
                timestamp: 2,
                window_id: 1,
                which: u32::MAX,
                mouse_btn: MouseButton::Left,
                clicks: 1,
                x,
                y,
            }
        }
    }
    #[test]
    fn held_touch_requires_release_and_normalized_geometry_is_not_panel_specific() {
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
