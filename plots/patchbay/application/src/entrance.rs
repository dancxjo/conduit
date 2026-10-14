//! Shared renderer-neutral state for the resident Patchbay entrance.

use alloc::{format, string::String, vec::Vec};
use conduit_body::BodyId;
use conduit_face::{Face, FacePropertyValue, FaceRole};
use serde::{Deserialize, Serialize};

pub const MAX_ENTRANCE_ACTIONS: usize = 8;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntranceLayer {
    World,
    Intent,
    Realization,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntranceAction {
    Inspect,
    Open,
    Join,
    Birth,
    OpenPlots,
    Admit,
    Refuse,
    Replan,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntranceUpdateDisposition {
    SelectionPreserved,
    SelectionBecameStale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntranceRefusal {
    InvalidFace,
    MissingBody,
    WrongBody,
    StaleRevision,
    UnknownSubject,
    LayerUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayEntranceState {
    pub body_id: Option<BodyId>,
    pub face_id: String,
    pub face_revision: u64,
    pub layer: EntranceLayer,
    pub selected_subject: Option<String>,
    pub available_actions: Vec<EntranceAction>,
    pub last_refusal: Option<EntranceRefusal>,
}

impl PatchbayEntranceState {
    pub fn enter(face: &Face) -> Result<Self, EntranceRefusal> {
        face.validate().map_err(|_| EntranceRefusal::InvalidFace)?;
        let body_subject = face
            .basis
            .body_id
            .as_ref()
            .map(|body_id| format!("body/{}", body_id.as_str()));
        if body_subject.as_ref().is_some_and(|identity| {
            !face
                .subjects
                .iter()
                .any(|subject| &subject.identity == identity && subject.role == FaceRole::Body)
        }) {
            return Err(EntranceRefusal::MissingBody);
        }
        let selected_subject = here_part(face)
            .or(body_subject)
            .or_else(|| this_host(face))
            .ok_or(EntranceRefusal::InvalidFace)?;
        Ok(Self {
            body_id: face.basis.body_id.clone(),
            face_id: face.identity.as_str().into(),
            face_revision: face.revision,
            layer: EntranceLayer::World,
            available_actions: actions_for(face, Some(&selected_subject)),
            selected_subject: Some(selected_subject),
            last_refusal: None,
        })
    }

    pub fn update(&mut self, face: &Face) -> Result<EntranceUpdateDisposition, EntranceRefusal> {
        face.validate().map_err(|_| EntranceRefusal::InvalidFace)?;
        if face.basis.body_id != self.body_id {
            return self.refuse(EntranceRefusal::WrongBody);
        }
        if face.revision <= self.face_revision {
            return self.refuse(EntranceRefusal::StaleRevision);
        }
        let disposition = match self.selected_subject.as_deref() {
            None => EntranceUpdateDisposition::SelectionPreserved,
            Some(subject) if has_subject(face, subject) => {
                EntranceUpdateDisposition::SelectionPreserved
            }
            Some(_) => {
                self.selected_subject = Some(
                    here_part(face)
                        .or_else(|| {
                            self.body_id
                                .as_ref()
                                .map(|body_id| format!("body/{}", body_id.as_str()))
                        })
                        .or_else(|| this_host(face))
                        .ok_or(EntranceRefusal::InvalidFace)?,
                );
                EntranceUpdateDisposition::SelectionBecameStale
            }
        };
        self.face_id = face.identity.as_str().into();
        self.face_revision = face.revision;
        self.available_actions = actions_for(face, self.selected_subject.as_deref());
        self.last_refusal = None;
        Ok(disposition)
    }

    pub fn select(&mut self, face: &Face, subject: &str) -> Result<(), EntranceRefusal> {
        self.require_current(face)?;
        if !has_subject(face, subject) {
            return self.refuse(EntranceRefusal::UnknownSubject);
        }
        self.selected_subject = Some(subject.into());
        self.available_actions = actions_for(face, Some(subject));
        self.last_refusal = None;
        Ok(())
    }

    pub fn clear_selection(&mut self, face: &Face) -> Result<(), EntranceRefusal> {
        self.require_current(face)?;
        self.selected_subject = None;
        self.available_actions.clear();
        self.layer = EntranceLayer::World;
        self.last_refusal = None;
        Ok(())
    }

    pub fn show_layer(&mut self, face: &Face, layer: EntranceLayer) -> Result<(), EntranceRefusal> {
        self.require_current(face)?;
        let available = match layer {
            EntranceLayer::World => true,
            EntranceLayer::Intent => face
                .subjects
                .iter()
                .any(|subject| subject.role == FaceRole::Plot),
            EntranceLayer::Realization => face.basis.plan_id.is_some(),
        };
        if !available {
            return self.refuse(EntranceRefusal::LayerUnavailable);
        }
        self.layer = layer;
        self.last_refusal = None;
        Ok(())
    }

    fn require_current(&mut self, face: &Face) -> Result<(), EntranceRefusal> {
        if face.validate().is_err() {
            return self.refuse(EntranceRefusal::InvalidFace);
        }
        if face.basis.body_id != self.body_id {
            return self.refuse(EntranceRefusal::WrongBody);
        }
        if face.revision != self.face_revision || face.identity.as_str() != self.face_id {
            return self.refuse(EntranceRefusal::StaleRevision);
        }
        Ok(())
    }

    fn refuse<T>(&mut self, refusal: EntranceRefusal) -> Result<T, EntranceRefusal> {
        self.last_refusal = Some(refusal.clone());
        Err(refusal)
    }
}

fn here_part(face: &Face) -> Option<String> {
    face.properties.iter().find_map(|property| {
        (property.name == "membership-state"
            && property.value == FacePropertyValue::Text("here".into()))
        .then(|| property.subject.clone())
    })
}

fn this_host(face: &Face) -> Option<String> {
    face.properties.iter().find_map(|property| {
        (property.name == "this-host" && property.value == FacePropertyValue::Flag(true))
            .then(|| property.subject.clone())
    })
}

fn has_subject(face: &Face, identity: &str) -> bool {
    face.subjects
        .iter()
        .any(|subject| subject.identity == identity)
}

fn actions_for(face: &Face, identity: Option<&str>) -> Vec<EntranceAction> {
    let role = identity.and_then(|identity| {
        face.subjects
            .iter()
            .find(|subject| subject.identity == identity)
            .map(|subject| subject.role.clone())
    });
    let actions = match role {
        Some(FaceRole::Body)
            if identity.is_some_and(|identity| property_flag(face, identity, "current")) =>
        {
            &[EntranceAction::Inspect, EntranceAction::OpenPlots][..]
        }
        Some(FaceRole::Body)
            if identity.is_some_and(|identity| property_flag(face, identity, "opened")) =>
        {
            &[EntranceAction::Inspect, EntranceAction::Join]
        }
        Some(FaceRole::Body) => &[EntranceAction::Inspect, EntranceAction::Open],
        Some(FaceRole::Plot)
            if identity.is_some_and(|identity| property_flag(face, identity, "opened")) =>
        {
            &[EntranceAction::Inspect, EntranceAction::Birth]
        }
        Some(FaceRole::Plot) => &[EntranceAction::Inspect, EntranceAction::Open],
        Some(FaceRole::Candidate) => &[
            EntranceAction::Inspect,
            EntranceAction::Admit,
            EntranceAction::Refuse,
        ],
        Some(FaceRole::Plan) => &[EntranceAction::Inspect, EntranceAction::Replan],
        Some(FaceRole::Host)
            if identity.is_some_and(|identity| property_flag(face, identity, "creche-primary")) =>
        {
            &[EntranceAction::Inspect, EntranceAction::Birth]
        }
        Some(_) => &[EntranceAction::Inspect],
        None => &[],
    };
    debug_assert!(actions.len() <= MAX_ENTRANCE_ACTIONS);
    actions.to_vec()
}

fn property_flag(face: &Face, identity: &str, name: &str) -> bool {
    face.properties.iter().any(|property| {
        property.subject == identity
            && property.name == name
            && property.value == FacePropertyValue::Flag(true)
    })
}
