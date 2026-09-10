//! Canonical bounded Body/Wake/Plan/Play state for the ordinary product entrance.

use alloc::{borrow::ToOwned, boxed::Box, format, string::String, vec::Vec};

use conduit_body::{Body, BodyMembership, BodyPlan, BodyPlayIdentity, BodyState, PartId, Wake};
use conduit_core::{BootId, ExpandedFormId, HostId, OfferGeneration, SignId};

use crate::{
    identity::BootIdentities,
    keyboard_text_plan::{self, KeyboardTextFormIdentity},
    native_workset::{self, NativeForm, NativeWorksetPlay},
    offer::HostOffer,
    ordinary_plan::PreparationError,
};

mod birth;
mod result_window;
use result_window::ResultWindow;
mod play;
mod workset;
use workset::FormResult;
pub use workset::{WorkspaceForm, WorkspaceProjection};

pub use patchbay_control::{
    PatchbayAction as JourneyAction, PatchbayControlRequest as JourneyRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyStatus {
    World,
    FormOpened,
    BornLulled,
    Awake,
    Planned,
    Playing,
    ResultVisible,
    Lulled,
    Stopped,
}

impl JourneyStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::World => "world",
            Self::FormOpened => "form-opened",
            Self::BornLulled => "born-lulled",
            Self::Awake => "awake",
            Self::Planned => "planned",
            Self::Playing => "playing",
            Self::ResultVisible => "result-visible",
            Self::Lulled => "lulled",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyError {
    StalePresentation,
    WrongTarget,
    FormNotOpened,
    AlreadyBorn,
    BodyAbsent,
    InvalidTransition,
    Membership,
    Plan(PreparationError),
    Workset(native_workset::WorksetRefusal),
    Play(native_workset::PlayRefusal),
    Kernel,
    InputUnavailable,
    InputSequenceExhausted,
    RevisionExhausted,
}

impl JourneyError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StalePresentation => "product-interaction-stale-presentation",
            Self::WrongTarget => "product-interaction-wrong-target",
            Self::FormNotOpened => "product-birth-form-not-open",
            Self::AlreadyBorn => "product-birth-duplicate",
            Self::BodyAbsent => "product-body-absent",
            Self::InvalidTransition => "product-lifecycle-transition-refused",
            Self::Membership => "product-birth-membership-refused",
            Self::Plan(error) => error.as_str(),
            Self::Workset(error) => error.as_str(),
            Self::Play(error) => error.as_str(),
            Self::Kernel => "product-kernel-refused",
            Self::InputUnavailable => "product-input-unavailable",
            Self::InputSequenceExhausted => "product-input-sequence-exhausted",
            Self::RevisionExhausted => "product-presentation-revision-exhausted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyProjection {
    pub status: JourneyStatus,
    pub revision: u64,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub body_id: Option<conduit_body::BodyId>,
    pub friendly_name: Option<String>,
    pub born_sign_id: Option<SignId>,
    pub part_id: Option<PartId>,
    pub wake_id: Option<conduit_body::WakeId>,
    pub plan_id: Option<conduit_core::PlanId>,
    pub active_play_id: Option<conduit_core::ActivePlayId>,
    pub gear_ids: Vec<String>,
    pub port_ids: Vec<String>,
    pub cord_ids: Vec<String>,
    pub input_sign_id: Option<SignId>,
    pub result_sign_id: Option<SignId>,
    pub result: Option<String>,
    pub result_omitted_bytes: u64,
    pub input_count: u32,
    pub kernel_sign_gap: Option<conduit_kernel::SignRetentionGap>,
    pub last_request_id: Option<String>,
}

pub struct ProductJourney {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    form: KeyboardTextFormIdentity,
    status: JourneyStatus,
    revision: u64,
    request_sequence: u64,
    body: Option<Body>,
    friendly_name: Option<String>,
    born_sign_id: Option<SignId>,
    membership: Option<BodyMembership>,
    part_id: Option<PartId>,
    wake: Option<Wake>,
    plan: Option<BodyPlan>,
    planned_play: Option<BodyPlayIdentity>,
    play: Option<BodyPlayIdentity>,
    kernel: Option<Box<NativeWorksetPlay>>,
    foreground: usize,
    forms: [Option<NativeForm>; 2],
    input_count: u32,
    input_sign_id: Option<SignId>,
    results: [FormResult; 2],
    retained_kernel_sign_gap: Option<conduit_kernel::SignRetentionGap>,
    last_request_id: Option<String>,
}

impl ProductJourney {
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, JourneyError> {
        let form = keyboard_text_plan::checked_form_identity().map_err(JourneyError::Plan)?;
        Ok(Self {
            host_id,
            boot_id,
            offer_generation,
            form,
            status: JourneyStatus::World,
            revision: 1,
            request_sequence: 0,
            body: None,
            friendly_name: None,
            born_sign_id: None,
            membership: None,
            part_id: None,
            wake: None,
            plan: None,
            planned_play: None,
            play: None,
            kernel: None,
            foreground: 0,
            forms: [None; 2],
            input_count: 0,
            input_sign_id: None,
            results: core::array::from_fn(|_| FormResult::new()),
            retained_kernel_sign_gap: None,
            last_request_id: None,
        })
    }

    pub fn form(&self) -> &KeyboardTextFormIdentity {
        &self.form
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
            JourneyAction::OpenBack => self.open_form()?,
            JourneyAction::Birth => self.birth()?,
            JourneyAction::Wake => self.wake()?,
            JourneyAction::Plan => self.plan(identities, offer, build_id)?,
            JourneyAction::Play => self.play()?,
            JourneyAction::Stop => self.stop()?,
            JourneyAction::Lull => self.lull()?,
            _ => return Err(JourneyError::WrongTarget),
        }
        self.last_request_id = Some(request.request_id);
        self.advance()
    }

    pub fn projection(&self) -> JourneyProjection {
        JourneyProjection {
            status: self.status,
            revision: self.revision,
            source_document_id: self.form.source_document_id.clone(),
            checked_form_id: self.form.checked_form_id.clone(),
            expanded_form_id: self.form.expanded_form_id.clone(),
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            body_id: self.body.as_ref().map(|body| body.body_id.clone()),
            born_sign_id: self.born_sign_id.clone(),
            friendly_name: self.friendly_name.clone(),
            part_id: self.part_id.clone(),
            wake_id: self.wake.as_ref().map(|wake| wake.wake_id.clone()),
            plan_id: self.plan.as_ref().map(|plan| plan.plan_id.clone()),
            active_play_id: self.play.as_ref().map(|play| play.active_play_id.clone()),
            gear_ids: self
                .plan
                .iter()
                .flat_map(|plan| &plan.forms)
                .flat_map(|form| &form.plan.fragments)
                .flat_map(|fragment| &fragment.placements)
                .map(|placement| placement.gear_id.as_str().to_owned())
                .collect(),
            port_ids: self
                .plan
                .iter()
                .flat_map(|plan| &plan.forms)
                .flat_map(|form| &form.plan.fragments)
                .flat_map(|fragment| &fragment.connections)
                .flat_map(|connection| {
                    [
                        format!(
                            "{}.{}",
                            connection.source_placement_id.as_str(),
                            connection.source_port_id.as_str()
                        ),
                        format!(
                            "{}.{}",
                            connection.sink_placement_id.as_str(),
                            connection.sink_port_id.as_str()
                        ),
                    ]
                })
                .collect(),
            cord_ids: self
                .plan
                .iter()
                .flat_map(|plan| &plan.forms)
                .flat_map(|form| &form.plan.fragments)
                .flat_map(|fragment| &fragment.connections)
                .map(|connection| connection.connection_id.as_str().to_owned())
                .collect(),
            input_sign_id: self.input_sign_id.clone(),
            result_sign_id: self.results[self.foreground].sign.clone(),
            result: self.foreground_result().map(|text| text.into()),
            result_omitted_bytes: self.results[self.foreground]
                .omitted_bytes(self.forms[self.foreground]),
            input_count: self.input_count,
            kernel_sign_gap: self
                .kernel
                .as_ref()
                .and_then(|kernel| kernel.sign_retention_gap())
                .or(self.retained_kernel_sign_gap),
            last_request_id: self.last_request_id.clone(),
        }
    }

    fn validate_target(&self, request: &JourneyRequest) -> Result<(), JourneyError> {
        let expected = match request.action {
            JourneyAction::OpenBack | JourneyAction::Birth => {
                format!("form/{}", self.form.checked_form_id.as_str())
            }
            JourneyAction::Wake
            | JourneyAction::Plan
            | JourneyAction::Play
            | JourneyAction::Stop
            | JourneyAction::Lull => self
                .body
                .as_ref()
                .map(|body| format!("body/{}", body.body_id.as_str()))
                .ok_or(JourneyError::BodyAbsent)?,
            _ => return Err(JourneyError::WrongTarget),
        };
        if request.target_identity != expected {
            return Err(JourneyError::WrongTarget);
        }
        Ok(())
    }

    fn open_form(&mut self) -> Result<(), JourneyError> {
        if self.body.is_some() {
            return Err(JourneyError::AlreadyBorn);
        }
        self.status = JourneyStatus::FormOpened;
        Ok(())
    }

    fn wake(&mut self) -> Result<(), JourneyError> {
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        if body.state != BodyState::Lulled {
            return Err(JourneyError::InvalidTransition);
        }
        let (body, wake) = body
            .wake(
                self.revision,
                SignId::from(format!("conduitos/product/woke/{}", self.revision)),
            )
            .map_err(|_| JourneyError::InvalidTransition)?;
        self.body = Some(body);
        self.wake = Some(wake);
        self.plan = None;
        self.planned_play = None;
        self.play = None;
        self.status = JourneyStatus::Awake;
        Ok(())
    }

    fn advance(&mut self) -> Result<(), JourneyError> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod workset_tests;

#[cfg(test)]
pub(crate) mod test_support;
