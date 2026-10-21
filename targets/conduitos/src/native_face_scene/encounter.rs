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
        // A geometry refusal changes realization, not the requested reading
        // view. Retry that request under the new Look, unless the user has
        // explicitly left graph reading in the meantime.
        let retry_diagram = previous.showing_diagram
            || (previous.showing_details
                && previous
                    .graph_reading
                    .as_ref()
                    .is_some_and(|reading| reading.active && previous.diagram_refusal().is_some()));
        self.graph_reading = previous.graph_reading;
        if previous.showing_diagram
            && let Some(reading) = self.graph_reading.as_mut()
        {
            reading.diagram_page = previous.page;
        }
        self.local_notice = previous.local_notice;
        self.showing_details = previous.showing_details;
        self.showing_diagram = false;
        self.page = previous.page.min(self.pages() - 1);
        if retry_diagram {
            self.show_diagram(true);
            if self.showing_diagram && previous.showing_diagram {
                self.page = previous.page.min(self.pages() - 1);
            }
        } else if self.showing_details {
            self.page = self.graph_focus_page().unwrap_or(self.page);
        }
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
