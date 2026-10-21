//! Local graph inspection follows immutable Face subjects, never display indices.
use super::*;
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

impl NativeFaceScene {
    pub(super) fn inspect_key(&mut self, event: KeyEvent) -> Result<bool, FaceSceneError> {
        if event.transition() != KeyTransition::Pressed {
            return Ok(false);
        }
        let modifiers = event.modifiers_after().bits();
        let alt = modifiers == KeyModifiers::LEFT_ALT.bits()
            || modifiers == KeyModifiers::RIGHT_ALT.bits();
        let identity = if alt
            && matches!(event.usage(), 79 | 80)
            && (self.showing_diagram || self.showing_details)
            && self.has_diagram()
        {
            let eligible = |(_, subject): &(usize, &conduit_face::FaceSubject)| {
                matches!(
                    subject.role,
                    conduit_face::FaceRole::Gear
                        | conduit_face::FaceRole::Port
                        | conduit_face::FaceRole::Cord
                )
            };
            let mut subjects = self.face.subjects.iter().enumerate().filter(eligible);
            let selected = if event.usage() == 79 {
                subjects
                    .find(|(index, _)| self.inspected_subject.is_none_or(|old| *index > old))
                    .or_else(|| self.face.subjects.iter().enumerate().find(eligible))
            } else {
                subjects
                    .rev()
                    .find(|(index, _)| self.inspected_subject.is_none_or(|old| *index < old))
                    .or_else(|| self.face.subjects.iter().enumerate().rev().find(eligible))
            };
            selected.map(|(_, subject)| subject.identity.clone())
        } else if self.showing_diagram && modifiers == 0 && (30..=35).contains(&event.usage()) {
            let mut drawing = GraphicsScene::empty();
            diagram::append(&mut drawing, &self.face, self.screen, self.page)?
                [usize::from(event.usage() - 30)]
            .map(|(_, identity)| identity.into())
        } else {
            return Ok(false);
        };
        let Some(identity) = identity else {
            return Ok(false);
        };
        self.inspect_subject(&self.face.identity.clone(), self.face.revision, &identity)?;
        Ok(true)
    }
}
