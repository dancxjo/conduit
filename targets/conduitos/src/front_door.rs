//! Bounded zero-body Patchbay state for an ordinary ConduitOS boot.

use alloc::{format, string::String, vec};
use conduit_core::{BootId, CheckedPlotId, HostId, OfferGeneration, SourceDocumentId};
use conduit_face::{
    Face, FaceAction, FaceActionRefusal, FaceBasis, FaceDisclosure, FaceDisclosureLevel,
    FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    FaceText,
};
use conduit_human::KeyEvent;

use crate::display::DisplayError;
use crate::product_journey::{JourneyProjection, JourneyStatus};

#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
mod application_layout;
mod arrival;
mod entrance_facts;
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
mod graph_facts;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
mod graph_projection;
mod home;
mod home_projection;
mod joining;
#[cfg(any(test, feature = "native-compositor"))]
mod presenter;
mod projection;
mod reading;
mod scene;
mod workspace;
mod workspace_mask;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
mod workspace_scene;
pub use arrival::ArrivalInput;
pub use home::{HomeInput, HomeView};
pub use joining::PendingJoinView;
mod semantics;
#[cfg(any(test, feature = "native-compositor"))]
pub use presenter::{FrontDoorPresenter, PresenterError};
use semantics::lifecycle_summary;

#[cfg(test)]
const ENTER: u8 = 40;
#[cfg(test)]
const ESCAPE: u8 = 41;
#[cfg(test)]
const TAB: u8 = 43;
#[cfg(test)]
const F2: u8 = 59;
#[cfg(test)]
const RIGHT: u8 = 79;
#[cfg(test)]
const LEFT: u8 = 80;
#[cfg(test)]
const DOWN: u8 = 81;
#[cfg(test)]
const UP: u8 = 82;

pub struct FrontDoor {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    profile_id: String,
    build_id: String,
    image_id: String,
    source_document_id: SourceDocumentId,
    checked_plot_id: CheckedPlotId,
    plot_subject: String,
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    reading: conduitos_mask_core::control::View,
    revision: u64,
    offer_count: u64,
    lifecycle_authority_admitted: bool,
    journey: Option<JourneyProjection>,
    connectivity: Option<ConnectivityProjection>,
    arrival: Option<arrival::Arrival>,
    joining: Option<PendingJoinView>,
    home: Option<home::Home>,
    refusal: Option<workspace::WorkspaceRefusal>,
    workspace: Option<crate::product_journey::WorkspaceProjection>,
    application_view: Option<conduit_face::ApplicationView>,
    patchbay_graph: Option<patchbay_graph::PatchbayGraph>,
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    graph_facts: Option<conduitos_application_core::graph_facts::Contribution>,
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    application_viewport: application_layout::ApplicationViewport,
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    workspace_mask: core::cell::RefCell<crate::protected_workspace_mask::Owner>,
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
        self.mask_status().label()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    StaleInput,
    StaleAction,
    UnknownAction,
    ActionUnavailable,
    ActionRefused,
    Face,
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
            Self::Face => "front-door-face-refused",
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
        checked_plot_id: CheckedPlotId,
        offer_count: u64,
        lifecycle_authority_admitted: bool,
    ) -> Self {
        let plot_subject = format!("plot/{}", checked_plot_id.as_str());
        Self {
            host_id,
            boot_id,
            offer_generation,
            profile_id: profile_id.into(),
            build_id: build_id.into(),
            image_id: image_id.into(),
            source_document_id,
            checked_plot_id,
            plot_subject,
            #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
            reading: conduitos_mask_core::control::View::default(),
            revision: 1,
            offer_count,
            lifecycle_authority_admitted,
            journey: None,
            connectivity: None,
            arrival: None,
            joining: None,
            home: None,
            refusal: None,
            workspace: None,
            application_view: None,
            patchbay_graph: None,
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            graph_facts: None,
            #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
            application_viewport: application_layout::ApplicationViewport::default(),
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            workspace_mask: core::cell::RefCell::new(
                crate::protected_workspace_mask::Owner::default(),
            ),
        }
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn observe_journey(&mut self, projection: JourneyProjection) -> Result<(), Error> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            self.graph_facts = None;
        }
        if projection
            .source_document_id
            .as_ref()
            .is_some_and(|identity| identity != &self.source_document_id)
            || projection
                .checked_plot_id
                .as_ref()
                .is_some_and(|identity| identity != &self.checked_plot_id)
        {
            return Err(Error::Face);
        }
        self.revision.checked_add(1).ok_or(Error::Face)?;
        self.reading_command(conduitos_mask_core::control::Operation::ObservePlot {
            opened: projection.status == JourneyStatus::PlotOpened,
        })?;
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
            return Err(Error::Face);
        }
        self.connectivity = Some(projection);
        self.advance()
    }

    pub fn accept(&mut self, event: KeyEvent, revision: u64) -> Result<bool, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        // The outer revision remains Root's pre-effect fence. Reserve it before
        // asking the guest to mutate any presentation state.
        let operation = conduitos_mask_core::control::Operation::Key(event.encode());
        if operation.changes().map_err(|_| Error::Face)? {
            self.revision.checked_add(1).ok_or(Error::Face)?;
        }
        let changed = self.reading_command(operation)?;
        if changed {
            self.advance()?;
        }
        Ok(changed)
    }

    pub fn startup_refused(&mut self, reason: &str) -> Result<(), Error> {
        self.refusal = Some(workspace::WorkspaceRefusal::Startup(reason.into()));
        self.advance()
    }

    fn advance(&mut self) -> Result<(), Error> {
        self.revision = self.revision.checked_add(1).ok_or(Error::Face)?;
        Ok(())
    }

    pub fn resolve_action(
        &self,
        action: patchbay_control::PatchbayAction,
        face_revision: u64,
    ) -> Result<FaceAction, Error> {
        let face = self.face()?;
        let semantic = face
            .actions
            .iter()
            .find(|candidate| candidate.intent == action.face_intent())
            .ok_or(Error::Face)?;
        face.resolve_action(face_revision, &semantic.identity)
            .cloned()
            .map_err(|error| match error {
                FaceActionRefusal::StaleRevision => Error::StaleAction,
                FaceActionRefusal::UnknownAction => Error::UnknownAction,
                FaceActionRefusal::Unavailable { .. } => Error::ActionUnavailable,
                FaceActionRefusal::Refused { .. } => Error::ActionRefused,
            })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod shell_audit_tests;
