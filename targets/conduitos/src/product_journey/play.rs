use alloc::{boxed::Box, format, string::String};

use conduit_body::WakeLifecycle;
use conduit_core::SignId;
use conduit_human::KeyEvent;
use conduit_kernel::scheduler::SchedulerStatus;

use crate::{
    identity::BootIdentities,
    keyboard_text_plan,
    keyboard_text_play::{KeyboardTextKernel, KeyboardTextRequestKind},
    offer::HostOffer,
};

use super::{JourneyError, JourneyStatus, ProductJourney};

impl ProductJourney {
    pub fn accept_play_input(&mut self, event: KeyEvent) -> Result<bool, JourneyError> {
        if self.status != JourneyStatus::Playing {
            return Ok(false);
        }
        let request = self
            .pending_keyboard
            .take()
            .ok_or(JourneyError::InputUnavailable)?;
        let play = self.play.as_ref().ok_or(JourneyError::InvalidTransition)?;
        self.input_sign_id = Some(SignId::from(format!(
            "conduitos/product/input/{}/{}",
            play.active_play_id.as_str(),
            self.input_count
        )));
        self.input_count = self
            .input_count
            .checked_add(1)
            .ok_or(JourneyError::Kernel)?;
        self.kernel
            .as_mut()
            .ok_or(JourneyError::Kernel)?
            .complete_keyboard(request, event)
            .map_err(|_| JourneyError::Kernel)?;
        self.drive_kernel()?;
        self.advance()?;
        Ok(true)
    }

    pub fn input_lost(&mut self) -> Result<(), JourneyError> {
        if self.status == JourneyStatus::Planned {
            self.kernel = None;
            self.planned_play = None;
            self.status = JourneyStatus::Stopped;
            return self.advance();
        }
        let request = self
            .pending_keyboard
            .take()
            .ok_or(JourneyError::InputUnavailable)?;
        self.kernel
            .as_mut()
            .ok_or(JourneyError::Kernel)?
            .fail_keyboard_device_removed(request)
            .map_err(|_| JourneyError::Kernel)?;
        self.kernel = None;
        self.status = JourneyStatus::Stopped;
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
        let prepared =
            keyboard_text_plan::prepare(identities, offer, build_id).map_err(JourneyError::Plan)?;
        if prepared.source_document_id != self.form.source_document_id
            || prepared.checked_form_id != self.form.checked_form_id
            || prepared.expanded_form_id != self.form.expanded_form_id
        {
            return Err(JourneyError::WrongTarget);
        }
        let kernel =
            Box::new(KeyboardTextKernel::prepare(&prepared, 2).map_err(JourneyError::Plan)?);
        self.wake = Some(
            wake.plan_ready(
                &prepared.plan,
                SignId::from(format!("conduitos/product/planned/{}", self.revision)),
            )
            .map_err(|_| JourneyError::InvalidTransition)?,
        );
        self.plan = Some(prepared.plan);
        self.planned_play = Some(prepared.active_play);
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
        self.wake = Some(
            wake.play_started(
                play,
                SignId::from(format!("conduitos/product/playing/{}", self.revision)),
            )
            .map_err(|_| JourneyError::InvalidTransition)?,
        );
        self.play = Some(play.clone());
        self.status = JourneyStatus::Playing;
        self.drive_kernel()
    }

    pub(super) fn stop(&mut self) -> Result<(), JourneyError> {
        if !matches!(
            self.status,
            JourneyStatus::Playing | JourneyStatus::ResultVisible
        ) {
            return Err(JourneyError::InvalidTransition);
        }
        if self.status == JourneyStatus::Playing
            && let Some(kernel) = self.kernel.as_mut()
        {
            kernel.cancel().map_err(|_| JourneyError::Kernel)?;
        }
        self.kernel = None;
        self.pending_keyboard = None;
        self.status = JourneyStatus::Stopped;
        Ok(())
    }

    pub(super) fn lull(&mut self) -> Result<(), JourneyError> {
        if !matches!(
            self.status,
            JourneyStatus::ResultVisible | JourneyStatus::Stopped
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
        self.body = Some(
            body.retain_after_lull(
                &lulled,
                SignId::from(format!("conduitos/product/body-retained/{}", self.revision)),
            )
            .map_err(|_| JourneyError::InvalidTransition)?,
        );
        self.wake = Some(lulled);
        self.status = JourneyStatus::Lulled;
        Ok(())
    }

    fn drive_kernel(&mut self) -> Result<(), JourneyError> {
        for _ in 0..256 {
            while let Some(request) = self
                .kernel
                .as_mut()
                .ok_or(JourneyError::Kernel)?
                .next_host_request()
            {
                let kind = self
                    .kernel
                    .as_ref()
                    .ok_or(JourneyError::Kernel)?
                    .request_kind(request)
                    .map_err(|_| JourneyError::Kernel)?;
                match kind {
                    KeyboardTextRequestKind::Keyboard => {
                        self.pending_keyboard = Some(request);
                        return Ok(());
                    }
                    KeyboardTextRequestKind::Keymap => self
                        .kernel
                        .as_mut()
                        .ok_or(JourneyError::Kernel)?
                        .complete_keymap(request)
                        .map_err(|_| JourneyError::Kernel)?,
                    KeyboardTextRequestKind::Upper => self
                        .kernel
                        .as_mut()
                        .ok_or(JourneyError::Kernel)?
                        .complete_upper(request)
                        .map_err(|_| JourneyError::Kernel)?,
                    KeyboardTextRequestKind::Presentation => {
                        let fragment = self
                            .kernel
                            .as_mut()
                            .ok_or(JourneyError::Kernel)?
                            .complete_presentation(request)
                            .map_err(|_| JourneyError::Kernel)?;
                        let value = core::str::from_utf8(fragment.as_bytes())
                            .map_err(|_| JourneyError::Kernel)?;
                        self.result.get_or_insert_with(String::new).push_str(value);
                        let play = self.play.as_ref().ok_or(JourneyError::Kernel)?;
                        self.result_sign_id = Some(SignId::from(format!(
                            "conduitos/product/result/{}/{}",
                            play.active_play_id.as_str(),
                            self.input_count
                        )));
                    }
                }
            }
            match self
                .kernel
                .as_mut()
                .ok_or(JourneyError::Kernel)?
                .step()
                .map_err(|_| JourneyError::Kernel)?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => return Err(JourneyError::Kernel),
                SchedulerStatus::Complete => {
                    self.status = JourneyStatus::ResultVisible;
                    return Ok(());
                }
                SchedulerStatus::Cancelled => {
                    self.status = JourneyStatus::Stopped;
                    return Ok(());
                }
            }
        }
        Err(JourneyError::Kernel)
    }
}
