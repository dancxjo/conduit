//! One bounded tap recognizer. It owns no semantic effects or device history.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Control(FaceControl),
    Subject(usize),
    Page(bool),
}

pub(super) struct PendingTap {
    target: Target,
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
                Target::Control(control) => Some(control),
                Target::Subject(_) | Target::Page(_) => None,
            })
    }

    pub(super) fn pressed_page(&self) -> Option<bool> {
        self.input
            .pointer
            .as_ref()
            .and_then(|tap| match tap.target {
                Target::Page(forward) => Some(forward),
                _ => None,
            })
    }

    pub fn cancel_pointer_gesture(&mut self) -> bool {
        self.input.pointer.take().is_some()
    }

    /// Eight logical pixels on either axis cede ownership to movement. Leaving
    /// the original control cancels permanently, even if the pointer returns.
    /// A new physical press edge from the validated device history owner is
    /// required to acquire another sequence; Show replacement cannot create it.
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
        if !press_edge && self.input.pointer.is_none() {
            return Ok(FaceSceneInput::Unchanged);
        }
        let target = self.pointer_target(x, y)?;
        if press_edge {
            self.input.pointer = target.map(|target| PendingTap {
                target,
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
            || target != Some(tap.target)
            || x.abs_diff(tap.x) > 8
            || y.abs_diff(tap.y) > 8
        {
            self.cancel_pointer_gesture();
            return Ok(FaceSceneInput::Changed);
        }
        if pressed {
            return Ok(FaceSceneInput::Unchanged);
        }
        self.cancel_pointer_gesture();
        // Only completed recognition enters the existing semantic input path.
        self.pointer(x, y, show, sequence)
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
