//! Shared ordinary body replanning owner for browser and native Patchbay.

use conduit_body::{
    BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology, BodyPlayIdentity, WakeLifecycle,
};
use conduit_core::SignId;
use conduit_presentation::{Manifestation, Presentation};

use crate::{
    BodyPlanningSession, BodyPlanningSessionError, BodyPlanningTransition, MaskTopology,
    MaskTopologyRefusal, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MaskTopologyMode {
    Graphical,
    GraphicalAndSpeech,
    Speech,
}

#[derive(Debug, Clone)]
pub struct MaskControlSession {
    presentation: Presentation,
    selector: BodyFaceSelector,
    graphical_kind: RendererAdapterKind,
    graphical_identity: RendererAdapterIdentity,
    speech_identity: RendererAdapterIdentity,
    graphical: Option<RendererExecution>,
    speech: Option<RendererExecution>,
    sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskControlError {
    StaleRequest,
    UnavailableMask,
    BodyPlanning(BodyPlanningSessionError),
    InvalidRealization,
    Projection(MaskTopologyRefusal),
}

impl MaskControlSession {
    pub fn new(
        presentation: Presentation,
        selector: BodyFaceSelector,
        graphical_kind: RendererAdapterKind,
        graphical_identity: RendererAdapterIdentity,
        speech_identity: RendererAdapterIdentity,
    ) -> Result<Self, MaskControlError> {
        let selector_matches = match &selector.form {
            Some(form) => {
                presentation.basis.source_document_id.as_ref() == Some(&form.source_document_id)
                    && presentation.basis.checked_form_id.as_ref() == Some(&form.checked_form_id)
            }
            None => {
                presentation.basis.source_document_id.is_none()
                    && presentation.basis.checked_form_id.is_none()
            }
        };
        if !selector_matches {
            return Err(MaskControlError::StaleRequest);
        }
        Ok(Self {
            presentation,
            selector,
            graphical_kind,
            graphical_identity,
            speech_identity,
            graphical: None,
            speech: None,
            sequence: 1,
        })
    }

    pub fn install_initial_graphical(
        &mut self,
        planning: &mut BodyPlanningSession,
    ) -> Result<(), MaskControlError> {
        if planning.wake().lifecycle != WakeLifecycle::AwaitingPlan
            || !planning.current_plan().mask_topologies.is_empty()
        {
            return Err(MaskControlError::StaleRequest);
        }
        let mut candidate = self.clone();
        let mut next_planning = planning.clone();
        candidate.graphical = Some(candidate.prepare(
            candidate.graphical_kind,
            candidate.graphical_identity.clone(),
            "graphical",
        )?);
        let forms = next_planning.current_plan().forms.clone();
        next_planning
            .replace_proposal_with_masks(
                forms,
                vec![candidate.body_topology(MaskTopologyMode::Graphical)?],
            )
            .map_err(MaskControlError::BodyPlanning)?;
        *self = candidate;
        *planning = next_planning;
        Ok(())
    }

    pub fn prepare_initial_graphical(&mut self) -> Result<BodyMaskTopology, MaskControlError> {
        if self.graphical.is_some() || self.speech.is_some() {
            return Err(MaskControlError::StaleRequest);
        }
        self.graphical = Some(self.prepare(
            self.graphical_kind,
            self.graphical_identity.clone(),
            "graphical",
        )?);
        self.body_topology(MaskTopologyMode::Graphical)
    }

    pub fn request_mode(
        &mut self,
        basis_plan_id: &conduit_core::PlanId,
        mode: MaskTopologyMode,
        planning: &mut BodyPlanningSession,
        transition: BodyPlanningTransition,
    ) -> Result<MaskTopology, MaskControlError> {
        if planning.current_plan().plan_id != *basis_plan_id {
            return Err(MaskControlError::StaleRequest);
        }
        let mut candidate = self.clone();
        let mut next_planning = planning.clone();
        if matches!(
            mode,
            MaskTopologyMode::Graphical | MaskTopologyMode::GraphicalAndSpeech
        ) && candidate.graphical.is_none()
        {
            candidate.graphical = Some(candidate.prepare(
                candidate.graphical_kind,
                candidate.graphical_identity.clone(),
                "graphical-restored",
            )?);
        }
        if matches!(
            mode,
            MaskTopologyMode::Speech | MaskTopologyMode::GraphicalAndSpeech
        ) && candidate.speech.is_none()
        {
            candidate.speech = Some(candidate.prepare(
                RendererAdapterKind::TestSpeech,
                candidate.speech_identity.clone(),
                "speech",
            )?);
        }
        let topology = candidate.body_topology(mode)?;
        let forms = next_planning.current_plan().forms.clone();
        next_planning
            .replan_with_masks(forms, vec![topology], transition.clone())
            .map_err(MaskControlError::BodyPlanning)?;
        if mode == MaskTopologyMode::Graphical {
            candidate.speech = None;
        } else if mode == MaskTopologyMode::Speech {
            candidate.graphical = None;
        }
        let play = BodyPlayIdentity::bind(next_planning.current_plan(), transition.play_sequence);
        let projected = MaskTopology::from_body_plan_truth(
            &candidate.presentation,
            next_planning.current_plan(),
            &play,
            &candidate.manifestations(),
        )
        .map_err(MaskControlError::Projection)?;
        *self = candidate;
        *planning = next_planning;
        Ok(projected)
    }

    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }

    /// Retarget the selected Mask Plans at a fresh immutable projection of
    /// the same body/Form subject. Runtime evolution changes Presentation
    /// identity; it does not make the renderer the owner of that evolution.
    pub fn refresh_presentation(
        &mut self,
        presentation: Presentation,
    ) -> Result<(), MaskControlError> {
        let selector_matches = match &self.selector.form {
            Some(form) => {
                presentation.basis.source_document_id.as_ref() == Some(&form.source_document_id)
                    && presentation.basis.checked_form_id.as_ref() == Some(&form.checked_form_id)
            }
            None => {
                presentation.basis.source_document_id.is_none()
                    && presentation.basis.checked_form_id.is_none()
            }
        };
        if !selector_matches {
            return Err(MaskControlError::StaleRequest);
        }
        let mut sequence = self.sequence;
        let refresh = |current: &RendererExecution, sequence: &mut u64| {
            let current_sequence = *sequence;
            *sequence = sequence.saturating_add(1);
            let mut next = RendererExecution::prepare_planned_sequence(
                presentation.clone(),
                current.plan.clone(),
                current.manifestation.target_subject.clone(),
                current_sequence,
                SignId::from(format!("mask-control/refresh/{current_sequence}/prepared")),
            )
            .map_err(|_| MaskControlError::InvalidRealization)?;
            next.mark_available(SignId::from(format!(
                "mask-control/refresh/{current_sequence}/available"
            )))
            .map_err(|_| MaskControlError::InvalidRealization)?;
            Ok(next)
        };
        let graphical = self
            .graphical
            .as_ref()
            .map(|current| refresh(current, &mut sequence))
            .transpose()?;
        let speech = self
            .speech
            .as_ref()
            .map(|current| refresh(current, &mut sequence))
            .transpose()?;
        self.graphical = graphical;
        self.speech = speech;
        self.sequence = sequence;
        self.presentation = presentation;
        Ok(())
    }

    pub fn project_current(
        &self,
        planning: &BodyPlanningSession,
        play: &BodyPlayIdentity,
    ) -> Result<MaskTopology, MaskControlError> {
        MaskTopology::from_body_plan_truth(
            &self.presentation,
            planning.current_plan(),
            play,
            &self.manifestations(),
        )
        .map_err(MaskControlError::Projection)
    }

    fn prepare(
        &mut self,
        kind: RendererAdapterKind,
        identity: RendererAdapterIdentity,
        label: &str,
    ) -> Result<RendererExecution, MaskControlError> {
        let sequence = self.sequence;
        self.sequence = self.sequence.saturating_add(1);
        let mut execution = RendererExecution::prepare_with_offer_generation(
            self.presentation.clone(),
            kind,
            identity,
            sequence,
            SignId::from(format!("mask-control/{label}/{sequence}/prepared")),
        )
        .map_err(|_| MaskControlError::UnavailableMask)?;
        execution
            .mark_available(SignId::from(format!(
                "mask-control/{label}/{sequence}/available"
            )))
            .map_err(|_| MaskControlError::InvalidRealization)?;
        Ok(execution)
    }

    fn body_topology(&self, mode: MaskTopologyMode) -> Result<BodyMaskTopology, MaskControlError> {
        let mut chains = Vec::new();
        if matches!(
            mode,
            MaskTopologyMode::Graphical | MaskTopologyMode::GraphicalAndSpeech
        ) {
            chains.push(chain(
                self.graphical
                    .as_ref()
                    .ok_or(MaskControlError::UnavailableMask)?,
            ));
        }
        if matches!(
            mode,
            MaskTopologyMode::Speech | MaskTopologyMode::GraphicalAndSpeech
        ) {
            chains.push(chain(
                self.speech
                    .as_ref()
                    .ok_or(MaskControlError::UnavailableMask)?,
            ));
        }
        Ok(BodyMaskTopology {
            face: self.selector.clone(),
            chains,
        })
    }

    fn manifestations(&self) -> Vec<Manifestation> {
        self.graphical
            .iter()
            .chain(self.speech.iter())
            .map(|execution| execution.manifestation.clone())
            .collect()
    }
}

fn chain(execution: &RendererExecution) -> BodyMaskChainPlan {
    BodyMaskChainPlan {
        plan: execution.plan.clone(),
        stage_placement_ids: vec![execution.placement_id.clone()],
    }
}
