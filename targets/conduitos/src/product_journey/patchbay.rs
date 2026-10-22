//! Current resident Patchbay meaning and effects through its admitted Fore.
use super::*;
use conduit_face::{Face, FaceContributionBasis, FaceFragment, FaceInteraction, MaskShow};
use conduitos_application_core::data::CanonicalRefusal;

impl ProductJourney {
    pub fn foreground_canonical_patchbay_active(&self) -> bool {
        self.status == JourneyStatus::QuiescentAwaitingInput
            && self
                .kernel
                .as_ref()
                .is_some_and(|kernel| kernel.has_patchbay_fore(self.foreground_index()))
    }
    fn patchbay_basis(&self) -> Result<FaceContributionBasis, JourneyError> {
        if !self.foreground_canonical_patchbay_active() {
            return Err(JourneyError::InputUnavailable);
        }
        let play = self.current_play().ok_or(JourneyError::InvalidTransition)?;
        Ok(FaceContributionBasis {
            checked_plot_id: self
                .plot
                .as_ref()
                .ok_or(JourneyError::PlotNotOpened)?
                .checked_plot_id
                .clone(),
            plan_id: play.plan_id.clone(),
            active_play_id: play.active_play_id.clone(),
            required_interaction_context: None,
        })
    }
    pub fn foreground_patchbay_fragment(&self) -> Result<Option<FaceFragment>, JourneyError> {
        if !self.foreground_canonical_patchbay_active() {
            return Ok(None);
        }
        let body = self.body().ok_or(JourneyError::BodyAbsent)?.body_id.clone();
        self.kernel
            .as_ref()
            .ok_or(JourneyError::Kernel)?
            .patchbay_fragment(
                self.foreground_index(),
                body,
                self.patchbay_basis()?,
                self.application_request.is_none(),
            )
            .map(Some)
            .map_err(JourneyError::Play)
    }
    pub fn accept_patchbay_interaction(
        &mut self,
        face: Face,
        show: MaskShow,
        input: &FaceInteraction,
    ) -> Result<Option<CanonicalRefusal>, JourneyError> {
        let basis = self.patchbay_basis()?;
        if self.application_request.is_some() {
            return Err(JourneyError::Play(
                native_workset::PlayRefusal::InputPressure,
            ));
        }
        let next = self
            .input_count
            .checked_add(1)
            .ok_or(JourneyError::InputSequenceExhausted)?;
        self.revision
            .checked_add(1)
            .ok_or(JourneyError::RevisionExhausted)?;
        let body = self.body().ok_or(JourneyError::BodyAbsent)?.body_id.clone();
        let foreground = self.foreground_index();
        let kernel = self.kernel.as_mut().ok_or(JourneyError::Kernel)?;
        let refusal =
            match kernel.patchbay_interaction(foreground, body, basis.clone(), face, show, input) {
                Ok(refusal) => refusal,
                Err(error @ native_workset::PlayRefusal::InputPressure) => {
                    return Err(JourneyError::Play(error));
                }
                Err(error) => {
                    self.retire_realization()?;
                    self.status = JourneyStatus::Stopped;
                    self.advance()?;
                    return Err(JourneyError::Play(error));
                }
            };
        let request = kernel.take_application_request(foreground);
        self.input_sign_id = Some(SignId::from(format!(
            "conduitos/product/input/{}/{}",
            basis.active_play_id.as_str(),
            self.input_count
        )));
        self.input_count = next;
        self.advance()?;
        self.retain_application_request(request)?;
        Ok(refusal)
    }
    pub(super) fn retain_application_request(
        &mut self,
        request: Option<native_workset::NativeApplicationRequest>,
    ) -> Result<(), JourneyError> {
        self.patchbay_request_authorization = None;
        if request == Some(native_workset::NativeApplicationRequest::OpenPatchbay) {
            let patchbay = native_workset::resident(native_workset::NativePlot::Patchbay)
                .map_err(JourneyError::Workset)?;
            self.select_plot(&patchbay, self.revision)?;
        } else if let Some(request) = request {
            if self.application_request.is_some() {
                return Err(JourneyError::Play(
                    native_workset::PlayRefusal::InputPressure,
                ));
            }
            self.patchbay_request_authorization = match &request {
                native_workset::NativeApplicationRequest::EditCurrent(request) => {
                    super::patchbay::request_digest(request).map(|digest| (digest, self.revision))
                }
                _ => None,
            };
            self.application_request = Some(request);
        }
        Ok(())
    }
}

/// One fixed receipt of the exact owner-issued fields. Plan identity alone is
/// not authority to edit Source or change the Body's admitted Masks.
pub(super) fn request_digest(
    request: &patchbay_application::PatchbayApplicationRequest,
) -> Option<[u8; 32]> {
    use patchbay_application::{PatchbayApplicationRequest, PatchbayMaskMode};
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    match request {
        PatchbayApplicationRequest::EditCurrent {
            expanded_plot_id,
            subject_identity,
        } => {
            if subject_identity.len() > 256 || expanded_plot_id.as_str().len() > 256 {
                return None;
            }
            hash.update(b"conduitos/issued-source-edit@1");
            for field in [expanded_plot_id.as_str(), subject_identity.as_str()] {
                hash.update((field.len() as u64).to_le_bytes());
                hash.update(field.as_bytes());
            }
        }
        PatchbayApplicationRequest::ChangeMasks { body_plan_id, mode } => {
            if body_plan_id.as_str().len() > 256 {
                return None;
            }
            hash.update(b"conduitos/issued-mask-change@1");
            hash.update((body_plan_id.as_str().len() as u64).to_le_bytes());
            hash.update(body_plan_id.as_str().as_bytes());
            hash.update([match mode {
                PatchbayMaskMode::Graphical => 0,
                PatchbayMaskMode::GraphicalAndSpeech => 1,
                PatchbayMaskMode::Speech => 2,
            }]);
        }
    }
    Some(hash.finalize().into())
}
