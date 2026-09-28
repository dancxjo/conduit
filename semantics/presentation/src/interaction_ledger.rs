//! Finite admission and evidence retention for inward Face interactions.

use alloc::{collections::VecDeque, vec::Vec};

use crate::{
    presentation::validate_id, FaceInteraction, FaceInteractionArgumentEvidence,
    FaceInteractionDisposition, FaceInteractionEvidence, FaceInteractionRefusal,
};

pub const MAX_QUEUED_FACE_INTERACTIONS: usize = 8;
pub const MAX_RETAINED_INTERACTION_EVIDENCE: usize = 32;

#[derive(Debug)]
pub struct FaceInteractionLedger {
    maximum_queued: usize,
    maximum_evidence: usize,
    queued: VecDeque<FaceInteraction>,
    evidence: Vec<FaceInteractionEvidence>,
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
        })
    }

    pub fn admit(&mut self, interaction: FaceInteraction) -> Result<(), FaceInteractionRefusal> {
        if self
            .queued
            .iter()
            .any(|item| item.identity == interaction.identity)
            || self
                .evidence
                .iter()
                .any(|item| item.interaction_id == interaction.identity)
        {
            return Err(FaceInteractionRefusal::DuplicateDelivery);
        }
        if self.queued.len() == self.maximum_queued {
            return Err(FaceInteractionRefusal::QueuePressure);
        }
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
        let interaction = self
            .queued
            .pop_front()
            .ok_or(FaceInteractionRefusal::NoQueuedInteraction)?;
        if let FaceInteractionDisposition::Accepted {
            operation_request_id,
        } = &disposition
        {
            validate_id(operation_request_id)
                .map_err(|_| FaceInteractionRefusal::MalformedEncoding)?;
        }
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
}
