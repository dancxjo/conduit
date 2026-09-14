//! One admitted native Body Plan/Play across exact resident Form partitions.
use super::{JourneyError, JourneyLossKind, JourneyStatus, ProductJourney};
use crate::{identity::BootIdentities, native_workset, offer::HostOffer};
use alloc::{boxed::Box, format};
use conduit_body::{BodyPlayIdentity, WakeLifecycle};
use conduit_core::SignId;
use conduit_human::KeyEvent;
use conduit_presentation::{ApplicationEvent, ApplicationView};

impl ProductJourney {
    pub fn foreground_input_owner(&self) -> Option<&native_workset::AdmittedFormInput> {
        (self.status == JourneyStatus::QuiescentAwaitingInput)
            .then(|| self.kernel.as_ref()?.input_owner(self.foreground))?
    }

    pub fn foreground_application_view(&self) -> Option<&ApplicationView> {
        (self.status == JourneyStatus::QuiescentAwaitingInput)
            .then(|| self.kernel.as_ref()?.application_view(self.foreground))?
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
            .forms
            .iter()
            .position(|form| *form == Some(native_workset::NativeForm::Tour))
            .ok_or(JourneyError::Kernel)?;
        self.kernel
            .as_mut()
            .ok_or(JourneyError::Kernel)?
            .complete_tour_run(
                tour,
                conduit_tour_model::TourRunProof {
                    specimen_id: conduit_tour_model::CANONICAL_SPECIMEN_ID.into(),
                    source_document_id: evidence.source_document_id.clone(),
                    checked_form_id: evidence.checked_form_id.clone(),
                    expanded_form_id: evidence.expanded_form_id.clone(),
                    plan_id: evidence.plan_id.clone(),
                    active_play_id: evidence.active_play_id.clone(),
                    result: evidence.result.into(),
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
        let kernel = self.kernel.as_mut().ok_or(JourneyError::Kernel)?;
        let current = kernel
            .application_view(self.foreground)
            .ok_or(JourneyError::InputUnavailable)?;
        let encoded = event
            .encode(current)
            .map_err(|_| JourneyError::WrongTarget)?;
        let _ = kernel.take_application_view(self.foreground);
        kernel
            .application_event(self.foreground, &encoded)
            .map_err(JourneyError::Play)?;
        let application_request = kernel.take_application_request(self.foreground);
        let play = self.play.as_ref().ok_or(JourneyError::InvalidTransition)?;
        self.input_sign_id = Some(SignId::from(format!(
            "conduitos/product/input/{}/{}",
            play.active_play_id.as_str(),
            self.input_count
        )));
        self.input_count = next_count;
        self.advance()?;
        if application_request == Some(native_workset::NativeApplicationRequest::OpenPatchbay) {
            let patchbay = native_workset::resident(native_workset::NativeForm::Patchbay)
                .map_err(JourneyError::Workset)?;
            self.select_form(&patchbay, self.revision)?;
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
        let kernel = self.kernel.as_mut().ok_or(JourneyError::Kernel)?;
        let accepted = match kernel.input(self.foreground, event) {
            Ok(accepted) => accepted,
            Err(error) => {
                kernel.cancel().map_err(JourneyError::Play)?;
                self.retained_kernel_sign_gap = kernel.sign_retention_gap();
                self.kernel = None;
                self.application_request = None;
                self.status = JourneyStatus::Stopped;
                self.advance()?;
                return Err(JourneyError::Play(error));
            }
        };
        if !accepted {
            return Ok(false);
        }
        let play = self.play.as_ref().ok_or(JourneyError::InvalidTransition)?;
        self.input_sign_id = Some(SignId::from(format!(
            "conduitos/product/input/{}/{}",
            play.active_play_id.as_str(),
            self.input_count
        )));
        for index in 0..self.forms.len() {
            if let Some(value) = kernel.take_presentation(index) {
                self.results[index].record(
                    self.forms[index].ok_or(JourneyError::Kernel)?,
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
        if let Some(kernel) = self.kernel.as_mut() {
            kernel.input_lost().map_err(JourneyError::Play)?;
            self.retained_kernel_sign_gap = kernel.sign_retention_gap();
        }
        self.kernel = None;
        self.application_request = None;
        self.planned_play = None;
        self.play = None;
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
        let wake = self.wake.as_ref().ok_or(JourneyError::BodyAbsent)?;
        if wake.lifecycle != WakeLifecycle::AwaitingPlan {
            return Err(JourneyError::InvalidTransition);
        }
        let prepared = native_workset::prepare(wake, identities, offer, build_id)
            .map_err(JourneyError::Workset)?;
        if prepared.advertisement().host_id != self.host_id
            || prepared.advertisement().boot_id != self.boot_id
            || prepared.advertisement().offer_generation != self.offer_generation
        {
            return Err(JourneyError::WrongTarget);
        }
        let input_owners =
            core::array::from_fn(|index| prepared.input_owners().get(index).cloned());
        let kernel = Box::new(
            native_workset::NativeWorksetPlay::prepare(&prepared).map_err(JourneyError::Workset)?,
        );
        let plan = prepared.into_plan();
        self.wake = Some(
            wake.body_plan_ready(
                &plan,
                SignId::from(format!("conduitos/product/planned/{}", self.revision)),
            )
            .map_err(|_| JourneyError::InvalidTransition)?,
        );
        self.results = core::array::from_fn(|_| super::FormResult::new());
        self.input_count = 0;
        self.input_sign_id = None;
        self.loss_kind = None;
        self.loss_sign_id = None;
        self.retained_kernel_sign_gap = None;
        self.application_request = None;
        self.planned_play = Some(BodyPlayIdentity::bind(&plan, self.revision));
        self.plan = Some(plan);
        self.input_owners = input_owners;
        self.kernel = Some(kernel);
        self.status = JourneyStatus::Planned;
        Ok(())
    }

    pub(super) fn play(&mut self) -> Result<(), JourneyError> {
        let wake = self.wake.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let play = self
            .planned_play
            .as_ref()
            .ok_or(JourneyError::InvalidTransition)?;
        let plan = self.plan.as_ref().ok_or(JourneyError::InvalidTransition)?;
        let wake = wake
            .body_play_started(
                plan,
                play,
                SignId::from(format!("conduitos/product/playing/{}", self.revision)),
            )
            .map_err(|_| JourneyError::InvalidTransition)?;
        self.kernel
            .as_mut()
            .ok_or(JourneyError::Kernel)?
            .start()
            .map_err(JourneyError::Play)?;
        self.wake = Some(wake);
        self.play = Some(play.clone());
        // This Form has no checked completion witness. Its initial structural
        // drain leaves the admitted Play resident and awaiting later input.
        self.status = JourneyStatus::QuiescentAwaitingInput;
        Ok(())
    }

    pub(super) fn stop(&mut self) -> Result<(), JourneyError> {
        if !matches!(
            self.status,
            JourneyStatus::QuiescentAwaitingInput | JourneyStatus::SemanticCompleted
        ) {
            return Err(JourneyError::InvalidTransition);
        }
        if let Some(kernel) = self.kernel.as_mut() {
            kernel.cancel().map_err(JourneyError::Play)?;
            self.retained_kernel_sign_gap = kernel.sign_retention_gap();
        }
        self.kernel = None;
        self.application_request = None;
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
        let wake = self.wake.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let lulled = wake
            .lull(SignId::from(format!(
                "conduitos/product/lulled/{}",
                self.revision
            )))
            .map_err(|_| JourneyError::InvalidTransition)?;
        let body = self.body.as_ref().ok_or(JourneyError::BodyAbsent)?;
        let retained = body
            .retain_after_lull(
                &lulled,
                SignId::from(format!("conduitos/product/body-retained/{}", self.revision)),
            )
            .map_err(|_| JourneyError::InvalidTransition)?;
        // Prepare the biography transition first; publish it only after the
        // actual kernel has retired every pending operation and owned value.
        if let Some(kernel) = self.kernel.as_mut() {
            kernel.cancel().map_err(JourneyError::Play)?;
            self.retained_kernel_sign_gap = kernel.sign_retention_gap();
        }
        self.kernel = None;
        self.application_request = None;
        self.body = Some(retained);
        self.wake = Some(lulled);
        self.status = JourneyStatus::Lulled;
        Ok(())
    }
}
