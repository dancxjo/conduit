//! Correlation from an ordinary runtime manifestation back to an admitted model request.

use alloc::{format, vec};
use conduit_core::{Observation, ObservationKind, PlatformEffect};

use crate::{AuthorizedEffectRequest, EffectReceipt, ProposalGate, ProposalGateError};

impl ProposalGate {
    /// Records completion only when one actual runtime manifestation and its
    /// resulting Sign describe the exact same planned effect.
    pub fn complete_runtime_manifestation(
        &mut self,
        request: &AuthorizedEffectRequest,
        effect: &PlatformEffect,
        observation: &Observation,
    ) -> Result<EffectReceipt, ProposalGateError> {
        let PlatformEffect::PresentValue {
            plan_id,
            presentation_id,
            placement_id,
            presentation_kind,
            value,
            ..
        } = effect
        else {
            return Err(ProposalGateError::InvalidEffectReceipt);
        };
        let ObservationKind::ValuePresented {
            value: observed_value,
        } = &observation.kind
        else {
            return Err(ProposalGateError::InvalidEffectReceipt);
        };
        if plan_id != &request.plan_id
            || presentation_kind != &request.operation_kind
            || observation.plan_id.as_ref() != Some(plan_id)
            || observation.placement_id.as_ref() != Some(placement_id)
            || observation.presentation_id.as_ref() != Some(presentation_id)
            || observed_value != value
        {
            return Err(ProposalGateError::InvalidEffectReceipt);
        }
        self.complete(
            request,
            format!("effect/manifestation/{}", presentation_id.as_str()),
            vec![observation.sign_id.clone()],
        )
    }
}
