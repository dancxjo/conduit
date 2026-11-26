//! Bounded tap/swipe arbitration over one exact Show, without device history.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Control(FaceControl),
    Subject(usize),
    Page(bool),
}

pub(super) struct PendingGesture {
    target: Option<Target>,
    swipe: bool,
    x: i16,
    y: i16,
    show: [u8; 32],
    context: (usize, bool, bool),
}

impl NativeFaceScene {
    pub(crate) fn pressed_control(&self) -> Option<FaceControl> {
        self.input
            .pointer
            .as_ref()
            .and_then(|tap| match tap.target {
                Some(Target::Control(control)) => Some(control),
                _ => None,
            })
    }

    pub(super) fn pressed_page(&self) -> Option<bool> {
        self.input
            .pointer
            .as_ref()
            .and_then(|tap| match tap.target {
                Some(Target::Page(forward)) => Some(forward),
                _ => None,
            })
    }

    pub fn cancel_pointer_gesture(&mut self) -> bool {
        self.input.pointer.take().is_some()
    }

    /// A tap permanently loses its target after eight logical pixels or leaving
    /// its hit. A page swipe may continue, but completes only on release with
    /// horizontal travel of at least max(48, width/5), and never more than 24
    /// vertical pixels. Both interpretations retain one exact Show/view basis.
    pub fn pointer_sample(
        &mut self,
        x: i16,
        y: i16,
        pressed: bool,
        press_edge: bool,
        show: &MaskShow,
        sequence: u64,
    ) -> Result<FaceSceneInput, FaceSceneError> {
        if !self.local_input_available {
            self.cancel_pointer_gesture();
            return Err(FaceSceneError::UnsupportedInput);
        }
        if let Err(error) = self.check_show(show) {
            self.cancel_pointer_gesture();
            return Err(error);
        }
        if x < 0 || y < 0 || x as u16 >= self.screen.width || y as u16 >= self.screen.height {
            return Ok(if self.cancel_pointer_gesture() {
                FaceSceneInput::Changed
            } else {
                FaceSceneInput::Unchanged
            });
        }
        if !press_edge && self.input.pointer.is_none() {
            return Ok(FaceSceneInput::Unchanged);
        }
        let target = self.pointer_target(x, y)?;
        if press_edge {
            let swipe = self.page_controls()?.is_some();
            self.input.pointer = (target.is_some() || swipe).then_some(PendingGesture {
                target,
                swipe,
                x,
                y,
                show: Sha256::digest(show.show_id.as_str().as_bytes()).into(),
                context: (self.page, self.showing_details, self.showing_diagram),
            });
            return Ok(if self.input.pointer.is_some() {
                FaceSceneInput::Changed
            } else {
                FaceSceneInput::Unchanged
            });
        }
        let Some(tap) = self.input.pointer.as_ref() else {
            return Ok(FaceSceneInput::Unchanged);
        };
        if tap.show != <[u8; 32]>::from(Sha256::digest(show.show_id.as_str().as_bytes()))
            || tap.context != (self.page, self.showing_details, self.showing_diagram)
        {
            self.cancel_pointer_gesture();
            return Ok(FaceSceneInput::Changed);
        }
        let dx = i32::from(x) - i32::from(tap.x);
        let dy = y.abs_diff(tap.y);
        let lost_target =
            tap.target.is_some() && (target != tap.target || dx.unsigned_abs() > 8 || dy > 8);
        let tap = self.input.pointer.as_mut().unwrap();
        if lost_target {
            tap.target = None;
        }
        if dy > 24 {
            tap.swipe = false;
        }
        if pressed {
            return Ok(if lost_target {
                FaceSceneInput::Changed
            } else {
                FaceSceneInput::Unchanged
            });
        }
        let tap = self.input.pointer.take().unwrap();
        if tap.swipe
            && dx.unsigned_abs() >= u32::from((self.screen.width / 5).max(48))
            && self.page_controls()?.is_some()
        {
            let before = self.page;
            self.turn_page(dx < 0);
            return Ok(if self.page != before {
                FaceSceneInput::Changed
            } else {
                FaceSceneInput::Unchanged
            });
        }
        if tap.target.is_some() {
            // Only a completed tap enters the existing semantic input path.
            self.pointer(x, y, show, sequence)
        } else {
            Ok(if lost_target {
                FaceSceneInput::Changed
            } else {
                FaceSceneInput::Unchanged
            })
        }
    }

    fn pointer_target(&self, x: i16, y: i16) -> Result<Option<Target>, FaceSceneError> {
        if let Some((forward, available)) = self.page_target(x, y)? {
            return Ok(available.then_some(Target::Page(forward)));
        }
        if self.showing_diagram {
            let mut drawing = GraphicsScene::empty();
            let hits = diagram::append(
                &mut drawing,
                &self.face,
                self.screen,
                self.page,
                self.look_corners.map(|corners| corners.panel),
            )?;
            return Ok(hits
                .hit_test(x, y)
                .and_then(|identity| {
                    self.face
                        .subjects
                        .iter()
                        .position(|subject| subject.identity == identity)
                })
                .map(Target::Subject));
        }
        Ok(self
            .frame()?
            .hit_test(x, y)
            .map(|hit| Target::Control(hit.control)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_control_reuses_one_admitted_marker() {
        let mut face = super::super::tests::face(7);
        for action in &mut face.actions {
            action.name =
                "A deliberately long label for a control that wraps across several measured rows "
                    .repeat(4);
        }
        let face = Face::new_with_semantics(
            face.revision,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            face.actions,
            face.disclosures,
        )
        .unwrap();
        let show = super::super::tests::fixture::show(&face);
        let mut scene = NativeFaceScene::prepare(face, 320, 480).unwrap();
        scene
            .focus_named(&FaceFocusRequest {
                action_id: "finish".into(),
                argument_name: None,
            })
            .unwrap();
        let control = scene.focused().unwrap();
        assert!(
            scene
                .rows()
                .iter()
                .filter(|row| row.page == scene.page() && row.control == Some(control))
                .count()
                > 1
        );
        let frame = scene.frame().unwrap();
        let count = frame.scene.commands().len();
        let hit = frame
            .hits
            .iter()
            .find(|hit| hit.control == control)
            .unwrap();
        let (x, y) = (hit.bounds.x + 4, hit.bounds.y + 4);
        scene.pointer_sample(x, y, true, true, &show, 1).unwrap();
        assert_eq!(scene.focused(), Some(control));
        assert_eq!(
            scene.frame().unwrap().scene.commands().len(),
            count,
            "wrapped rows must share the original single focus-marker budget"
        );
    }
}
