//! Bounded zero-Body Patchbay state for an ordinary ConduitOS boot.

use alloc::{format, string::String, vec};
use conduit_core::{BootId, CheckedFormId, HostId, OfferGeneration, SourceDocumentId};
use conduit_human::KeyEvent;
use conduit_presentation::{
    Presentation, PresentationAction, PresentationActionRefusal, PresentationBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};

use crate::display::DisplayError;
use crate::product_journey::{JourneyProjection, JourneyStatus};

#[cfg(any(test, feature = "native-compositor"))]
mod presenter;
mod scene;
mod semantics;
#[cfg(any(test, feature = "native-compositor"))]
pub use presenter::{FrontDoorPresenter, PresenterError};
use semantics::lifecycle_summary;

const ENTER: u8 = 40;
const ESCAPE: u8 = 41;
const TAB: u8 = 43;
const F2: u8 = 59;
const RIGHT: u8 = 79;
const LEFT: u8 = 80;
const DOWN: u8 = 81;
const UP: u8 = 82;

pub struct FrontDoor {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    profile_id: String,
    build_id: String,
    image_id: String,
    source_document_id: SourceDocumentId,
    checked_form_id: CheckedFormId,
    form_subject: String,
    selected_subject: String,
    exact_details_open: bool,
    form_open: bool,
    revision: u64,
    offer_count: u64,
    lifecycle_authority_admitted: bool,
    details_page: u8,
    journey: Option<JourneyProjection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    StaleInput,
    StaleAction,
    UnknownAction,
    ActionUnavailable,
    ActionRefused,
    Presentation,
    Display(DisplayError),
    Scene,
}

impl Error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaleInput => "front-door-input-stale",
            Self::StaleAction => "front-door-action-stale",
            Self::UnknownAction => "front-door-action-unknown",
            Self::ActionUnavailable => "front-door-action-unavailable",
            Self::ActionRefused => "front-door-action-refused",
            Self::Presentation => "front-door-presentation-refused",
            Self::Display(error) => error.as_str(),
            Self::Scene => "front-door-scene-refused",
        }
    }
}

impl FrontDoor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile_id: impl Into<String>,
        build_id: impl Into<String>,
        image_id: impl Into<String>,
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        offer_count: u64,
        lifecycle_authority_admitted: bool,
    ) -> Self {
        let form_subject = format!("form/{}", checked_form_id.as_str());
        let selected_subject = form_subject.clone();
        Self {
            host_id,
            boot_id,
            offer_generation,
            profile_id: profile_id.into(),
            build_id: build_id.into(),
            image_id: image_id.into(),
            source_document_id,
            checked_form_id,
            form_subject,
            selected_subject,
            exact_details_open: false,
            form_open: false,
            revision: 1,
            offer_count,
            lifecycle_authority_admitted,
            details_page: 0,
            journey: None,
        }
    }

    pub const fn exact_details_open(&self) -> bool {
        self.exact_details_open
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn observe_journey(&mut self, projection: JourneyProjection) -> Result<(), Error> {
        if projection.source_document_id != self.source_document_id
            || projection.checked_form_id != self.checked_form_id
        {
            return Err(Error::Presentation);
        }
        self.form_open = projection.status == JourneyStatus::FormOpened;
        self.journey = Some(projection);
        self.advance()
    }

    pub fn accept(&mut self, event: KeyEvent, revision: u64) -> Result<bool, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        if event.transition() != conduit_human::KeyTransition::Pressed {
            return Ok(false);
        }
        match event.usage() {
            TAB | RIGHT | LEFT | DOWN | UP => {
                let form = self.form_subject.clone();
                let host = format!("host/{}/{}", self.host_id.as_str(), self.boot_id.as_str());
                self.selected_subject = if self.selected_subject == form {
                    host
                } else {
                    form
                };
                self.exact_details_open = false;
                self.advance()?;
                Ok(true)
            }
            F2 => {
                if self.exact_details_open {
                    self.details_page = (self.details_page + 1) % 16;
                }
                self.exact_details_open = true;
                self.advance()?;
                Ok(true)
            }
            ENTER => {
                self.form_open = self.selected_subject.starts_with("form/");
                self.exact_details_open = !self.form_open;
                self.advance()?;
                Ok(true)
            }
            ESCAPE => {
                self.exact_details_open = false;
                self.advance()?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn advance(&mut self) -> Result<(), Error> {
        self.revision = self.revision.checked_add(1).ok_or(Error::Presentation)?;
        Ok(())
    }

    pub fn presentation(&self) -> Result<Presentation, Error> {
        let host = format!("host/{}/{}", self.host_id.as_str(), self.boot_id.as_str());
        let form = self.form_subject.clone();
        let mut subjects = vec![
            PresentationSubject {
                identity: host.clone(),
                role: PresentationRole::Host,
                label: "This Host".into(),
                accessibility_name: "This Host; current Body none".into(),
            },
            PresentationSubject {
                identity: form.clone(),
                role: PresentationRole::Form,
                label: "ConduitOS entrance Form".into(),
                accessibility_name: "Openable checked IMAGE Form; opening is inert".into(),
            },
        ];
        let mut relationships = vec![PresentationRelationship {
            source: host.clone(),
            target: form.clone(),
            kind: PresentationRelationshipKind::Observes,
        }];
        let mut properties = vec![
            property(
                &host,
                "current-body",
                PresentationPropertyValue::Text("none".into()),
            ),
            property(&host, "host-id", identity(self.host_id.as_str())),
            property(&host, "boot-id", identity(self.boot_id.as_str())),
            property(
                &host,
                "offer-generation",
                PresentationPropertyValue::Count(self.offer_generation.0),
            ),
            property(
                &host,
                "offer-count",
                PresentationPropertyValue::Count(self.offer_count),
            ),
            property(
                &form,
                "source-document-id",
                identity(self.source_document_id.as_str()),
            ),
            property(
                &form,
                "checked-form-id",
                identity(self.checked_form_id.as_str()),
            ),
            property(
                &form,
                "opened",
                PresentationPropertyValue::Flag(self.form_open),
            ),
        ];
        if self.exact_details_open {
            properties.extend([
                property(&host, "profile-id", identity(&self.profile_id)),
                property(&host, "build-id", identity(&self.build_id)),
                property(&host, "image-id", identity(&self.image_id)),
            ]);
        }
        if let Some(journey) = &self.journey {
            if let Some(body_id) = &journey.body_id {
                let body = format!("body/{}", body_id.as_str());
                subjects.push(PresentationSubject {
                    identity: body.clone(),
                    role: PresentationRole::Body,
                    label: "Current Body".into(),
                    accessibility_name: format!("Current Body; {:?}", journey.status),
                });
                relationships.push(PresentationRelationship {
                    source: host.clone(),
                    target: body.clone(),
                    kind: PresentationRelationshipKind::Contains,
                });
                properties.push(property(&body, "body-id", identity(body_id.as_str())));
                if let Some(born_sign_id) = &journey.born_sign_id {
                    properties.push(property(
                        &body,
                        "born-sign-id",
                        identity(born_sign_id.as_str()),
                    ));
                }
                if let Some(part_id) = &journey.part_id {
                    properties.push(property(&body, "part-id", identity(part_id.as_str())));
                }
            }
            properties.push(property(
                &form,
                "expanded-form-id",
                identity(journey.expanded_form_id.as_str()),
            ));
            if let Some(wake_id) = &journey.wake_id {
                properties.push(property(&host, "wake-id", identity(wake_id.as_str())));
            }
            if let Some(plan_id) = &journey.plan_id {
                properties.push(property(&host, "plan-id", identity(plan_id.as_str())));
            }
            if let Some(play_id) = &journey.active_play_id {
                properties.push(property(
                    &host,
                    "active-play-id",
                    identity(play_id.as_str()),
                ));
            }
            if let Some(result) = &journey.result {
                properties.push(property(
                    &host,
                    "semantic-result",
                    PresentationPropertyValue::Text(result.clone()),
                ));
            }
        }
        let basis = self.journey.as_ref().map_or_else(
            || PresentationBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_form_id: None,
                expanded_form_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            |journey| {
                if journey.body_id.is_some() && journey.wake_id.is_some() {
                    PresentationBasis {
                        body_id: journey.body_id.clone(),
                        wake_id: journey.wake_id.clone(),
                        source_document_id: Some(self.source_document_id.clone()),
                        checked_form_id: Some(self.checked_form_id.clone()),
                        expanded_form_id: Some(journey.expanded_form_id.clone()),
                        plan_id: journey.plan_id.clone(),
                        active_play_id: journey.active_play_id.clone(),
                        sign_ids: journey
                            .input_sign_id
                            .iter()
                            .chain(journey.result_sign_id.iter())
                            .cloned()
                            .collect(),
                    }
                } else {
                    PresentationBasis {
                        body_id: None,
                        wake_id: None,
                        source_document_id: None,
                        checked_form_id: None,
                        expanded_form_id: None,
                        plan_id: None,
                        active_play_id: None,
                        sign_ids: vec![],
                    }
                }
            },
        );
        let host_text = self.journey.as_ref().map_or_else(
            || "BODY NONE; entering Patchbay creates no Body, Wake, Plan, or Play".into(),
            |journey| lifecycle_summary(journey).into(),
        );
        Presentation::new_with_semantics(
            self.revision,
            basis,
            subjects,
            relationships,
            properties,
            vec![
                PresentationText {
                    subject: host.clone(),
                    text: host_text,
                },
                PresentationText {
                    subject: form.clone(),
                    text: "IMAGE-embedded checked Form; OPEN permits inspection only".into(),
                },
            ],
            self.semantic_actions(&form),
            vec![
                PresentationDisclosure {
                    subject: form,
                    level: PresentationDisclosureLevel::Primary,
                },
                PresentationDisclosure {
                    subject: host,
                    level: PresentationDisclosureLevel::Context,
                },
            ],
        )
        .map_err(|_| Error::Presentation)
    }

    pub fn resolve_action(
        &self,
        action: patchbay_control::PatchbayAction,
        presentation_revision: u64,
    ) -> Result<PresentationAction, Error> {
        let presentation = self.presentation()?;
        let semantic = presentation
            .actions
            .iter()
            .find(|candidate| candidate.intent == action.presentation_intent())
            .ok_or(Error::Presentation)?;
        presentation
            .resolve_action(presentation_revision, &semantic.identity)
            .cloned()
            .map_err(|error| match error {
                PresentationActionRefusal::StaleRevision => Error::StaleAction,
                PresentationActionRefusal::UnknownAction => Error::UnknownAction,
                PresentationActionRefusal::Unavailable { .. } => Error::ActionUnavailable,
                PresentationActionRefusal::Refused { .. } => Error::ActionRefused,
            })
    }
}

fn identity(value: &str) -> PresentationPropertyValue {
    PresentationPropertyValue::Identity(value.into())
}

fn property(subject: &str, name: &str, value: PresentationPropertyValue) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}

#[cfg(test)]
mod tests;
