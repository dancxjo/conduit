//! Shared ordinary Body replanning owner for browser and native Patchbay.

use conduit_body::{
    BodyPlayIdentity, BodyPresentationSelector, BodyPresenterChainPlan, BodyPresenterTopology,
    WakeLifecycle,
};
use conduit_core::SignId;
use conduit_presentation::{Manifestation, Presentation};

use crate::{
    BodyPlanningSession, BodyPlanningSessionError, BodyPlanningTransition, PresenterTopology,
    PresenterTopologyRefusal, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PresenterTopologyMode {
    Graphical,
    GraphicalAndSpeech,
    Speech,
}

#[derive(Debug, Clone)]
pub struct PresenterControlSession {
    presentation: Presentation,
    selector: BodyPresentationSelector,
    graphical_kind: RendererAdapterKind,
    graphical_identity: RendererAdapterIdentity,
    speech_identity: RendererAdapterIdentity,
    graphical: Option<RendererExecution>,
    speech: Option<RendererExecution>,
    sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenterControlError {
    StaleRequest,
    UnavailablePresenter,
    BodyPlanning(BodyPlanningSessionError),
    InvalidRealization,
    Projection(PresenterTopologyRefusal),
}

impl PresenterControlSession {
    pub fn new(
        presentation: Presentation,
        selector: BodyPresentationSelector,
        graphical_kind: RendererAdapterKind,
        graphical_identity: RendererAdapterIdentity,
        speech_identity: RendererAdapterIdentity,
    ) -> Result<Self, PresenterControlError> {
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
            return Err(PresenterControlError::StaleRequest);
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
    ) -> Result<(), PresenterControlError> {
        if planning.wake().lifecycle != WakeLifecycle::AwaitingPlan
            || !planning.current_plan().presenter_topologies.is_empty()
        {
            return Err(PresenterControlError::StaleRequest);
        }
        self.graphical = Some(self.prepare(
            self.graphical_kind,
            self.graphical_identity.clone(),
            "graphical",
        )?);
        let forms = planning.current_plan().forms.clone();
        planning
            .replace_proposal_with_presenters(
                forms,
                vec![self.body_topology(PresenterTopologyMode::Graphical)?],
            )
            .map_err(PresenterControlError::BodyPlanning)?;
        Ok(())
    }

    pub fn prepare_initial_graphical(
        &mut self,
    ) -> Result<BodyPresenterTopology, PresenterControlError> {
        if self.graphical.is_some() || self.speech.is_some() {
            return Err(PresenterControlError::StaleRequest);
        }
        self.graphical = Some(self.prepare(
            self.graphical_kind,
            self.graphical_identity.clone(),
            "graphical",
        )?);
        self.body_topology(PresenterTopologyMode::Graphical)
    }

    pub fn request_mode(
        &mut self,
        basis_plan_id: &conduit_core::PlanId,
        mode: PresenterTopologyMode,
        planning: &mut BodyPlanningSession,
        transition: BodyPlanningTransition,
    ) -> Result<PresenterTopology, PresenterControlError> {
        if planning.current_plan().plan_id != *basis_plan_id {
            return Err(PresenterControlError::StaleRequest);
        }
        let prior_graphical = self.graphical.clone();
        let prior_speech = self.speech.clone();
        if matches!(
            mode,
            PresenterTopologyMode::Graphical | PresenterTopologyMode::GraphicalAndSpeech
        ) && self.graphical.is_none()
        {
            self.graphical = Some(self.prepare(
                self.graphical_kind,
                self.graphical_identity.clone(),
                "graphical-restored",
            )?);
        }
        if matches!(
            mode,
            PresenterTopologyMode::Speech | PresenterTopologyMode::GraphicalAndSpeech
        ) && self.speech.is_none()
        {
            self.speech = Some(self.prepare(
                RendererAdapterKind::TestSpeech,
                self.speech_identity.clone(),
                "speech",
            )?);
        }
        let topology = self.body_topology(mode)?;
        let forms = planning.current_plan().forms.clone();
        if let Err(error) =
            planning.replan_with_presenters(forms, vec![topology], transition.clone())
        {
            self.graphical = prior_graphical;
            self.speech = prior_speech;
            return Err(PresenterControlError::BodyPlanning(error));
        }
        if mode == PresenterTopologyMode::Graphical {
            self.speech = None;
        } else if mode == PresenterTopologyMode::Speech {
            self.graphical = None;
        }
        let play = BodyPlayIdentity::bind(planning.current_plan(), transition.play_sequence);
        PresenterTopology::from_body_plan_truth(
            &self.presentation,
            planning.current_plan(),
            &play,
            &self.manifestations(),
        )
        .map_err(PresenterControlError::Projection)
    }

    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }

    pub fn project_current(
        &self,
        planning: &BodyPlanningSession,
        play: &BodyPlayIdentity,
    ) -> Result<PresenterTopology, PresenterControlError> {
        PresenterTopology::from_body_plan_truth(
            &self.presentation,
            planning.current_plan(),
            play,
            &self.manifestations(),
        )
        .map_err(PresenterControlError::Projection)
    }

    fn prepare(
        &mut self,
        kind: RendererAdapterKind,
        identity: RendererAdapterIdentity,
        label: &str,
    ) -> Result<RendererExecution, PresenterControlError> {
        let sequence = self.sequence;
        self.sequence = self.sequence.saturating_add(1);
        let mut execution = RendererExecution::prepare_with_offer_generation(
            self.presentation.clone(),
            kind,
            identity,
            sequence,
            SignId::from(format!("presenter-control/{label}/{sequence}/prepared")),
        )
        .map_err(|_| PresenterControlError::UnavailablePresenter)?;
        execution
            .mark_available(SignId::from(format!(
                "presenter-control/{label}/{sequence}/available"
            )))
            .map_err(|_| PresenterControlError::InvalidRealization)?;
        Ok(execution)
    }

    fn body_topology(
        &self,
        mode: PresenterTopologyMode,
    ) -> Result<BodyPresenterTopology, PresenterControlError> {
        let mut chains = Vec::new();
        if matches!(
            mode,
            PresenterTopologyMode::Graphical | PresenterTopologyMode::GraphicalAndSpeech
        ) {
            chains.push(chain(
                self.graphical
                    .as_ref()
                    .ok_or(PresenterControlError::UnavailablePresenter)?,
            ));
        }
        if matches!(
            mode,
            PresenterTopologyMode::Speech | PresenterTopologyMode::GraphicalAndSpeech
        ) {
            chains.push(chain(
                self.speech
                    .as_ref()
                    .ok_or(PresenterControlError::UnavailablePresenter)?,
            ));
        }
        Ok(BodyPresenterTopology {
            presentation: self.selector.clone(),
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

fn chain(execution: &RendererExecution) -> BodyPresenterChainPlan {
    BodyPresenterChainPlan {
        plan: execution.plan.clone(),
        stage_placement_ids: vec![execution.placement_id.clone()],
    }
}
