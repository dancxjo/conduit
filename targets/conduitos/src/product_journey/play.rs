//! One admitted native Body Plan/Play across exact resident Plot partitions.
use super::{JourneyError, JourneyLossKind, JourneyStatus, ProductJourney};
use crate::{identity::BootIdentities, native_workset, offer::HostOffer};
use alloc::{boxed::Box, format};
use conduit_body::BodyPlayIdentity;
use conduit_core::{SignId, bind_sign};
use conduit_human::KeyEvent;
use conduit_presentation::{ApplicationEvent, ApplicationView};

impl ProductJourney {
    pub fn foreground_input_owner(&self) -> Option<&native_workset::AdmittedPlotInput> {
        (self.status == JourneyStatus::QuiescentAwaitingInput)
            .then(|| self.kernel.as_ref()?.input_owner(self.foreground_index()))?
    }

    pub fn foreground_application_view(&self) -> Option<&ApplicationView> {
        (self.status == JourneyStatus::QuiescentAwaitingInput).then(|| {
            self.kernel
                .as_ref()?
                .application_view(self.foreground_index())
        })?
    }

    pub fn foreground_patchbay_graph(&self) -> Option<&patchbay_graph::PatchbayGraph> {
        (self.status == JourneyStatus::QuiescentAwaitingInput).then(|| {
            self.kernel
                .as_ref()?
                .patchbay_graph(self.foreground_index())
        })?
    }

    pub fn take_application_request(&mut self) -> Option<native_workset::NativeApplicationRequest> {
        self.application_request.take()
    }

    pub fn complete_tour_run(
        &mut self,
        evidence: &crate::tour_play::TourPlayEvidence,
    ) -> Result<(), JourneyError> {
        if self.status != JourneyStatus::QuiescentAwaitingInput {
            return Err(JourneyError::InvalidTransition);
        }
        let tour = self
            .plots
            .iter()
            .position(|plot| *plot == Some(native_workset::NativePlot::Tour))
            .ok_or(JourneyError::Kernel)?;
        self.kernel
            .as_mut()
            .ok_or(JourneyError::Kernel)?
            .complete_tour_run(
                tour,
                conduit_tour_model::TourRunProof {
                    specimen_id: evidence.specimen_id.into(),
                    source_document_id: evidence.source_document_id.clone(),
                    checked_plot_id: evidence.checked_plot_id.clone(),
                    expanded_plot_id: evidence.expanded_plot_id.clone(),
                    plan_id: evidence.plan_id.clone(),
                    active_play_id: evidence.active_play_id.clone(),
                    result: evidence.result.into(),
                    terminal: conduit_tour_model::TourRunTerminal::Completed,
                    comparison: None,
                    multi_host: None,
                },
            )
            .map_err(JourneyError::Play)?;
        self.advance()
    }

    pub fn accept_application_event(
        &mut self,
        event: &ApplicationEvent,
    ) -> Result<bool, JourneyError> {
        if self.status != JourneyStatus::QuiescentAwaitingInput {
            return Ok(false);
        }
        let next_count = self
            .input_count
            .checked_add(1)
            .ok_or(JourneyError::InputSequenceExhausted)?;
        self.revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        let foreground = self.foreground_index();
        let play = self
            .current_play()
            .cloned()
            .ok_or(JourneyError::InvalidTransition)?;
        let kernel = self.kernel.as_mut().ok_or(JourneyError::Kernel)?;
        let current = kernel
            .application_view(foreground)
            .ok_or(JourneyError::InputUnavailable)?;
        let encoded = event
            .encode(current)
            .map_err(|_| JourneyError::WrongTarget)?;
        let _ = kernel.take_application_view(foreground);
        kernel
            .application_event(foreground, &encoded)
            .map_err(JourneyError::Play)?;
        let application_request = kernel.take_application_request(foreground);
        self.input_sign_id = Some(SignId::from(format!(
            "conduitos/product/input/{}/{}",
            play.active_play_id.as_str(),
            self.input_count
        )));
        self.input_count = next_count;
        self.advance()?;
        if application_request == Some(native_workset::NativeApplicationRequest::OpenPatchbay) {
            let patchbay = native_workset::resident(native_workset::NativePlot::Patchbay)
                .map_err(JourneyError::Workset)?;
            self.select_plot(&patchbay, self.revision)?;
        } else if application_request.is_some() {
            if self.application_request.is_some() {
                return Err(JourneyError::Play(
                    native_workset::PlayRefusal::InputPressure,
                ));
            }
            self.application_request = application_request;
        }
        Ok(true)
    }

    pub fn owns_key_release(&self, event: KeyEvent) -> bool {
        self.status == JourneyStatus::QuiescentAwaitingInput
            && self
                .kernel
                .as_ref()
                .is_some_and(|kernel| kernel.owns_release(event))
    }
    pub fn accept_play_input(&mut self, event: KeyEvent) -> Result<bool, JourneyError> {
        if self.status != JourneyStatus::QuiescentAwaitingInput {
            return Ok(false);
        }
        let next_count = self
            .input_count
            .checked_add(1)
            .ok_or(JourneyError::InputSequenceExhausted)?;
        self.revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        let foreground = self.foreground_index();
        let play = self
            .current_play()
            .cloned()
            .ok_or(JourneyError::InvalidTransition)?;
        let kernel = self.kernel.as_mut().ok_or(JourneyError::Kernel)?;
        let accepted = match kernel.input(foreground, event) {
            Ok(accepted) => accepted,
            Err(error) => {
                self.retire_realization()?;
                self.status = JourneyStatus::Stopped;
                self.advance()?;
                return Err(JourneyError::Play(error));
            }
        };
        if !accepted {
            return Ok(false);
        }
        self.input_sign_id = Some(SignId::from(format!(
            "conduitos/product/input/{}/{}",
            play.active_play_id.as_str(),
            self.input_count
        )));
        for index in 0..self.plots.len() {
            if let Some(value) = kernel.take_presentation(index) {
                self.results[index].record(
                    self.plots[index].ok_or(JourneyError::Kernel)?,
                    value,
                    SignId::from(format!(
                        "conduitos/product/result/{}/{}/{}",
                        play.active_play_id.as_str(),
                        index,
                        self.input_count
                    )),
                )?;
                self.results[index].input_sequence = Some(next_count);
            }
        }
        self.input_count = next_count;
        self.advance()?;
        Ok(true)
    }

    pub fn input_lost(&mut self, kind: JourneyLossKind) -> Result<(), JourneyError> {
        if !matches!(
            self.status,
            JourneyStatus::Planned | JourneyStatus::QuiescentAwaitingInput
        ) {
            return Err(JourneyError::InputUnavailable);
        }
        self.retire_realization()?;
        self.loss_kind = Some(kind);
        self.loss_sign_id = Some(SignId::from(format!(
            "conduitos/product/loss/{}/{}",
            kind.as_str(),
            self.revision
        )));
        self.status = JourneyStatus::InputUnavailable;
        self.advance()
    }

    pub(super) fn plan(
        &mut self,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        build_id: &str,
    ) -> Result<(), JourneyError> {
        if self.status == JourneyStatus::InputUnavailable {
            self.wake(identities, offer, build_id)?;
        }
        if self.status != JourneyStatus::Awake {
            return Err(JourneyError::InvalidTransition);
        }
        let current = self
            .session
            .as_ref()
            .and_then(|session| session.realization())
            .ok_or(JourneyError::BodyAbsent)?;
        if self.mask_control.is_some() {
            let selector = crate::mask_control::patchbay_selector(&current.plan)
                .map_err(|_| JourneyError::Kernel)?;
            if !current
                .plan
                .mask_topologies
                .iter()
                .any(|topology| topology.face == selector)
            {
                return Err(JourneyError::InvalidTransition);
            }
        }
        let prepared = native_workset::prepare_exact(
            &current.wake,
            &current.plan,
            identities,
            offer,
            build_id,
        )
        .map_err(JourneyError::Workset)?;
        let input_owners =
            core::array::from_fn(|index| prepared.input_owners().get(index).cloned());
        let kernel = Box::new(
            native_workset::NativeWorksetPlay::prepare_with_biography(
                &prepared,
                self.biography().ok_or(JourneyError::BodyAbsent)?,
            )
            .map_err(JourneyError::Workset)?,
        );
        self.results = core::array::from_fn(|_| super::PlotResult::new());
        self.input_count = 0;
        self.input_sign_id = None;
        self.loss_kind = None;
        self.loss_sign_id = None;
        self.retained_kernel_sign_gap = None;
        self.application_request = None;
        self.input_owners = input_owners;
        self.kernel = Some(kernel);
        self.status = JourneyStatus::Planned;
        Ok(())
    }

    pub(super) fn play(&mut self) -> Result<(), JourneyError> {
        if self.status != JourneyStatus::Planned {
            return Err(JourneyError::InvalidTransition);
        }
        let mut session = self.session.clone().ok_or(JourneyError::BodyAbsent)?;
        let current = session
            .realization()
            .ok_or(JourneyError::InvalidTransition)?;
        let plan = current.plan.clone();
        let play = BodyPlayIdentity::bind(&plan, self.revision);
        let sign = |sequence| {
            bind_sign(
                &self.host_id,
                &self.boot_id,
                Some(&play.active_play_id),
                sequence,
            )
            .sign_id
        };
        let wake = current
            .wake
            .body_plan_ready(&plan, sign(0))
            .and_then(|wake| wake.body_play_started(&plan, &play, sign(1)))
            .map_err(|_| JourneyError::InvalidTransition)?;
        // Record Play only after actual kernel start. Any refusal retires
        // partial execution before lulling the still-unstarted proposal.
        if let Err(error) = self.kernel.as_mut().ok_or(JourneyError::Kernel)?.start() {
            self.retire_realization()?;
            return Err(JourneyError::Play(error));
        }
        if let Err(error) =
            session.started(&self.host_id, &self.boot_id, play.clone(), wake.clone())
        {
            self.retire_realization()?;
            return Err(JourneyError::Lifecycle(error));
        }
        self.session = Some(session);
        self.status = JourneyStatus::QuiescentAwaitingInput;
        let activation = (|| {
            self.refresh_tutorial()?;
            if let Some(control) = self.mask_control.as_mut() {
                let topology = control
                    .activate(&wake, &plan, &play)
                    .map_err(|_| JourneyError::Kernel)?;
                self.kernel
                    .as_mut()
                    .ok_or(JourneyError::Kernel)?
                    .set_mask_topology(&topology)
                    .map_err(JourneyError::Mask)?;
            }
            Ok(())
        })();
        if let Err(error) = activation {
            self.retire_realization()?;
            return Err(error);
        }
        Ok(())
    }

    pub fn replan_masks(
        &mut self,
        request: patchbay_application::PatchbayApplicationRequest,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        build_id: &str,
    ) -> Result<(), JourneyError> {
        let (basis_plan_id, mode) = match request {
            patchbay_application::PatchbayApplicationRequest::ChangeMasks {
                body_plan_id,
                mode,
            } => (body_plan_id, mode),
            _ => return Err(JourneyError::WrongTarget),
        };
        if self.status != JourneyStatus::QuiescentAwaitingInput
            || self.current_plan().map(|plan| &plan.plan_id) != Some(&basis_plan_id)
        {
            return Err(JourneyError::StalePresentation);
        }
        self.revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        let mut control = self
            .mask_control
            .clone()
            .ok_or(JourneyError::InvalidTransition)?;
        control.request(mode).map_err(|_| JourneyError::Kernel)?;
        // The sole Play is retired before replacement preparation or start.
        // Any subsequent refusal leaves an explicit Lulled Body.
        self.retire_realization()?;
        self.propose(identities, offer, build_id, Some(control))?;
        self.plan(identities, offer, build_id)?;
        self.play()?;
        self.advance()
    }

    pub(super) fn retire_realization(&mut self) -> Result<(), JourneyError> {
        let receipt = self.current_play().cloned();
        if let Some(kernel) = self.kernel.as_mut() {
            kernel.cancel().map_err(JourneyError::Play)?;
            self.retained_kernel_sign_gap = kernel.sign_retention_gap();
        }
        self.kernel = None;
        self.application_request = None;
        if self.current_wake().is_some() {
            self.session
                .as_mut()
                .ok_or(JourneyError::BodyAbsent)?
                .lull(&self.host_id, &self.boot_id, receipt.as_ref())
                .map_err(JourneyError::Lifecycle)?;
        }
        self.status = JourneyStatus::Lulled;
        Ok(())
    }

    pub(super) fn stop(&mut self) -> Result<(), JourneyError> {
        if !matches!(
            self.status,
            JourneyStatus::QuiescentAwaitingInput | JourneyStatus::SemanticCompleted
        ) {
            return Err(JourneyError::InvalidTransition);
        }
        self.retire_realization()?;
        self.status = JourneyStatus::Stopped;
        Ok(())
    }

    pub(super) fn lull(&mut self) -> Result<(), JourneyError> {
        if !matches!(
            self.status,
            JourneyStatus::QuiescentAwaitingInput
                | JourneyStatus::SemanticCompleted
                | JourneyStatus::InputUnavailable
                | JourneyStatus::Stopped
        ) {
            return Err(JourneyError::InvalidTransition);
        }
        self.retire_realization()
    }
}
