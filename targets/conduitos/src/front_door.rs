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

mod arrival;
#[cfg(any(test, feature = "native-compositor"))]
mod presenter;
mod projection;
mod scene;
mod workspace;
mod workspace_scene;
pub use arrival::ArrivalInput;
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
    connectivity: Option<ConnectivityProjection>,
    arrival: Option<arrival::Arrival>,
    startup_refusal: Option<String>,
    workspace: Option<crate::product_journey::WorkspaceProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectivityProjection {
    pub line_id: String,
    pub status: ConnectivityStatus,
    pub value: Option<String>,
    pub body_id: conduit_body::BodyId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectivityStatus {
    Current,
    PeerAttached,
    ValueVisible,
    Lost,
}

impl ConnectivityStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Current => "USB LINE CURRENT",
            Self::PeerAttached => "PEER ATTACHED / MEMBERSHIP NOT REQUESTED",
            Self::ValueVisible => "LINE VALUE VISIBLE",
            Self::Lost => "USB LINE LOST / STALE SESSION REFUSED",
        }
    }
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
            connectivity: None,
            arrival: None,
            startup_refusal: None,
            workspace: None,
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

    pub fn observe_connectivity(
        &mut self,
        projection: ConnectivityProjection,
    ) -> Result<(), Error> {
        if self
            .journey
            .as_ref()
            .and_then(|journey| journey.body_id.as_ref())
            != Some(&projection.body_id)
            || projection.line_id.is_empty()
        {
            return Err(Error::Presentation);
        }
        self.connectivity = Some(projection);
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

    pub fn startup_refused(&mut self, reason: &str) -> Result<(), Error> {
        self.startup_refusal = Some(reason.into());
        self.advance()
    }

    fn advance(&mut self) -> Result<(), Error> {
        self.revision = self.revision.checked_add(1).ok_or(Error::Presentation)?;
        Ok(())
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

#[cfg(test)]
mod tests;
