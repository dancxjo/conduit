//! Request-admitted input origin, not an action mapping or physical-device receipt.
//!
//! Descriptors exist before Play, but become current only when the installed
//! scheduler's exact input request is accepted. Body origins additionally require
//! confirmation of their prospective identity inside reserved execution.
use conduit_core::{ActivePlayIdentity, HostAdvertisement, PlanFragment, PlannedGear};
use conduit_kernel::scheduler::HostCallRequest;
use conduit_plan_lowering::lowering::{KernelIdentityMap, LoweredHostCall};

pub(super) fn has_input_source(operations: &[LoweredHostCall]) -> bool {
    operations
        .iter()
        .any(|operation| InputRealization::from_contract(operation.contract_id.as_str()).is_some())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InputRealization {
    RawKeyboard,
    SpaceButton,
}

impl InputRealization {
    pub(super) fn from_contract(contract: &str) -> Option<Self> {
        match contract {
            conduit_std_offers::NEXT_KEY_EVENT_HOST_CALL_CONTRACT => Some(Self::RawKeyboard),
            conduit_std_offers::button::NEXT_TRANSITION_HOST_CALL => Some(Self::SpaceButton),
            _ => None,
        }
    }
}

struct InputDescriptor {
    placement: PlannedGear,
    operation: LoweredHostCall,
    input_plan: conduit_core::PlanId,
    realization: InputRealization,
    state: RequestAdmission,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestAdmission {
    Prepared,
    Current(conduit_kernel::RequestId),
    Revoked,
}

/// Private origin custody. No public IDs, proposals or adapter-existence flag
/// can construct this value. It deliberately has no current-map witness API.
pub(super) struct KeyboardRequestAdmission {
    descriptors: Vec<InputDescriptor>,
    host: HostAdvertisement,
    body_plan: Option<conduit_body::BodyPlan>,
    fragment_play: Option<ActivePlayIdentity>,
    body_play: Option<conduit_body::BodyPlayIdentity>,
    issued: bool,
    revoked: bool,
    pub(super) completions: super::keyboard_input_completion::KeyboardInputCompletions,
}

impl KeyboardRequestAdmission {
    /// Called only from the ordinary installed run after reservation and issued
    /// Play. Preparation validates realization but never activates authority.
    pub(super) fn prepare(
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
        operations: &[LoweredHostCall],
        play: &ActivePlayIdentity,
    ) -> Result<Self, String> {
        if fragment.host_id != advertisement.host_id
            || fragment.boot_id != advertisement.boot_id
            || fragment.offer_generation != advertisement.offer_generation
            || identity.plan_id != fragment.plan_id
            || identity.fragment_id != fragment.fragment_id
            || play.plan_id != fragment.plan_id
            || play.host_id != advertisement.host_id
            || play.boot_id != advertisement.boot_id
            || *play
                != conduit_core::bind_active_play(
                    &fragment.plan_id,
                    &advertisement.host_id,
                    &advertisement.boot_id,
                    play.play_sequence,
                )
        {
            return Err("input origin has stale admitted Host/Boot/Plan custody".into());
        }
        let mut admission = Self {
            descriptors: Vec::new(),
            host: advertisement.clone(),
            body_plan: None,
            fragment_play: Some(play.clone()),
            body_play: None,
            issued: true,
            revoked: false,
            completions: super::keyboard_input_completion::KeyboardInputCompletions::new(),
        };
        admission.prepare_fragment(advertisement, fragment, identity, operations)?;
        admission.require_all_input_operations(operations)?;
        Ok(admission)
    }

    pub(super) fn prepare_body(
        advertisement: &HostAdvertisement,
        plan: &conduit_body::BodyPlan,
        prospective: &conduit_body::BodyPlayIdentity,
        identities: &[KernelIdentityMap],
        operations: &[LoweredHostCall],
    ) -> Result<Self, String> {
        if !prospective.validate_for(plan) || identities.len() != plan.plots.len() {
            return Err("input origin has a different prospective Body Play/partition set".into());
        }
        let mut admission = Self {
            descriptors: Vec::new(),
            host: advertisement.clone(),
            body_plan: Some(plan.clone()),
            fragment_play: None,
            body_play: Some(prospective.clone()),
            issued: false,
            revoked: false,
            completions: super::keyboard_input_completion::KeyboardInputCompletions::new(),
        };
        for (partition, identity) in plan.plots.iter().zip(identities) {
            if !conduit_core::verify_plan(&partition.plan) {
                return Err("Body input origin requires exact whole partition Plan".into());
            }
            let fragment = partition
                .plan
                .fragments
                .first()
                .filter(|_| partition.plan.fragments.len() == 1)
                .ok_or("Body input origin requires one exact local partition fragment")?;
            admission.prepare_fragment(advertisement, fragment, identity, operations)?;
        }
        admission.require_all_input_operations(operations)?;
        Ok(admission)
    }

    fn require_all_input_operations(&self, operations: &[LoweredHostCall]) -> Result<(), String> {
        if self.descriptors.len()
            != operations
                .iter()
                .filter(|operation| {
                    InputRealization::from_contract(operation.contract_id.as_str()).is_some()
                })
                .count()
        {
            return Err("input operation is absent from exact partition custody".into());
        }
        Ok(())
    }

    pub(super) fn confirm_body_play(
        &mut self,
        advertisement: &HostAdvertisement,
        issued: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), String> {
        if self.revoked
            || self.issued
            || self.body_play.as_ref() != Some(issued)
            || advertisement != &self.host
            || self.descriptors.iter().any(|descriptor| {
                descriptor.placement.host_id != advertisement.host_id
                    || descriptor.placement.boot_id != advertisement.boot_id
                    || descriptor.placement.offer_generation != advertisement.offer_generation
                    || descriptor.state != RequestAdmission::Prepared
            })
        {
            self.revoke();
            return Err("input origin cannot confirm changed or revoked Body Play/Host".into());
        }
        self.issued = true;
        Ok(())
    }

    fn prepare_fragment(
        &mut self,
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
        operations: &[LoweredHostCall],
    ) -> Result<(), String> {
        if identity.plan_id != fragment.plan_id
            || identity.fragment_id != fragment.fragment_id
            || fragment.host_id != advertisement.host_id
            || fragment.boot_id != advertisement.boot_id
            || fragment.offer_generation != advertisement.offer_generation
        {
            return Err("input partition differs from current Host/Boot/Plan".into());
        }
        for operation in operations {
            let Some(realization) = InputRealization::from_contract(operation.contract_id.as_str())
            else {
                continue;
            };
            let Some(placement_id) = identity.placement_for_node(operation.node) else {
                continue;
            };
            let placement = fragment
                .placements
                .iter()
                .find(|placement| &placement.placement_id == placement_id)
                .ok_or("input origin placement is absent from admitted fragment")?;
            if placement.host_id != advertisement.host_id
                || placement.boot_id != advertisement.boot_id
                || placement.offer_generation != advertisement.offer_generation
                || operation.call != operation.binding.call
                || operation.binding.maximum_input_bytes != 0
                || operation.maximum_in_flight != 1
                || !identity.host_calls.iter().any(|(node, call, contract)| {
                    *node == operation.node
                        && *call == operation.call
                        && contract == &operation.contract_id
                })
                || !placement.host_calls.iter().any(|requirement| {
                    requirement.contract_id == operation.contract_id
                        && requirement.target_kind == operation.target_kind
                        && requirement.maximum_in_flight == operation.maximum_in_flight
                        && requirement.maximum_input_bytes == operation.binding.maximum_input_bytes
                        && requirement.maximum_output_bytes
                            == operation.binding.maximum_output_bytes
                })
            {
                return Err("input origin differs from exact lowered Host call/placement".into());
            }
            let factory = match realization {
                InputRealization::RawKeyboard => &super::keyboard_input_back::FACTORY,
                InputRealization::SpaceButton => &super::keyboard_input_back::button::FACTORY,
            };
            (factory.budget)(placement)?;
            if !advertisement.capabilities.iter().any(|offer| {
                offer.capability_id == placement.capability_id
                    && offer.kind_id == placement.kind_id
                    && offer.kind_contract_revision == placement.kind_contract_revision
                    && offer.implementation.execution_profile_id == placement.execution_profile_id
                    && offer.implementation.implementation_id == placement.implementation_id
                    && offer.implementation.artifact_id == placement.artifact_id
                    && offer.inputs == placement.inputs
                    && offer.outputs == placement.outputs
                    && offer.semantic_contract == placement.semantic_contract
                    && offer.limits == placement.limits
                    && offer.host_calls == placement.host_calls
            }) {
                return Err("input origin Back is absent from current Host offers".into());
            }
            if self.descriptors.len() >= super::MAX_NODES
                || self.descriptors.iter().any(|descriptor| {
                    descriptor.operation.node == operation.node
                        && descriptor.operation.call == operation.call
                })
            {
                return Err("input origin has duplicate or excessive source placements".into());
            }
            self.descriptors.push(InputDescriptor {
                placement: placement.clone(),
                operation: operation.clone(),
                input_plan: fragment.plan_id.clone(),
                realization,
                state: RequestAdmission::Prepared,
            });
        }
        Ok(())
    }

    pub(super) fn accept(
        &mut self,
        realization: InputRealization,
        request: HostCallRequest,
    ) -> Result<(), String> {
        if self.revoked || !self.issued {
            self.revoke();
            return Err("input origin has no issued Play".into());
        }
        let Some(descriptor) = self.descriptors.iter_mut().find(|descriptor| {
            descriptor.realization == realization
                && descriptor.operation.node == request.node
                && descriptor.operation.call == request.call
        }) else {
            self.revoke();
            return Err("input request has no prepared origin".into());
        };
        if request.node != descriptor.operation.node
            || request.call != descriptor.operation.call
            || request.input.value.byte_len != 0
            || matches!(descriptor.state, RequestAdmission::Revoked)
            || matches!(descriptor.state, RequestAdmission::Current(prior) if request.request.0 <= prior.0)
        {
            self.revoke();
            return Err("input request differs from current admitted origin".into());
        }
        descriptor.state = RequestAdmission::Current(request.request);
        Ok(())
    }

    /// Toggle only preallocated completion custody during preparation. This
    /// cannot admit a request before later successful issued-Play confirmation.
    pub(super) fn enable_completed_event_custody(&mut self) -> Result<(), String> {
        if self.revoked || self.issued {
            return Err("completed event custody must be enabled before issued Play".into());
        }
        self.completions.enable()
    }

    pub(super) fn completed_origin_index(
        &self,
        realization: InputRealization,
        request: HostCallRequest,
    ) -> Result<usize, String> {
        if self.revoked || !self.issued {
            return Err("input completion has no current issued origin".into());
        }
        self.descriptors.iter().position(|descriptor|
            descriptor.realization == realization
                && descriptor.operation.node == request.node
                && descriptor.operation.call == request.call
                && matches!(descriptor.state, RequestAdmission::Current(current) if current == request.request)
        ).ok_or_else(|| "input completion differs from exact accepted request".into())
    }

    /// Standing input availability only after an actual scheduler request was
    /// accepted. Prepared descriptors, proposals and caller identities confer none.
    pub(super) fn require_current_input_scope(
        &self,
        realization: InputRealization,
        plan: &conduit_body::BodyPlan,
        play: &conduit_body::BodyPlayIdentity,
        host: &HostAdvertisement,
    ) -> Result<&conduit_core::PlanId, String> {
        if self.revoked
            || !self.issued
            || !self.completions.enabled()
            || host != &self.host
            || self.body_plan.as_ref() != Some(plan)
            || self.body_play.as_ref() != Some(play)
        {
            return Err("standing input differs from current issued Body Plan/Play/Host".into());
        }
        let descriptor = self
            .descriptors
            .iter()
            .find(|descriptor| {
                descriptor.realization == realization
                    && matches!(descriptor.state, RequestAdmission::Current(_))
            })
            .ok_or("standing input request has not been accepted or was revoked")?;
        if InputRealization::from_contract(descriptor.operation.contract_id.as_str())
            != Some(realization)
            || descriptor.placement.host_id != host.host_id
            || descriptor.placement.boot_id != host.boot_id
            || descriptor.placement.offer_generation != host.offer_generation
            || !plan.plots.iter().any(|partition| {
                partition.plan.plan_id == descriptor.input_plan
                    && partition.plan.fragments.iter().any(|fragment| {
                        fragment
                            .placements
                            .iter()
                            .any(|placement| placement == &descriptor.placement)
                    })
            })
        {
            return Err("standing input differs from original admitted realization".into());
        }
        Ok(&descriptor.input_plan)
    }

    /// Completed-event origin and current issued scope only. A captured view
    /// epoch, action map, Face/Show and typed command remain separate checks.
    /// Preparation already validates the Plan/Play derivation; exact retained
    /// equality here avoids rebuilding that allocating identity during admission.
    pub(super) fn require_completed_event_scope(
        &self,
        event: &super::keyboard_input_completion::CompletedInputEvent,
        plan: &conduit_body::BodyPlan,
        play: &conduit_body::BodyPlayIdentity,
        host: &HostAdvertisement,
    ) -> Result<&conduit_core::PlanId, String> {
        if self.revoked
            || !self.issued
            || !self.completions.enabled()
            || host != &self.host
            || self.body_plan.as_ref() != Some(plan)
            || self.body_play.as_ref() != Some(play)
        {
            return Err("completed input differs from current issued Body Plan/Play/Host".into());
        }
        let descriptor = self
            .descriptors
            .get(event.descriptor)
            .ok_or("completed input descriptor is absent")?;
        let RequestAdmission::Current(latest) = descriptor.state else {
            return Err("completed input descriptor is not current".into());
        };
        if event.realization != descriptor.realization
            || InputRealization::from_contract(descriptor.operation.contract_id.as_str())
                != Some(event.realization)
            || event.request.node != descriptor.operation.node
            || event.request.call != descriptor.operation.call
            || descriptor.operation.binding.call != event.request.call
            || event.request.request.0 > latest.0
            || event.request.input.value.byte_len != 0
            || event.request.input.admitted_bytes != 0
            || event.value.byte_len > descriptor.operation.binding.maximum_output_bytes
            || descriptor.placement.host_id != host.host_id
            || descriptor.placement.boot_id != host.boot_id
            || descriptor.placement.offer_generation != host.offer_generation
            || !plan.plots.iter().any(|partition| {
                partition.plan.plan_id == descriptor.input_plan
                    && partition.plan.fragments.iter().any(|fragment| {
                        fragment
                            .placements
                            .iter()
                            .any(|placement| placement == &descriptor.placement)
                    })
            })
        {
            return Err("completed input differs from original admitted descriptor".into());
        }
        Ok(&descriptor.input_plan)
    }

    /// Check input provenance only. A source-correlated proposal still requires
    /// an authorized current mapping/revision and an exact control/label match.
    /// RawKeyboard is not itself evidence of an Enter mapping; SpaceButton is
    /// not itself evidence that semantic Fire currently means Start or Shoot.
    #[allow(dead_code)] // The downstream source-owned map admission consumes this seam.
    pub(super) fn require_input_origin(
        &self,
        realization: InputRealization,
        correlated: &conduit_face::FaceCorrelatedInputBindingProposal<'_>,
    ) -> Result<(), String> {
        let proposal = correlated.produced().proposal();
        let descriptor = self
            .descriptors
            .iter()
            .find(|descriptor| {
                self.issued
                    && descriptor.realization == realization
                    && descriptor.input_plan.as_str() == proposal.input_plan_id()
                    && matches!(descriptor.state, RequestAdmission::Current(_))
            })
            .ok_or("input origin is unaccepted or revoked")?;
        if proposal.input_plan_id() != descriptor.input_plan.as_str()
            || proposal.host_id() != descriptor.placement.host_id.as_str()
            || proposal.boot_id() != descriptor.placement.boot_id.as_str()
            || self.fragment_play.as_ref().is_some_and(|play| {
                play.plan_id != descriptor.input_plan
                    || play.host_id != descriptor.placement.host_id
                    || play.boot_id != descriptor.placement.boot_id
            })
        {
            return Err("source proposal differs from request-admitted input origin".into());
        }
        Ok(())
    }

    pub(super) fn revoke(&mut self) {
        self.revoked = true;
        self.completions.revoke();
        for descriptor in &mut self.descriptors {
            descriptor.state = RequestAdmission::Revoked;
        }
    }

    #[cfg(test)]
    pub(super) fn is_current(&self, realization: InputRealization) -> bool {
        self.issued
            && self.descriptors.iter().any(|descriptor| {
                descriptor.realization == realization
                    && matches!(descriptor.state, RequestAdmission::Current(_))
            })
    }
}

impl Drop for KeyboardRequestAdmission {
    fn drop(&mut self) {
        self.revoke();
    }
}

#[cfg(test)]
#[path = "keyboard_input_binding_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "keyboard_input_binding_body_tests.rs"]
mod body_tests;

#[cfg(test)]
#[path = "keyboard_input_scope_tests.rs"]
mod scope_tests;
