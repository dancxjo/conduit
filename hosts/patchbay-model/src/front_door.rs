//! Shared renderer-neutral state for the world-first Patchbay entrance.

use conduit_body::BodyId;
use conduit_presentation::{Presentation, PresentationPropertyValue, PresentationRole};
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
    BeBorn,
    OpenForms,
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
    InvalidPresentation,
    MissingBody,
    WrongBody,
    StaleRevision,
    UnknownSubject,
    LayerUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayEntranceState {
    pub body_id: Option<BodyId>,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub layer: EntranceLayer,
    pub selected_subject: Option<String>,
    pub available_actions: Vec<EntranceAction>,
    pub last_refusal: Option<EntranceRefusal>,
}

impl PatchbayEntranceState {
    pub fn enter(presentation: &Presentation) -> Result<Self, EntranceRefusal> {
        presentation
            .validate()
            .map_err(|_| EntranceRefusal::InvalidPresentation)?;
        let body_subject = presentation
            .basis
            .body_id
            .as_ref()
            .map(|body_id| format!("body/{}", body_id.as_str()));
        if body_subject.as_ref().is_some_and(|identity| {
            !presentation.subjects.iter().any(|subject| {
                &subject.identity == identity && subject.role == PresentationRole::Body
            })
        }) {
            return Err(EntranceRefusal::MissingBody);
        }
        let selected_subject = here_part(presentation)
            .or(body_subject)
            .or_else(|| this_host(presentation))
            .ok_or(EntranceRefusal::InvalidPresentation)?;
        Ok(Self {
            body_id: presentation.basis.body_id.clone(),
            presentation_id: presentation.identity.as_str().into(),
            presentation_revision: presentation.revision,
            layer: EntranceLayer::World,
            available_actions: actions_for(presentation, Some(&selected_subject)),
            selected_subject: Some(selected_subject),
            last_refusal: None,
        })
    }

    pub fn update(
        &mut self,
        presentation: &Presentation,
    ) -> Result<EntranceUpdateDisposition, EntranceRefusal> {
        presentation
            .validate()
            .map_err(|_| EntranceRefusal::InvalidPresentation)?;
        if presentation.basis.body_id != self.body_id {
            return self.refuse(EntranceRefusal::WrongBody);
        }
        if presentation.revision <= self.presentation_revision {
            return self.refuse(EntranceRefusal::StaleRevision);
        }
        let disposition = match self.selected_subject.as_deref() {
            None => EntranceUpdateDisposition::SelectionPreserved,
            Some(subject) if has_subject(presentation, subject) => {
                EntranceUpdateDisposition::SelectionPreserved
            }
            Some(_) => {
                self.selected_subject = Some(
                    here_part(presentation)
                        .or_else(|| {
                            self.body_id
                                .as_ref()
                                .map(|body_id| format!("body/{}", body_id.as_str()))
                        })
                        .or_else(|| this_host(presentation))
                        .ok_or(EntranceRefusal::InvalidPresentation)?,
                );
                EntranceUpdateDisposition::SelectionBecameStale
            }
        };
        self.presentation_id = presentation.identity.as_str().into();
        self.presentation_revision = presentation.revision;
        self.available_actions = actions_for(presentation, self.selected_subject.as_deref());
        self.last_refusal = None;
        Ok(disposition)
    }

    pub fn select(
        &mut self,
        presentation: &Presentation,
        subject: &str,
    ) -> Result<(), EntranceRefusal> {
        self.require_current(presentation)?;
        if !has_subject(presentation, subject) {
            return self.refuse(EntranceRefusal::UnknownSubject);
        }
        self.selected_subject = Some(subject.into());
        self.available_actions = actions_for(presentation, Some(subject));
        self.last_refusal = None;
        Ok(())
    }

    pub fn clear_selection(&mut self, presentation: &Presentation) -> Result<(), EntranceRefusal> {
        self.require_current(presentation)?;
        self.selected_subject = None;
        self.available_actions.clear();
        self.layer = EntranceLayer::World;
        self.last_refusal = None;
        Ok(())
    }

    pub fn show_layer(
        &mut self,
        presentation: &Presentation,
        layer: EntranceLayer,
    ) -> Result<(), EntranceRefusal> {
        self.require_current(presentation)?;
        let available = match layer {
            EntranceLayer::World => true,
            EntranceLayer::Intent => presentation
                .subjects
                .iter()
                .any(|subject| subject.role == PresentationRole::Form),
            EntranceLayer::Realization => presentation.basis.plan_id.is_some(),
        };
        if !available {
            return self.refuse(EntranceRefusal::LayerUnavailable);
        }
        self.layer = layer;
        self.last_refusal = None;
        Ok(())
    }

    fn require_current(&mut self, presentation: &Presentation) -> Result<(), EntranceRefusal> {
        if presentation.validate().is_err() {
            return self.refuse(EntranceRefusal::InvalidPresentation);
        }
        if presentation.basis.body_id != self.body_id {
            return self.refuse(EntranceRefusal::WrongBody);
        }
        if presentation.revision != self.presentation_revision
            || presentation.identity.as_str() != self.presentation_id
        {
            return self.refuse(EntranceRefusal::StaleRevision);
        }
        Ok(())
    }

    fn refuse<T>(&mut self, refusal: EntranceRefusal) -> Result<T, EntranceRefusal> {
        self.last_refusal = Some(refusal.clone());
        Err(refusal)
    }
}

fn here_part(presentation: &Presentation) -> Option<String> {
    presentation.properties.iter().find_map(|property| {
        (property.name == "membership-state"
            && property.value == PresentationPropertyValue::Text("here".into()))
        .then(|| property.subject.clone())
    })
}

fn this_host(presentation: &Presentation) -> Option<String> {
    presentation.properties.iter().find_map(|property| {
        (property.name == "this-host" && property.value == PresentationPropertyValue::Flag(true))
            .then(|| property.subject.clone())
    })
}

fn has_subject(presentation: &Presentation, identity: &str) -> bool {
    presentation
        .subjects
        .iter()
        .any(|subject| subject.identity == identity)
}

fn actions_for(presentation: &Presentation, identity: Option<&str>) -> Vec<EntranceAction> {
    let role = identity.and_then(|identity| {
        presentation
            .subjects
            .iter()
            .find(|subject| subject.identity == identity)
            .map(|subject| subject.role)
    });
    let actions = match role {
        Some(PresentationRole::Body)
            if identity
                .is_some_and(|identity| property_flag(presentation, identity, "current")) =>
        {
            &[EntranceAction::Inspect, EntranceAction::OpenForms][..]
        }
        Some(PresentationRole::Body)
            if identity.is_some_and(|identity| property_flag(presentation, identity, "opened")) =>
        {
            &[EntranceAction::Inspect, EntranceAction::Join]
        }
        Some(PresentationRole::Body) => &[EntranceAction::Inspect, EntranceAction::Open],
        Some(PresentationRole::Seed)
            if identity.is_some_and(|identity| property_flag(presentation, identity, "opened")) =>
        {
            &[EntranceAction::Inspect, EntranceAction::BeBorn]
        }
        Some(PresentationRole::Seed) => &[EntranceAction::Inspect, EntranceAction::Open],
        Some(PresentationRole::Candidate) => &[
            EntranceAction::Inspect,
            EntranceAction::Admit,
            EntranceAction::Refuse,
        ],
        Some(PresentationRole::Plan) => &[EntranceAction::Inspect, EntranceAction::Replan],
        Some(_) => &[EntranceAction::Inspect],
        None => &[],
    };
    debug_assert!(actions.len() <= MAX_ENTRANCE_ACTIONS);
    actions.to_vec()
}

fn property_flag(presentation: &Presentation, identity: &str, name: &str) -> bool {
    presentation.properties.iter().any(|property| {
        property.subject == identity
            && property.name == name
            && property.value == PresentationPropertyValue::Flag(true)
    })
}
