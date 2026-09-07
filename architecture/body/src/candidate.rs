use alloc::{string::String, vec::Vec};
use conduit_core::{
    BootId, HostAdvertisement, HostId, LinkBindingId, OfferGeneration, SignId, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::identity::{bind_identity, validate_ids};
use crate::{BodyId, BodyLifecycleError, CandidateId, DiscoveryProofId};

pub const MAX_CANDIDATES: usize = 16;
pub const MAX_CANDIDATE_HISTORY: usize = 64;
pub const MAX_INGRESS_REFUSALS: usize = 16;
// The installed browser profile has 65 offers and encodes to about 67 KiB
// including its admission envelope. Keep a finite bound that admits that real
// catalog, instead of silently hiding installed capabilities at membership.
pub const MAX_CANDIDATE_ADVERTISEMENT_BYTES: u32 = 72 * 1024;
pub const MAX_CANDIDATE_TOTAL_BYTES: u32 = 4 * MAX_CANDIDATE_ADVERTISEMENT_BYTES;
pub const MAX_CANDIDATE_LABEL_BYTES: usize = 128;
pub const MAX_CANDIDATE_RESOURCES: usize = 32;
pub const MAX_CANDIDATE_CAPABILITIES: usize = 70;
pub const MAX_CANDIDATE_PLANNER_CAPABILITIES: usize = 8;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateState {
    Discovered,
    RequestingAdmission,
    Admitted,
    Refused,
    Expired,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateObservation {
    /// The exact current advertisement is retained without semantic reinterpretation.
    pub advertisement: HostAdvertisement,
    /// Untrusted presentation data; never identity or authentication.
    pub friendly_label: String,
    /// Exact already-observed Line provenance. This does not admit or plan the Line.
    pub observed_binding_id: LinkBindingId,
    pub observation_sign_id: SignId,
    /// Opaque discovery proof state. This is not a membership credential.
    pub proof_id: DiscoveryProofId,
    pub freshness_sequence: u64,
    /// Exact complete frame size supplied by the framing layer.
    pub encoded_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionCandidate {
    pub candidate_id: CandidateId,
    pub state: CandidateState,
    pub observation: CandidateObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateEvent {
    pub candidate_id: CandidateId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub state: CandidateState,
    pub sign_id: SignId,
    pub sequence: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IngressFailureKind {
    MalformedFraming,
    DisconnectedBeforeComplete,
    OversizedAdvertisement,
    InvalidAdvertisement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngressFailure {
    pub kind: IngressFailureKind,
    pub observed_binding_id: LinkBindingId,
    pub sign_id: SignId,
    pub encoded_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateInventory {
    pub body_id: BodyId,
    pub candidates: Vec<AdmissionCandidate>,
    pub history: Vec<CandidateEvent>,
    pub ingress_failures: Vec<IngressFailure>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CandidateRefusal {
    EmptyIdentity,
    IdentityTooLong,
    WrongProtocol,
    MalformedAdvertisement,
    OversizedAdvertisement,
    CandidateCapacityExhausted,
    ByteCapacityExhausted,
    HistoryCapacityExhausted,
    RefusalHistoryCapacityExhausted,
    DuplicateObservation,
    StaleObservation,
    StaleBoot,
    StaleOfferGeneration,
    ConflictingProof,
    UnknownCandidate,
    InvalidTransition,
    DuplicateSign,
}

impl From<BodyLifecycleError> for CandidateRefusal {
    fn from(value: BodyLifecycleError) -> Self {
        match value {
            BodyLifecycleError::EmptyIdentity => Self::EmptyIdentity,
            BodyLifecycleError::IdentityTooLong => Self::IdentityTooLong,
            _ => Self::MalformedAdvertisement,
        }
    }
}

impl CandidateInventory {
    pub fn new(body_id: BodyId) -> Result<Self, CandidateRefusal> {
        validate_ids(&[body_id.as_str()])?;
        Ok(Self {
            body_id,
            candidates: Vec::new(),
            history: Vec::new(),
            ingress_failures: Vec::new(),
        })
    }

    pub fn observe(
        &mut self,
        observation: CandidateObservation,
    ) -> Result<CandidateId, CandidateRefusal> {
        if let Err(refusal) = validate_observation(&observation) {
            let kind = match refusal {
                CandidateRefusal::OversizedAdvertisement => {
                    IngressFailureKind::OversizedAdvertisement
                }
                _ => IngressFailureKind::InvalidAdvertisement,
            };
            self.record_ingress_failure(IngressFailure {
                kind,
                observed_binding_id: observation.observed_binding_id.clone(),
                sign_id: observation.observation_sign_id.clone(),
                encoded_bytes: observation.encoded_bytes,
            })?;
            return Err(refusal);
        }
        if self.history.len() == MAX_CANDIDATE_HISTORY {
            return Err(CandidateRefusal::HistoryCapacityExhausted);
        }
        let existing = self.candidates.iter().position(|candidate| {
            candidate.observation.advertisement.host_id == observation.advertisement.host_id
        });
        if let Some(index) = existing {
            self.validate_update(&self.candidates[index], &observation)?;
            self.ensure_new_sign(&observation.observation_sign_id)?;
            let old_bytes = self.candidates[index].observation.encoded_bytes;
            self.ensure_total_bytes(observation.encoded_bytes, old_bytes)?;
            let candidate_id = self.candidates[index].candidate_id.clone();
            self.candidates[index].state = CandidateState::Discovered;
            self.candidates[index].observation = observation.clone();
            self.push_event(
                candidate_id.clone(),
                CandidateState::Discovered,
                &observation,
            );
            return Ok(candidate_id);
        }
        if self.candidates.len() == MAX_CANDIDATES {
            return Err(CandidateRefusal::CandidateCapacityExhausted);
        }
        self.ensure_new_sign(&observation.observation_sign_id)?;
        self.ensure_total_bytes(observation.encoded_bytes, 0)?;
        let candidate_id = CandidateId::bound(bind_identity(
            "candidate",
            &[
                self.body_id.as_str(),
                observation.advertisement.host_id.as_str(),
                observation.advertisement.boot_id.as_str(),
            ],
            observation.freshness_sequence,
        ));
        self.candidates.push(AdmissionCandidate {
            candidate_id: candidate_id.clone(),
            state: CandidateState::Discovered,
            observation: observation.clone(),
        });
        self.push_event(
            candidate_id.clone(),
            CandidateState::Discovered,
            &observation,
        );
        Ok(candidate_id)
    }

    pub fn transition(
        &mut self,
        candidate_id: &CandidateId,
        next: CandidateState,
        sign_id: SignId,
    ) -> Result<(), CandidateRefusal> {
        validate_ids(&[candidate_id.as_str(), sign_id.as_str()])?;
        self.ensure_new_sign(&sign_id)?;
        if self.history.len() == MAX_CANDIDATE_HISTORY {
            return Err(CandidateRefusal::HistoryCapacityExhausted);
        }
        let index = self
            .candidates
            .iter()
            .position(|candidate| &candidate.candidate_id == candidate_id)
            .ok_or(CandidateRefusal::UnknownCandidate)?;
        let prior = self.candidates[index].state;
        let permitted = matches!(
            (prior, next),
            (
                CandidateState::Discovered,
                CandidateState::RequestingAdmission
            ) | (CandidateState::Discovered, CandidateState::Refused)
                | (CandidateState::Discovered, CandidateState::Lost)
                | (CandidateState::Discovered, CandidateState::Expired)
                | (
                    CandidateState::RequestingAdmission,
                    CandidateState::Admitted
                )
                | (CandidateState::RequestingAdmission, CandidateState::Refused)
                | (CandidateState::RequestingAdmission, CandidateState::Expired)
                | (CandidateState::RequestingAdmission, CandidateState::Lost)
        );
        if !permitted {
            return Err(CandidateRefusal::InvalidTransition);
        }
        self.candidates[index].state = next;
        let observation = self.candidates[index].observation.clone();
        let sequence = self.history.len() as u64 + 1;
        self.history.push(CandidateEvent {
            candidate_id: candidate_id.clone(),
            host_id: observation.advertisement.host_id,
            boot_id: observation.advertisement.boot_id,
            offer_generation: observation.advertisement.offer_generation,
            state: next,
            sign_id,
            sequence,
        });
        Ok(())
    }

    pub fn record_incomplete(
        &mut self,
        kind: IngressFailureKind,
        observed_binding_id: LinkBindingId,
        sign_id: SignId,
        encoded_bytes: u32,
    ) -> Result<(), CandidateRefusal> {
        if !matches!(
            kind,
            IngressFailureKind::MalformedFraming | IngressFailureKind::DisconnectedBeforeComplete
        ) {
            return Err(CandidateRefusal::MalformedAdvertisement);
        }
        validate_ids(&[observed_binding_id.as_str(), sign_id.as_str()])?;
        self.record_ingress_failure(IngressFailure {
            kind,
            observed_binding_id,
            sign_id,
            encoded_bytes,
        })
    }

    fn validate_update(
        &self,
        current: &AdmissionCandidate,
        next: &CandidateObservation,
    ) -> Result<(), CandidateRefusal> {
        let prior = &current.observation;
        if next.freshness_sequence < prior.freshness_sequence {
            return Err(CandidateRefusal::StaleObservation);
        }
        if next.freshness_sequence == prior.freshness_sequence {
            return if next == prior {
                Err(CandidateRefusal::DuplicateObservation)
            } else if next.proof_id != prior.proof_id {
                Err(CandidateRefusal::ConflictingProof)
            } else {
                Err(CandidateRefusal::StaleObservation)
            };
        }
        if next.advertisement.boot_id == prior.advertisement.boot_id
            && next.advertisement.offer_generation <= prior.advertisement.offer_generation
        {
            return Err(CandidateRefusal::StaleOfferGeneration);
        }
        if next.advertisement.boot_id != prior.advertisement.boot_id
            && self.history.iter().any(|event| {
                event.host_id == next.advertisement.host_id
                    && event.boot_id == next.advertisement.boot_id
            })
        {
            return Err(CandidateRefusal::StaleBoot);
        }
        if next.proof_id != prior.proof_id
            && next.advertisement.boot_id == prior.advertisement.boot_id
        {
            return Err(CandidateRefusal::ConflictingProof);
        }
        Ok(())
    }

    fn push_event(
        &mut self,
        candidate_id: CandidateId,
        state: CandidateState,
        observation: &CandidateObservation,
    ) {
        self.history.push(CandidateEvent {
            candidate_id,
            host_id: observation.advertisement.host_id.clone(),
            boot_id: observation.advertisement.boot_id.clone(),
            offer_generation: observation.advertisement.offer_generation,
            state,
            sign_id: observation.observation_sign_id.clone(),
            sequence: self.history.len() as u64 + 1,
        });
    }

    fn ensure_new_sign(&self, sign_id: &SignId) -> Result<(), CandidateRefusal> {
        if self.history.iter().any(|event| &event.sign_id == sign_id)
            || self
                .ingress_failures
                .iter()
                .any(|failure| &failure.sign_id == sign_id)
        {
            return Err(CandidateRefusal::DuplicateSign);
        }
        Ok(())
    }

    fn ensure_total_bytes(&self, incoming: u32, replaced: u32) -> Result<(), CandidateRefusal> {
        let retained = self
            .candidates
            .iter()
            .map(|candidate| candidate.observation.encoded_bytes)
            .sum::<u32>();
        retained
            .checked_sub(replaced)
            .and_then(|bytes| bytes.checked_add(incoming))
            .filter(|bytes| *bytes <= MAX_CANDIDATE_TOTAL_BYTES)
            .map(|_| ())
            .ok_or(CandidateRefusal::ByteCapacityExhausted)
    }

    fn record_ingress_failure(&mut self, failure: IngressFailure) -> Result<(), CandidateRefusal> {
        validate_ids(&[
            failure.observed_binding_id.as_str(),
            failure.sign_id.as_str(),
        ])?;
        self.ensure_new_sign(&failure.sign_id)?;
        if self.ingress_failures.len() == MAX_INGRESS_REFUSALS {
            return Err(CandidateRefusal::RefusalHistoryCapacityExhausted);
        }
        self.ingress_failures.push(failure);
        Ok(())
    }
}

fn validate_observation(observation: &CandidateObservation) -> Result<(), CandidateRefusal> {
    validate_ids(&[
        observation.advertisement.host_id.as_str(),
        observation.advertisement.boot_id.as_str(),
        observation.advertisement.profile.as_str(),
        observation.observed_binding_id.as_str(),
        observation.observation_sign_id.as_str(),
        observation.proof_id.as_str(),
    ])?;
    if observation.advertisement.protocol_version != PROTOCOL_VERSION {
        return Err(CandidateRefusal::WrongProtocol);
    }
    if observation.encoded_bytes == 0
        || observation.encoded_bytes > MAX_CANDIDATE_ADVERTISEMENT_BYTES
    {
        return Err(CandidateRefusal::OversizedAdvertisement);
    }
    if observation.friendly_label.len() > MAX_CANDIDATE_LABEL_BYTES
        || observation.advertisement.resources.len() > MAX_CANDIDATE_RESOURCES
        || observation.advertisement.capabilities.len() > MAX_CANDIDATE_CAPABILITIES
        || observation.advertisement.planner_capabilities.len() > MAX_CANDIDATE_PLANNER_CAPABILITIES
    {
        return Err(CandidateRefusal::MalformedAdvertisement);
    }
    Ok(())
}
