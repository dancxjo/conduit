//! Portable checked focus/disclosure for exact Gear, Port and Cord subjects.
//! All subjects remain reachable even when their geometry is omitted or off-page.
use super::*;
use conduit_face::{
    FaceCursor, FaceDepth, FaceNavigation, FaceRole, NavigationOperation, NavigationState,
};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use patchbay_application::PatchbayNavigationProjection;

pub(super) struct GraphReading {
    bundle: PatchbayNavigationProjection,
    pub active: bool,
    pub diagram_page: usize,
}

impl GraphReading {
    pub fn prepare(face: &Face) -> Result<Option<Self>, FaceSceneError> {
        if !face.subjects.iter().any(|subject| {
            matches!(
                subject.role,
                FaceRole::Gear | FaceRole::Port | FaceRole::Cord
            )
        }) {
            return Ok(None);
        }
        let mut bundle = PatchbayNavigationProjection::for_graph(face)
            .map_err(|_| FaceSceneError::InvalidFace)?;
        // No local reading target is selected until the first navigation input.
        bundle.cursor.focus = None;
        Ok(Some(Self {
            bundle,
            active: false,
            diagram_page: 0,
        }))
    }

    fn focus(&mut self, face: &Face, identity: &str) -> Result<(), FaceSceneError> {
        // Each local selection is one checked operation. Do not accumulate a
        // hidden 64-entry history that would prevent traversing a larger Face.
        let mut state =
            NavigationState::new(&self.bundle.navigation, self.bundle.cursor.clone(), 1)
                .map_err(|_| FaceSceneError::StaleFace)?;
        state
            .navigate(
                face,
                &self.bundle.navigation,
                face.revision,
                NavigationOperation::FocusAndDisclose(identity.into(), FaceDepth::Exact),
            )
            .map_err(|_| FaceSceneError::UnknownControl)?;
        self.bundle.cursor = state.cursor().clone();
        self.active = true;
        Ok(())
    }

    fn adjacent(&self, forward: bool) -> Option<String> {
        let subjects = &self.bundle.navigation.places[0].aspects[0].focusable_subjects;
        let current = self
            .bundle
            .cursor
            .focus
            .as_ref()
            .and_then(|focus| subjects.iter().position(|subject| subject == focus));
        let index = match (current, forward) {
            (Some(index), true) => (index + 1) % subjects.len(),
            (Some(index), false) => (index + subjects.len() - 1) % subjects.len(),
            (None, true) => 0,
            (None, false) => subjects.len() - 1,
        };
        subjects.get(index).cloned()
    }
}

impl NativeFaceScene {
    pub fn graph_navigation(&self) -> Option<(&FaceNavigation, &FaceCursor)> {
        self.graph_reading
            .as_ref()
            .map(|reading| (&reading.bundle.navigation, &reading.bundle.cursor))
    }

    /// Same serializable bundle accepted by the browser workbench and existing
    /// render_linear_navigation; no native positions enter this description.
    pub fn graph_projection(&self) -> Option<&PatchbayNavigationProjection> {
        self.graph_reading.as_ref().map(|reading| &reading.bundle)
    }

    /// Pure reading: no editable graph operation or producer action is invented.
    pub fn inspect_subject(
        &mut self,
        face_id: &FaceContentId,
        revision: u64,
        identity: &str,
    ) -> Result<(), FaceSceneError> {
        if face_id != &self.face.identity || revision != self.face.revision {
            return Err(FaceSceneError::StaleFace);
        }
        if self.input.has_pending_input() {
            return Err(FaceSceneError::IncompleteInput);
        }
        let subject = self
            .face
            .subjects
            .iter()
            .position(|subject| subject.identity == identity)
            .ok_or(FaceSceneError::UnknownControl)?;
        let page = self
            .details
            .iter()
            .find(|row| row.subject_index == Some(subject))
            .ok_or(FaceSceneError::DocumentBound)?
            .page;
        let reading = self
            .graph_reading
            .as_mut()
            .ok_or(FaceSceneError::UnknownControl)?;
        reading.focus(&self.face, identity)?;
        if self.showing_diagram {
            reading.diagram_page = self.page;
        }
        self.show_details(true);
        self.graph_reading.as_mut().unwrap().active = true;
        self.focus = None;
        self.page = page;
        Ok(())
    }

    pub(super) fn graph_key(
        &mut self,
        event: KeyEvent,
    ) -> Result<Option<FaceSceneInput>, FaceSceneError> {
        if event.transition() != KeyTransition::Pressed {
            return Ok(None);
        }
        let active = self.showing_diagram
            || (self.showing_details
                && self
                    .graph_reading
                    .as_ref()
                    .is_some_and(|reading| reading.active));
        if !active {
            return Ok(None);
        }
        let modifiers = event.modifiers_after().bits();
        let shift = KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits();
        if (event.usage() == 43 && modifiers & !shift != 0)
            || ((30..=35).contains(&event.usage()) && modifiers != 0)
        {
            return Ok(Some(FaceSceneInput::Unchanged));
        }
        let identity = if self.showing_diagram && (30..=35).contains(&event.usage()) {
            let mut drawing = GraphicsScene::empty();
            let hits = diagram::append(&mut drawing, &self.face, self.screen, self.page)?;
            hits.gears[usize::from(event.usage() - 30)].map(|(_, id)| String::from(id))
        } else if event.usage() == 43 {
            let shifted = event.modifiers_after().bits()
                & (KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits())
                != 0;
            self.graph_reading
                .as_ref()
                .and_then(|reading| reading.adjacent(!shifted))
        } else if event.usage() == 40 {
            // A disclosed subject is reading focus, never an Invoke target.
            return Ok(Some(FaceSceneInput::Unchanged));
        } else if event.usage() == 41 && self.showing_details {
            self.show_diagram(true);
            return Ok(Some(FaceSceneInput::Changed));
        } else {
            return Ok(None);
        };
        if let Some(identity) = identity {
            self.inspect_subject(&self.face.identity.clone(), self.face.revision, &identity)?;
            Ok(Some(FaceSceneInput::Changed))
        } else {
            Ok(Some(FaceSceneInput::Unchanged))
        }
    }

    pub(super) fn graph_subject_focused(&self, index: Option<usize>) -> bool {
        self.showing_details
            && self.graph_reading.as_ref().is_some_and(|reading| {
                reading.active
                    && index
                        .and_then(|index| self.face.subjects.get(index))
                        .is_some_and(|subject| {
                            reading.bundle.cursor.focus.as_deref()
                                == Some(subject.identity.as_str())
                        })
            })
    }
}
