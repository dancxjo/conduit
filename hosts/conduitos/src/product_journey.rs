//! Canonical bounded Body/Wake/Plan/Play state for the ordinary product entrance.

use alloc::{borrow::ToOwned, boxed::Box, format, string::String, vec::Vec};

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, BodyState, MembershipProofId, PartId,
    SeedId, Wake,
};
use conduit_core::{
    ActivePlayIdentity, BootId, ExpandedFormId, HostId, OfferGeneration, Plan, SignId,
};
use conduit_kernel::scheduler::HostOperationRequest;

use crate::{
    identity::BootIdentities,
    keyboard_text_plan::{self, KeyboardTextSeedIdentity},
    keyboard_text_play::KeyboardTextKernel,
    offer::HostOffer,
    ordinary_plan::PreparationError,
};

mod play;

pub use conduit_core::{PatchbayAction as JourneyAction, PatchbayControlRequest as JourneyRequest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyStatus {
    World,
    SeedOpened,
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
            Self::SeedOpened => "seed-opened",
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
    SeedNotOpened,
    AlreadyBorn,
    BodyAbsent,
    InvalidTransition,
    Membership,
    Plan(PreparationError),
    Kernel,
    InputUnavailable,
    RevisionExhausted,
}

impl JourneyError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StalePresentation => "product-interaction-stale-presentation",
            Self::WrongTarget => "product-interaction-wrong-target",
            Self::SeedNotOpened => "product-birth-seed-not-open",
            Self::AlreadyBorn => "product-birth-duplicate",
            Self::BodyAbsent => "product-body-absent",
            Self::InvalidTransition => "product-lifecycle-transition-refused",
            Self::Membership => "product-birth-membership-refused",
            Self::Plan(error) => error.as_str(),
            Self::Kernel => "product-kernel-refused",
            Self::InputUnavailable => "product-input-unavailable",
            Self::RevisionExhausted => "product-presentation-revision-exhausted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyProjection {
    pub status: JourneyStatus,
    pub revision: u64,
    pub seed_id: SeedId,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub body_id: Option<conduit_body::BodyId>,
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
    pub last_request_id: Option<String>,
}

pub struct ProductJourney {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    seed: KeyboardTextSeedIdentity,
    status: JourneyStatus,
    revision: u64,
    request_sequence: u64,
    body: Option<Body>,
    born_sign_id: Option<SignId>,
    membership: Option<BodyMembership>,
    part_id: Option<PartId>,
    wake: Option<Wake>,
    plan: Option<Plan>,
    planned_play: Option<ActivePlayIdentity>,
    play: Option<ActivePlayIdentity>,
    kernel: Option<Box<KeyboardTextKernel>>,
    pending_keyboard: Option<HostOperationRequest>,
    input_count: u8,
    input_sign_id: Option<SignId>,
    result_sign_id: Option<SignId>,
    result: Option<String>,
    last_request_id: Option<String>,
}

impl ProductJourney {
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, JourneyError> {
        let seed = keyboard_text_plan::checked_seed_identity().map_err(JourneyError::Plan)?;
        Ok(Self {
            host_id,
            boot_id,
            offer_generation,
            seed,
            status: JourneyStatus::World,
            revision: 1,
            request_sequence: 0,
            body: None,
            born_sign_id: None,
            membership: None,
            part_id: None,
            wake: None,
            plan: None,
            planned_play: None,
            play: None,
            kernel: None,
            pending_keyboard: None,
            input_count: 0,
            input_sign_id: None,
            result_sign_id: None,
            result: None,
            last_request_id: None,
        })
    }

    pub fn seed(&self) -> &KeyboardTextSeedIdentity {
        &self.seed
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
            JourneyAction::OpenBack => self.open_seed()?,
            JourneyAction::BeBorn => self.be_born()?,
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
            seed_id: SeedId::bind(&self.seed.source_document_id, &self.seed.checked_form_id),
            source_document_id: self.seed.source_document_id.clone(),
            checked_form_id: self.seed.checked_form_id.clone(),
            expanded_form_id: self.seed.expanded_form_id.clone(),
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            body_id: self.body.as_ref().map(|body| body.body_id.clone()),
            born_sign_id: self.born_sign_id.clone(),
            part_id: self.part_id.clone(),
            wake_id: self.wake.as_ref().map(|wake| wake.wake_id.clone()),
            plan_id: self.plan.as_ref().map(|plan| plan.plan_id.clone()),
            active_play_id: self.play.as_ref().map(|play| play.active_play_id.clone()),
            gear_ids: self
                .plan
                .iter()
                .flat_map(|plan| &plan.fragments)
                .flat_map(|fragment| &fragment.placements)
                .map(|placement| placement.gear_id.as_str().to_owned())
                .collect(),
            port_ids: self
                .plan
                .iter()
                .flat_map(|plan| &plan.fragments)
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
                .flat_map(|plan| &plan.fragments)
                .flat_map(|fragment| &fragment.connections)
                .map(|connection| connection.connection_id.as_str().to_owned())
                .collect(),
            input_sign_id: self.input_sign_id.clone(),
            result_sign_id: self.result_sign_id.clone(),
            result: self.result.clone(),
            last_request_id: self.last_request_id.clone(),
        }
    }

    fn validate_target(&self, request: &JourneyRequest) -> Result<(), JourneyError> {
        let expected = match request.action {
            JourneyAction::OpenBack | JourneyAction::BeBorn => {
                format!(
                    "seed/{}",
                    SeedId::bind(&self.seed.source_document_id, &self.seed.checked_form_id)
                        .as_str()
                )
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

    fn open_seed(&mut self) -> Result<(), JourneyError> {
        if self.body.is_some() {
            return Err(JourneyError::AlreadyBorn);
        }
        self.status = JourneyStatus::SeedOpened;
        Ok(())
    }

    fn be_born(&mut self) -> Result<(), JourneyError> {
        if self.body.is_some() {
            return Err(JourneyError::AlreadyBorn);
        }
        if self.status != JourneyStatus::SeedOpened {
            return Err(JourneyError::SeedNotOpened);
        }
        let born_sign = SignId::from(format!("conduitos/product/born/{}", self.revision));
        let body = Body::born(
            self.seed.source_document_id.clone(),
            self.seed.checked_form_id.clone(),
            0,
            born_sign.clone(),
        )
        .map_err(|_| JourneyError::InvalidTransition)?;
        let part = PartId::bind(&body.body_id, self.host_id.as_str(), 0)
            .map_err(|_| JourneyError::Membership)?;
        let proof = MembershipProofId::bind("conduitos/product/local-birth")
            .map_err(|_| JourneyError::Membership)?;
        let mut membership =
            BodyMembership::new(body.body_id.clone()).map_err(|_| JourneyError::Membership)?;
        membership
            .admit(
                &body.body_id,
                membership.revision,
                part.clone(),
                proof.clone(),
                SignId::from("conduitos/product/part-admitted"),
            )
            .map_err(|_| JourneyError::Membership)?;
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: self.host_id.clone(),
                    boot_id: self.boot_id.clone(),
                    offer_generation: self.offer_generation,
                    proof_id: proof,
                    sequence: 0,
                },
                SignId::from("conduitos/product/host-attached"),
            )
            .map_err(|_| JourneyError::Membership)?;
        self.body = Some(body);
        self.born_sign_id = Some(born_sign);
        self.membership = Some(membership);
        self.part_id = Some(part);
        self.status = JourneyStatus::BornLulled;
        Ok(())
    }

    fn wake(&mut self) -> Result<(), JourneyError> {
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        if body.state != BodyState::Lulled {
            return Err(JourneyError::InvalidTransition);
        }
        let (body, wake) = body
            .wake(0, SignId::from("conduitos/product/woke"))
            .map_err(|_| JourneyError::InvalidTransition)?;
        self.body = Some(body);
        self.wake = Some(wake);
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
