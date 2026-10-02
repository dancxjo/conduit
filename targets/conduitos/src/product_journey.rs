//! Canonical bounded Body/Wake/Plan/Play state for the ordinary product entrance.

use alloc::{borrow::ToOwned, boxed::Box, format, string::String, vec::Vec};

use conduit_body::{
    Body, BodyLifecycleSession, BodyLifecycleSessionError, BodyMembership, BodyPlan,
    BodyPlayIdentity, BodyState, MembershipProofId, PartId, Wake, WakeLifecycleEvent,
};
use conduit_core::{AuthorityGrantId, BootId, ExpandedPlotId, HostId, OfferGeneration, SignId};

use crate::{
    identity::BootIdentities,
    keyboard_text_plan::{self, KeyboardTextPlotIdentity},
    native_workset::{self, NativePlot, NativeWorksetPlay},
    offer::HostOffer,
    ordinary_plan::PreparationError,
};

mod biography;
mod birth;
mod projection;
mod proposal;
mod tutorial;
pub use tutorial::{TutorialRefusal, TutorialSurface};
mod result_window;
use result_window::ResultWindow;
mod play;
mod workset;
use workset::PlotResult;
pub use workset::{WorkspacePlot, WorkspaceProjection};

pub use patchbay_control::{
    PatchbayAction as JourneyAction, PatchbayControlRequest as JourneyRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyStatus {
    World,
    PlotOpened,
    BornLulled,
    Awake,
    Planned,
    QuiescentAwaitingInput,
    SemanticCompleted,
    Lulled,
    Fulfilled,
    InputUnavailable,
    Stopped,
}

impl JourneyStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::World => "world",
            Self::PlotOpened => "plot-opened",
            Self::BornLulled => "born-lulled",
            Self::Awake => "awake",
            Self::Planned => "planned",
            Self::QuiescentAwaitingInput => "quiescent-awaiting-input",
            Self::SemanticCompleted => "semantic-completed",
            Self::Lulled => "lulled",
            Self::Fulfilled => "fulfilled",
            Self::InputUnavailable => "input-unavailable",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyLossKind {
    InputDevice,
    Line,
}

impl JourneyLossKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputDevice => "input-device-lost",
            Self::Line => "line-lost",
        }
    }

    pub const fn recovery(self) -> &'static str {
        match self {
            Self::InputDevice => "Reconnect an input device, then Plan and Play again.",
            Self::Line => "Restore or replace the Line, then Plan and Play again.",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyError {
    StalePresentation,
    WrongTarget,
    PlotNotOpened,
    AlreadyBorn,
    BodyAbsent,
    InvalidTransition,
    Biography(conduit_body::BodyBiographyError),
    Membership,
    AdmissionUnsupported,
    Lifecycle(BodyLifecycleSessionError),
    Plan(PreparationError),
    Workset(native_workset::WorksetRefusal),
    Play(native_workset::PlayRefusal),
    Kernel,
    InputUnavailable,
    InputSequenceExhausted,
    RevisionExhausted,
    Mask(native_workset::PlayRefusal),
}

impl JourneyError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StalePresentation => "product-interaction-stale-presentation",
            Self::WrongTarget => "product-interaction-wrong-target",
            Self::PlotNotOpened => "product-birth-plot-not-open",
            Self::AlreadyBorn => "product-birth-duplicate",
            Self::BodyAbsent => "product-body-absent",
            Self::InvalidTransition => "product-lifecycle-transition-refused",
            Self::Biography(conduit_body::BodyBiographyError::CapacityExhausted) => {
                "product-biography-capacity-exhausted"
            }
            Self::Biography(_) => "product-biography-evidence-refused",
            Self::AdmissionUnsupported => "product-peer-authenticated-admission-unsupported",
            Self::Lifecycle(BodyLifecycleSessionError::ArchivePersistenceRequired) => {
                "product-biography-archive-persistence-required"
            }
            Self::Lifecycle(_) => "product-canonical-lifecycle-refused",
            Self::Membership => "product-birth-membership-refused",
            Self::Plan(error) => error.as_str(),
            Self::Workset(error) => error.as_str(),
            Self::Play(error) => error.as_str(),
            Self::Kernel => "product-kernel-refused",
            Self::InputUnavailable => "product-input-unavailable",
            Self::InputSequenceExhausted => "product-input-sequence-exhausted",
            Self::RevisionExhausted => "product-presentation-revision-exhausted",
            Self::Mask(error) => error.as_str(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyProjection {
    pub status: JourneyStatus,
    pub revision: u64,
    pub source_document_id: Option<conduit_core::SourceDocumentId>,
    pub checked_plot_id: Option<conduit_core::CheckedPlotId>,
    pub expanded_plot_id: Option<ExpandedPlotId>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub body_id: Option<conduit_body::BodyId>,
    pub friendly_name: Option<String>,
    pub born_sign_id: Option<SignId>,
    pub fulfilled_sign_id: Option<SignId>,
    pub workload_revision: Option<u64>,
    pub workload_sign_id: Option<SignId>,
    pub lull_sign_id: Option<SignId>,
    pub workload_capacity_available: bool,
    pub part_id: Option<PartId>,
    pub wake_id: Option<conduit_body::WakeId>,
    pub wake_sign_id: Option<SignId>,
    pub plan_id: Option<conduit_core::PlanId>,
    pub plan_sign_id: Option<SignId>,
    pub active_play_id: Option<conduit_core::ActivePlayId>,
    pub play_sign_id: Option<SignId>,
    pub gear_ids: Vec<String>,
    pub port_ids: Vec<String>,
    pub cord_ids: Vec<String>,
    pub input_sign_id: Option<SignId>,
    pub loss_kind: Option<JourneyLossKind>,
    pub loss_sign_id: Option<SignId>,
    pub result_sign_id: Option<SignId>,
    pub result: Option<String>,
    pub result_omitted_bytes: u64,
    pub input_count: u32,
    pub kernel_sign_gap: Option<conduit_kernel::SignRetentionGap>,
    pub last_request_id: Option<String>,
    pub mask: Option<crate::mask_control::NativeMaskEvidence>,
}

pub struct ProductJourney {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    plot: Option<KeyboardTextPlotIdentity>,
    status: JourneyStatus,
    revision: u64,
    request_sequence: u64,
    session: Option<BodyLifecycleSession>,
    kernel: Option<Box<NativeWorksetPlay>>,
    last_working_plot: Option<conduit_body::ResidentPlot>,
    plots: [Option<NativePlot>; native_workset::NATIVE_PLOT_CAPACITY],
    input_owners: [Option<native_workset::AdmittedPlotInput>; native_workset::NATIVE_PLOT_CAPACITY],
    input_count: u32,
    input_sign_id: Option<SignId>,
    loss_kind: Option<JourneyLossKind>,
    loss_sign_id: Option<SignId>,
    results: [PlotResult; native_workset::NATIVE_PLOT_CAPACITY],
    retained_kernel_sign_gap: Option<conduit_kernel::SignRetentionGap>,
    last_request_id: Option<String>,
    application_request: Option<native_workset::NativeApplicationRequest>,
    mask_control: Option<crate::mask_control::MaskControl>,
    surface_provider: Option<crate::native_surface_provider::NativeSurfaceProvider>,
}

impl ProductJourney {
    /// A proof identifier is not an authenticated admission. The native Line
    /// adapter must supply the canonical invitation exchange before joining.
    pub fn admit_line_peer(
        &mut self,
        _host_id: HostId,
        _boot_id: BootId,
        _proof_id: MembershipProofId,
    ) -> Result<PartId, JourneyError> {
        Err(JourneyError::AdmissionUnsupported)
    }

    pub fn observe_line_peer_offline(
        &mut self,
        part: &PartId,
        boot: &BootId,
    ) -> Result<(), JourneyError> {
        let host = self
            .biography()
            .ok_or(JourneyError::BodyAbsent)?
            .membership
            .parts
            .iter()
            .find(|member| &member.part_id == part)
            .and_then(|member| member.current.as_ref())
            .filter(|current| &current.boot_id == boot)
            .map(|current| current.host_id.clone())
            .ok_or(JourneyError::Membership)?;
        self.session
            .as_mut()
            .ok_or(JourneyError::BodyAbsent)?
            .observe_host_lost(&host, boot, &self.host_id, &self.boot_id)
            .map_err(JourneyError::Lifecycle)?;
        self.refresh_tutorial()?;
        self.advance()
    }

    fn body(&self) -> Option<&Body> {
        self.biography().map(|evidence| &evidence.body)
    }
    fn current_wake(&self) -> Option<&Wake> {
        self.session
            .as_ref()?
            .realization()
            .map(|current| &current.wake)
    }
    fn current_plan(&self) -> Option<&BodyPlan> {
        self.session
            .as_ref()?
            .realization()
            .map(|current| &current.plan)
    }
    fn current_play(&self) -> Option<&BodyPlayIdentity> {
        self.session.as_ref()?.realization()?.play.as_ref()
    }
    fn foreground_index(&self) -> usize {
        self.session
            .as_ref()
            .and_then(|session| {
                session
                    .evidence()
                    .body
                    .workset
                    .plots()
                    .iter()
                    .position(|plot| Some(plot) == session.foreground())
            })
            .unwrap_or(0)
    }

    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, JourneyError> {
        Ok(Self {
            host_id,
            boot_id,
            offer_generation,
            plot: None,
            status: JourneyStatus::World,
            revision: 1,
            request_sequence: 0,
            session: None,
            kernel: None,
            last_working_plot: None,
            plots: [None; native_workset::NATIVE_PLOT_CAPACITY],
            input_owners: core::array::from_fn(|_| None),
            input_count: 0,
            input_sign_id: None,
            loss_kind: None,
            loss_sign_id: None,
            results: core::array::from_fn(|_| PlotResult::new()),
            retained_kernel_sign_gap: None,
            last_request_id: None,
            application_request: None,
            mask_control: None,
            surface_provider: None,
        })
    }

    pub fn admit_surface_provider(
        &mut self,
        provider: crate::native_surface_provider::NativeSurfaceProvider,
    ) {
        self.surface_provider = Some(provider);
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn status(&self) -> JourneyStatus {
        self.status
    }

    pub fn next_request(
        &mut self,
        action: JourneyAction,
        target_identity: impl Into<String>,
        presentation_revision: u64,
    ) -> Result<JourneyRequest, JourneyError> {
        let sequence = self.request_sequence;
        self.request_sequence = self
            .request_sequence
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        JourneyRequest::new(
            format!(
                "conduitos/product-interaction/{}/{sequence}",
                action.as_str()
            ),
            format!("conduitos/product-presentation/{presentation_revision}"),
            presentation_revision,
            format!("action/{}/{presentation_revision}", action.as_str()),
            action,
            target_identity,
        )
        .map_err(|_| JourneyError::WrongTarget)
    }

    pub fn apply(
        &mut self,
        request: JourneyRequest,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        build_id: &str,
        current_presentation_revision: u64,
    ) -> Result<(), JourneyError> {
        if request.presentation_revision != current_presentation_revision {
            return Err(JourneyError::StalePresentation);
        }
        self.validate_target(&request)?;
        match request.action {
            JourneyAction::OpenBack => self.open_plot()?,
            JourneyAction::Birth => self.birth()?,
            JourneyAction::Wake => self.wake(identities, offer, build_id)?,
            JourneyAction::Plan => self.plan(identities, offer, build_id)?,
            JourneyAction::Play => self.play()?,
            JourneyAction::Stop => self.stop()?,
            JourneyAction::Lull => self.lull()?,
            JourneyAction::Fulfill => self.fulfill()?,
            JourneyAction::AdmitPlot => self.admit_next_plot(identities, offer, build_id)?,
            _ => return Err(JourneyError::WrongTarget),
        }
        self.last_request_id = Some(request.request_id);
        self.advance()
    }

    fn validate_target(&self, request: &JourneyRequest) -> Result<(), JourneyError> {
        let expected = match request.action {
            JourneyAction::OpenBack => format!(
                "plot/{}",
                keyboard_text_plan::checked_plot_identity()
                    .map_err(JourneyError::Plan)?
                    .checked_plot_id
                    .as_str()
            ),
            JourneyAction::Birth => format!(
                "plot/{}",
                self.plot
                    .as_ref()
                    .ok_or(JourneyError::PlotNotOpened)?
                    .checked_plot_id
                    .as_str()
            ),
            JourneyAction::Wake
            | JourneyAction::Plan
            | JourneyAction::Play
            | JourneyAction::Stop
            | JourneyAction::Lull
            | JourneyAction::Fulfill
            | JourneyAction::AdmitPlot => self
                .body()
                .map(|body| format!("body/{}", body.body_id.as_str()))
                .ok_or(JourneyError::BodyAbsent)?,
            _ => return Err(JourneyError::WrongTarget),
        };
        if request.target_identity != expected {
            return Err(JourneyError::WrongTarget);
        }
        Ok(())
    }

    fn open_plot(&mut self) -> Result<(), JourneyError> {
        if self.body().is_some() {
            return Err(JourneyError::AlreadyBorn);
        }
        self.plot = Some(keyboard_text_plan::checked_plot_identity().map_err(JourneyError::Plan)?);
        self.status = JourneyStatus::PlotOpened;
        Ok(())
    }

    fn advance(&mut self) -> Result<(), JourneyError> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        Ok(())
    }

    fn fulfill(&mut self) -> Result<(), JourneyError> {
        if self.status != JourneyStatus::Lulled || self.kernel.is_some() {
            return Err(JourneyError::InvalidTransition);
        }
        let mut session = self.session.clone().ok_or(JourneyError::BodyAbsent)?;
        session
            .fulfill(
                &self.host_id,
                &self.boot_id,
                AuthorityGrantId::from("grant/conduitos/operator-fulfill"),
                "operator/conduitos-local-input".into(),
            )
            .map_err(JourneyError::Lifecycle)?;
        Self::require_archive_storage(&session)?;
        self.session = Some(session);
        self.status = JourneyStatus::Fulfilled;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod workset_tests;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tutorial_tests;

#[cfg(test)]
mod lifecycle_tests;
