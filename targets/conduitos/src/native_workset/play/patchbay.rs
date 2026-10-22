//! Canonical resident ingress in the existing admitted Body scheduler.
use super::*;
use conduit_body::BodyId;
use conduit_face::{Face, FaceContributionBasis, FaceFragment, FaceInteraction, MaskShow};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduitos_application_core::data::{CanonicalRefusal, Reply};

pub(super) struct IngressState {
    pub binding: super::super::patchbay_ingress::Ingress,
    sequence: u64,
    uncertain: bool,
    refusal: Option<CanonicalRefusal>,
}
impl IngressState {
    pub fn new(binding: super::super::patchbay_ingress::Ingress) -> Self {
        Self {
            binding,
            sequence: 0,
            uncertain: false,
            refusal: None,
        }
    }
}

impl NativeWorksetPlay {
    pub fn has_patchbay_fore(&self, plot: usize) -> bool {
        !self.cancelled
            && self.applications_live()
            && self.patchbay_inputs.get(plot).is_some_and(Option::is_some)
    }
    fn patchbay_basis(
        &self,
        plot: usize,
        body: &BodyId,
        basis: &FaceContributionBasis,
    ) -> Result<(), PlayRefusal> {
        let input = self
            .patchbay_inputs
            .get(plot)
            .and_then(Option::as_ref)
            .ok_or(PlayRefusal::InputOwnership)?;
        let play = self
            .active_body_play
            .as_ref()
            .ok_or(PlayRefusal::Preparation)?;
        if self.cancelled
            || !self.applications_live()
            || input.uncertain
            || body != &play.body_id
            || basis.plan_id != self.admitted_plan
            || basis.active_play_id != play.active_play_id
            || basis.checked_plot_id != input.binding.checked_plot_id
        {
            return Err(PlayRefusal::InputOwnership);
        }
        Ok(())
    }
    pub fn patchbay_fragment(
        &self,
        plot: usize,
        body: BodyId,
        basis: FaceContributionBasis,
    ) -> Result<FaceFragment, PlayRefusal> {
        self.patchbay_basis(plot, &body, &basis)?;
        let actions = self.application_requests[plot].is_none();
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        let result = self
            .applications
            .patchbay_fragment(plot, body, basis, actions);
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        let result = self
            .applications
            .patchbay_fragment(plot as u8, body, basis, actions);
        match result.map_err(|_| PlayRefusal::Kernel)? {
            Reply::PatchbayFragment { slot, fragment, .. } if usize::from(slot) == plot => {
                Ok(fragment)
            }
            _ => Err(PlayRefusal::Kernel),
        }
    }
    /// Publication is supplied only by the current independently acknowledged
    /// Mask session. The actual bytes then enter the exact resident input Cord.
    pub fn patchbay_interaction(
        &mut self,
        plot: usize,
        body: BodyId,
        basis: FaceContributionBasis,
        face: Face,
        show: MaskShow,
        input: &FaceInteraction,
    ) -> Result<Option<CanonicalRefusal>, PlayRefusal> {
        self.ensure_applications_live()?;
        self.patchbay_basis(plot, &body, &basis)?;
        if self.application_requests[plot].is_some() {
            return Err(PlayRefusal::InputPressure);
        }
        if self.patchbay_inputs[plot]
            .as_ref()
            .is_none_or(|state| state.sequence >= u64::from(u32::MAX))
        {
            return Err(PlayRefusal::WorkBound);
        }
        let route = &self.patchbay_inputs[plot]
            .as_ref()
            .ok_or(PlayRefusal::InputOwnership)?
            .binding;
        if show.show.host_id != route.host_id
            || show.show.boot_id != route.boot_id
            || show.show.offer_generation != route.offer_generation
            || show.planned_mask.plan.fragments.iter().any(|mask| {
                mask.host_id != route.host_id
                    || mask.boot_id != route.boot_id
                    || mask.offer_generation != route.offer_generation
            })
        {
            return Err(PlayRefusal::InputOwnership);
        }
        input
            .validate_against(&face, &show)
            .map_err(|_| PlayRefusal::InputOwnership)?;
        let bytes = input.encode();
        route
            .input
            .validate_value(&bytes)
            .map_err(|_| PlayRefusal::InputOwnership)?;
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        let published = self
            .applications
            .patchbay_publish(plot, body, basis, face, show);
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        let published = self
            .applications
            .patchbay_publish(plot as u8, body, basis, face, show);
        match self.application_result(published)? {
            Reply::PatchbayPublished { slot, .. } if usize::from(slot) == plot => {}
            Reply::PatchbayRefused { slot, refusal, .. } if usize::from(slot) == plot => {
                return Ok(Some(refusal));
            }
            _ => return Err(PlayRefusal::Kernel),
        }
        let (endpoint, cord, sequence) = {
            let state = self.patchbay_inputs[plot]
                .as_mut()
                .ok_or(PlayRefusal::InputOwnership)?;
            // Fence before crossing the transport. Only a confirmed Full may
            // reopen custody without consuming an input occurrence.
            state.uncertain = true;
            state.refusal = None;
            (
                state.binding.input.endpoint,
                state.binding.input.cord,
                state.sequence,
            )
        };
        let admitted = match self
            .scheduler
            .admit_remote_input(endpoint, cord, sequence, &bytes)
        {
            Ok(admitted) => admitted,
            Err(error) => {
                let _ = self.cancel();
                return Err(PlayRefusal::Scheduler(error));
            }
        };
        match admitted {
            RemoteIngressOutcome::Full { sequence: observed } if observed == sequence => {
                self.patchbay_inputs[plot]
                    .as_mut()
                    .ok_or(PlayRefusal::InputOwnership)?
                    .uncertain = false;
                return Err(PlayRefusal::InputPressure);
            }
            RemoteIngressOutcome::Accepted { sequence: observed } if observed == sequence => {
                self.patchbay_inputs[plot]
                    .as_mut()
                    .ok_or(PlayRefusal::InputOwnership)?
                    .sequence += 1;
            }
            _ => {
                let _ = self.cancel();
                return Err(PlayRefusal::Kernel);
            }
        }
        // No retry after acceptance. Drive the same scheduler through the real
        // owner Host Call and its completion before releasing input custody.
        if let Err(error) = self.drive() {
            if !self.cancelled {
                let _ = self.cancel();
            }
            return Err(error);
        }
        let state = self.patchbay_inputs[plot]
            .as_mut()
            .ok_or(PlayRefusal::InputOwnership)?;
        state.uncertain = false;
        Ok(state.refusal.take())
    }
    pub(super) fn consume_patchbay(
        &mut self,
        plot: usize,
        request: HostCallRequest,
    ) -> Result<(), PlayRefusal> {
        let state = self
            .patchbay_inputs
            .get(plot)
            .and_then(Option::as_ref)
            .ok_or(PlayRefusal::InputOwnership)?;
        if !state.uncertain
            || request.node != state.binding.node
            || self.application_requests[plot].is_some()
        {
            return Err(PlayRefusal::InputOwnership);
        }
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(PlayRefusal::Scheduler)?;
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        let result = self.applications.patchbay_consume(plot, input);
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        let result = self.applications.patchbay_consume(plot as u8, input);
        let Reply::PatchbayConsumed {
            slot,
            proposal,
            refusal,
            ..
        } = self.application_result(result)?
        else {
            return Err(PlayRefusal::Kernel);
        };
        if usize::from(slot) != plot || (refusal.is_some() && proposal.is_some()) {
            return Err(PlayRefusal::Kernel);
        }
        self.output(request, None)?;
        self.application_requests[plot] = proposal;
        self.patchbay_inputs[plot]
            .as_mut()
            .ok_or(PlayRefusal::InputOwnership)?
            .refusal = refusal;
        Ok(())
    }
}
