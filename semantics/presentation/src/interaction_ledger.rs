//! Finite admission and evidence retention for inward Face interactions.

use alloc::{collections::VecDeque, vec::Vec};

use crate::{
    presentation::validate_id, FaceInteraction, FaceInteractionArgumentEvidence,
    FaceInteractionDisposition, FaceInteractionEvidence, FaceInteractionId, FaceInteractionRefusal,
};

pub const MAX_QUEUED_FACE_INTERACTIONS: usize = 8;
pub const MAX_RETAINED_INTERACTION_EVIDENCE: usize = 32;
/// One ledger is scoped to one finite action campaign, such as a 64-item Play.
pub const MAX_INTERACTION_LEDGER_ADMISSIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceEvidenceAckRefusal {
    Empty,
    MismatchedPrefix,
}

#[derive(Debug)]
pub struct FaceInteractionLedger {
    maximum_queued: usize,
    maximum_evidence: usize,
    queued: VecDeque<FaceInteraction>,
    evidence: Vec<FaceInteractionEvidence>,
    retired: Vec<FaceInteractionId>,
}

impl FaceInteractionLedger {
    pub fn new(
        maximum_queued: usize,
        maximum_evidence: usize,
    ) -> Result<Self, FaceInteractionRefusal> {
        if maximum_queued == 0
            || maximum_queued > MAX_QUEUED_FACE_INTERACTIONS
            || maximum_evidence == 0
            || maximum_evidence > MAX_RETAINED_INTERACTION_EVIDENCE
        {
            return Err(FaceInteractionRefusal::QueuePressure);
        }
        Ok(Self {
            maximum_queued,
            maximum_evidence,
            queued: VecDeque::with_capacity(maximum_queued),
            evidence: Vec::with_capacity(maximum_evidence),
            retired: Vec::with_capacity(MAX_INTERACTION_LEDGER_ADMISSIONS),
        })
    }

    /// Preflight is stable while the caller holds exclusive ledger ownership.
    /// It lets an external finite queue refuse pressure without admitting an
    /// interaction that the caller could not then submit.
    pub fn check_admit(&self, interaction: &FaceInteraction) -> Result<(), FaceInteractionRefusal> {
        if self
            .queued
            .iter()
            .any(|item| item.identity == interaction.identity)
            || self
                .evidence
                .iter()
                .any(|item| item.interaction_id == interaction.identity)
            || self.retired.contains(&interaction.identity)
        {
            return Err(FaceInteractionRefusal::DuplicateDelivery);
        }
        if self.retired.len() + self.evidence.len() + self.queued.len()
            == MAX_INTERACTION_LEDGER_ADMISSIONS
        {
            return Err(FaceInteractionRefusal::EvidenceExhausted);
        }
        if self.evidence.len() == self.maximum_evidence {
            return Err(FaceInteractionRefusal::EvidenceExhausted);
        }
        if self.queued.len() == self.maximum_queued {
            return Err(FaceInteractionRefusal::QueuePressure);
        }
        Ok(())
    }

    pub fn admit(&mut self, interaction: FaceInteraction) -> Result<(), FaceInteractionRefusal> {
        self.check_admit(&interaction)?;
        self.queued.push_back(interaction);
        Ok(())
    }

    pub fn finish_front(
        &mut self,
        disposition: FaceInteractionDisposition,
    ) -> Result<&FaceInteractionEvidence, FaceInteractionRefusal> {
        if self.evidence.len() == self.maximum_evidence {
            return Err(FaceInteractionRefusal::EvidenceExhausted);
        }
        if self.queued.is_empty() {
            return Err(FaceInteractionRefusal::NoQueuedInteraction);
        }
        if let FaceInteractionDisposition::Accepted {
            operation_request_id,
        } = &disposition
        {
            validate_id(operation_request_id)
                .map_err(|_| FaceInteractionRefusal::MalformedEncoding)?;
        }
        let interaction = self
            .queued
            .pop_front()
            .expect("front was checked before disposition validation");
        self.evidence.push(FaceInteractionEvidence {
            interaction_id: interaction.identity,
            face_id: interaction.face_id,
            face_revision: interaction.face_revision,
            show_id: interaction.show_id,
            action_id: interaction.action_id,
            target: interaction.target,
            arguments: interaction
                .arguments
                .into_iter()
                .map(|argument| FaceInteractionArgumentEvidence {
                    name: argument.name,
                    value_kind: argument.value_kind,
                    value_bytes: argument.value.len() as u32,
                })
                .collect(),
            sequence: interaction.sequence,
            disposition,
        });
        Ok(self.evidence.last().expect("evidence was just appended"))
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }

    pub fn evidence(&self) -> &[FaceInteractionEvidence] {
        &self.evidence
    }

    /// A caller first retains the complete immutable evidence prefix outside
    /// this ledger, then acknowledges those exact identities. A stale, partial,
    /// or reordered acknowledgement changes nothing. Retired identities stay
    /// in a finite duplicate guard for the rest of this ledger's campaign.
    pub fn acknowledge_persisted_evidence_prefix(
        &mut self,
        expected: &[FaceInteractionId],
    ) -> Result<(), FaceEvidenceAckRefusal> {
        if expected.is_empty() {
            return Err(FaceEvidenceAckRefusal::Empty);
        }
        if expected.len() > self.evidence.len()
            || !self
                .evidence
                .iter()
                .take(expected.len())
                .zip(expected)
                .all(|(actual, expected)| &actual.interaction_id == expected)
        {
            return Err(FaceEvidenceAckRefusal::MismatchedPrefix);
        }
        self.retired.extend(
            self.evidence
                .drain(..expected.len())
                .map(|entry| entry.interaction_id),
        );
        Ok(())
    }
}
