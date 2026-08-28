//! Finite admission and evidence retention for Presentation interactions.

use alloc::{collections::VecDeque, vec::Vec};

use crate::{
    presentation::validate_id, PresentationInteraction, PresentationInteractionDisposition,
    PresentationInteractionEvidence, PresentationInteractionRefusal,
};

pub const MAX_QUEUED_PRESENTATION_INTERACTIONS: usize = 8;
pub const MAX_RETAINED_INTERACTION_EVIDENCE: usize = 32;

#[derive(Debug)]
pub struct PresentationInteractionLedger {
    maximum_queued: usize,
    maximum_evidence: usize,
    queued: VecDeque<PresentationInteraction>,
    evidence: Vec<PresentationInteractionEvidence>,
}

impl PresentationInteractionLedger {
    pub fn new(
        maximum_queued: usize,
        maximum_evidence: usize,
    ) -> Result<Self, PresentationInteractionRefusal> {
        if maximum_queued == 0
            || maximum_queued > MAX_QUEUED_PRESENTATION_INTERACTIONS
            || maximum_evidence == 0
            || maximum_evidence > MAX_RETAINED_INTERACTION_EVIDENCE
        {
            return Err(PresentationInteractionRefusal::QueuePressure);
        }
        Ok(Self {
            maximum_queued,
            maximum_evidence,
            queued: VecDeque::with_capacity(maximum_queued),
            evidence: Vec::with_capacity(maximum_evidence),
        })
    }

    pub fn admit(
        &mut self,
        interaction: PresentationInteraction,
    ) -> Result<(), PresentationInteractionRefusal> {
        if self
            .queued
            .iter()
            .any(|item| item.identity == interaction.identity)
            || self
                .evidence
                .iter()
                .any(|item| item.interaction_id == interaction.identity)
        {
            return Err(PresentationInteractionRefusal::DuplicateDelivery);
        }
        if self.queued.len() == self.maximum_queued {
            return Err(PresentationInteractionRefusal::QueuePressure);
        }
        self.queued.push_back(interaction);
        Ok(())
    }

    pub fn finish_front(
        &mut self,
        disposition: PresentationInteractionDisposition,
    ) -> Result<&PresentationInteractionEvidence, PresentationInteractionRefusal> {
        if self.evidence.len() == self.maximum_evidence {
            return Err(PresentationInteractionRefusal::EvidenceExhausted);
        }
        let interaction = self
            .queued
            .pop_front()
            .ok_or(PresentationInteractionRefusal::UnknownInput)?;
        if let PresentationInteractionDisposition::Accepted {
            operation_request_id,
        } = &disposition
        {
            validate_id(operation_request_id)
                .map_err(|_| PresentationInteractionRefusal::MalformedEncoding)?;
        }
        self.evidence.push(PresentationInteractionEvidence {
            interaction_id: interaction.identity,
            presentation_id: interaction.presentation_id,
            presentation_revision: interaction.presentation_revision,
            manifestation_id: interaction.manifestation_id,
            input_id: interaction.input_id,
            action_id: interaction.action_id,
            target: interaction.target,
            value_kind: interaction.value_kind,
            value_bytes: interaction.value.len() as u32,
            sequence: interaction.sequence,
            disposition,
        });
        Ok(self.evidence.last().expect("evidence was just appended"))
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }

    pub fn evidence(&self) -> &[PresentationInteractionEvidence] {
        &self.evidence
    }
}
