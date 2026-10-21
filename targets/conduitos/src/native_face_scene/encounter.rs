//! Preserve an unsubmitted Mask encounter only across an identical immutable Face.
use super::*;
impl NativeFaceScene {
    pub(crate) fn retain_encounter(&mut self, previous: Self) -> Result<(), FaceSceneError> {
        if self.face.identity != previous.face.identity
            || self.face.revision != previous.face.revision
        {
            return Err(FaceSceneError::StaleFace);
        }
        if self.interaction_admitted != previous.interaction_admitted || previous.input.submitted {
            return Err(FaceSceneError::InputUnavailable);
        }
        self.inspected_subject = previous.inspected_subject;
        self.showing_details = previous.showing_details;
        self.showing_diagram = previous.showing_diagram;
        self.page = previous.page.min(self.pages() - 1);
        if let Some(control) = previous.focus {
            let page = self
                .rows()
                .iter()
                .find_map(|row| {
                    (row.control == Some(control))
                        .then(|| self.eligible(row))
                        .flatten()
                })
                .ok_or(FaceSceneError::UnknownControl)?
                .1;
            self.focus = Some(control);
            self.page = page;
        }
        self.input = previous.input;
        self.local_notice = previous.local_notice;
        // The new layout must admit the retained draft/reading view before Show.
        self.frame()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_state_refuses_a_different_face_revision() {
        let previous = NativeFaceScene::prepare(super::super::tests::face(7), 640, 480).unwrap();
        let mut fresh = NativeFaceScene::prepare(super::super::tests::face(8), 640, 480).unwrap();
        assert_eq!(
            fresh.retain_encounter(previous),
            Err(FaceSceneError::StaleFace)
        );
        assert_eq!(fresh.face().revision, 8);
        assert_eq!(fresh.focused(), None);
    }
}
